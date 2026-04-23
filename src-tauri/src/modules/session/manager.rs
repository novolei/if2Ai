//! Session Manager - JSON file-based session persistence
//!
//! Provides session creation, restoration, and management with JSON file storage.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use super::session_undo::{ConversationUndoStatus, SessionUndoRegistry};

use chrono::{DateTime, SecondsFormat, Utc};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

pub use crate::modules::runtime::session::ConversationMessage;

/// Errors that can occur during session operations.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SessionError {
    /// Session not found.
    NotFound(String),
    /// Failed to read session file.
    ReadError(String),
    /// Failed to write session file.
    WriteError(String),
    /// Invalid session data.
    InvalidData(String),
    /// Session ID already exists.
    AlreadyExists(String),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "session not found: {id}"),
            Self::ReadError(msg) => write!(f, "failed to read session: {msg}"),
            Self::WriteError(msg) => write!(f, "failed to write session: {msg}"),
            Self::InvalidData(msg) => write!(f, "invalid session data: {msg}"),
            Self::AlreadyExists(id) => write!(f, "session already exists: {id}"),
        }
    }
}

impl std::error::Error for SessionError {}

/// Format SystemTime as RFC3339 string.
fn format_time(time: SystemTime) -> String {
    let dt: DateTime<Utc> = time.into();
    dt.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Session metadata for listing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub struct SessionMeta {
    /// Session ID (UUID v4).
    pub id: String,
    /// Session title.
    pub title: String,
    /// Creation timestamp (RFC3339).
    pub created_at: String,
    /// Last update timestamp (RFC3339).
    pub updated_at: String,
    /// Whether the session is pinned.
    #[serde(default)]
    pub pinned: bool,
    /// Logical total message count for the session, including compacted history.
    #[serde(default)]
    pub message_count: usize,
    /// Per-session memory toggle. `None` (default) = follow the master
    /// switch in [`crate::modules::runtime::config::MemoryFeatureConfig`].
    /// `Some(false)` silences memory writes for this session even when
    /// the master switch is on.  Phase 8A.4 / v2 §Sprint 1 / T-A4.
    #[serde(default)]
    pub memory_enabled: Option<bool>,
    /// Wall-clock UTC instant when memory was disabled for this session.
    /// Compile pipelines use this with [`memory_reenabled_at`] to skip
    /// summary windows the operator silenced — see v2 §Sprint 1 / T-A4
    /// implementation note 3.
    ///
    /// [`memory_reenabled_at`]: SessionMeta::memory_reenabled_at
    #[serde(default)]
    pub memory_disabled_since: Option<DateTime<Utc>>,
    /// Wall-clock UTC instant when memory was last re-enabled (paired
    /// with [`SessionMeta::memory_disabled_since`]).
    #[serde(default)]
    pub memory_reenabled_at: Option<DateTime<Utc>>,
    /// Session-level Soul override.
    #[serde(default)]
    pub soul_id: Option<String>,
    /// Session-level Persona override.
    #[serde(default)]
    pub persona_id: Option<String>,
    /// Active skill ids for this session (prompt + trust attenuation).
    #[serde(default)]
    pub active_skill_ids: Vec<String>,
}

#[allow(dead_code)]
impl SessionMeta {
    /// Create a new SessionMeta from a Session.
    #[must_use]
    pub fn from_session(session: &Session) -> Self {
        Self {
            id: session.id.clone(),
            title: session.title.clone(),
            created_at: session.created_at.clone(),
            updated_at: session.updated_at.clone(),
            pinned: session.pinned,
            message_count: session.logical_message_count(),
            memory_enabled: session.memory_enabled,
            memory_disabled_since: session.memory_disabled_since,
            memory_reenabled_at: session.memory_reenabled_at,
            soul_id: session.soul_id.clone(),
            persona_id: session.persona_id.clone(),
            active_skill_ids: session.active_skill_ids.clone(),
        }
    }
}

/// Three-state truth table for per-session memory: master OFF → `false`;
/// master ON + `session.memory_enabled = Some(false)` → `false`; master
/// ON + `session.memory_enabled` is `None` or `Some(true)` → `true`.
///
/// Used by the background ticker (lands in 8B) to short-circuit rolling
/// summary / compile / fact-extract jobs for sessions the operator
/// silenced, without touching the storage layer.  Pure function — no
/// I/O, no side effects, safe to call from any thread.  Phase 8A.4 /
/// v2 §Sprint 1 / T-A4.
///
/// `allow(dead_code)`: ticker producer lands in 8B; the function is
/// already covered by unit tests in this slice.
#[must_use]
#[allow(dead_code)]
pub fn is_session_memory_on(session: &SessionMeta, master_on: bool) -> bool {
    if !master_on {
        return false;
    }
    session.memory_enabled.unwrap_or(true)
}

/// A conversation session with messages and metadata.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub struct Session {
    /// Session ID (UUID v4).
    pub id: String,
    /// Owning project ID (empty string for legacy sessions without project).
    pub project_id: String,
    /// Session title.
    pub title: String,
    /// Conversation messages.
    pub messages: Vec<ConversationMessage>,
    /// Creation timestamp (RFC3339).
    pub created_at: String,
    /// Last update timestamp (RFC3339).
    pub updated_at: String,
    /// Total token count.
    pub token_count: u64,
    /// Whether the session is pinned.
    #[serde(default)]
    pub pinned: bool,
    /// Logical total message count for the session, including messages that may
    /// have been compacted out of the persisted transcript.
    #[serde(default)]
    pub message_count: usize,
    /// Per-session memory toggle (see [`SessionMeta::memory_enabled`]).
    /// Persisted alongside the rest of the session JSON so the toggle
    /// survives restarts.  Phase 8A.4 / v2 §Sprint 1 / T-A4.
    #[serde(default)]
    pub memory_enabled: Option<bool>,
    /// Wall-clock UTC instant when memory was disabled for this session.
    #[serde(default)]
    pub memory_disabled_since: Option<DateTime<Utc>>,
    /// Wall-clock UTC instant when memory was last re-enabled.
    #[serde(default)]
    pub memory_reenabled_at: Option<DateTime<Utc>>,
    /// Session-scoped Soul override applied on top of global defaults.
    #[serde(default)]
    pub soul_id: Option<String>,
    /// Session-scoped Persona override applied on top of global defaults.
    #[serde(default)]
    pub persona_id: Option<String>,
    /// Session-scoped active skill ids (names) for prompt + trust attenuation.
    #[serde(default)]
    pub active_skill_ids: Vec<String>,
    /// Per-session running totals for provider-billable token usage and
    /// USD cost (P2-11). Updated by `stream_finalize` after each successful
    /// turn so the chat UI can render `本会话累计` without re-walking the
    /// whole transcript on every render.
    ///
    /// Optional + `#[serde(default)]` so existing on-disk session JSON
    /// stays loadable; absent means "never recorded yet".
    #[serde(default)]
    pub session_totals: Option<SessionUsageTotals>,
}

/// Persisted per-session totals counterpart to
/// [`crate::modules::runtime::stream_emitter::SessionUsageTotalsPayload`].
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SessionUsageTotals {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
    #[serde(default)]
    pub cost_usd: f64,
    #[serde(default)]
    pub turns: u32,
}

impl SessionUsageTotals {
    /// Add one turn's usage to the running totals.
    pub fn record(&mut self, usage: crate::modules::runtime::usage::TokenUsage, cost_usd: f64) {
        self.input_tokens = self
            .input_tokens
            .saturating_add(u64::from(usage.input_tokens));
        self.output_tokens = self
            .output_tokens
            .saturating_add(u64::from(usage.output_tokens));
        self.cache_creation_input_tokens = self
            .cache_creation_input_tokens
            .saturating_add(u64::from(usage.cache_creation_input_tokens));
        self.cache_read_input_tokens = self
            .cache_read_input_tokens
            .saturating_add(u64::from(usage.cache_read_input_tokens));
        self.cost_usd += cost_usd;
        self.turns = self.turns.saturating_add(1);
    }
}

#[allow(dead_code)]
impl Session {
    /// Create a new session with a title and optional project_id.
    ///
    /// # Arguments
    /// * `title` - The session title
    /// * `project_id` - The owning project ID (empty string for legacy sessions)
    pub fn new(title: String, project_id: String) -> Self {
        let now = SystemTime::now();
        let now_str = format_time(now);

        Self {
            id: Uuid::new_v4().to_string(),
            project_id,
            title,
            messages: Vec::new(),
            created_at: now_str.clone(),
            updated_at: now_str,
            token_count: 0,
            pinned: false,
            message_count: 0,
            memory_enabled: None,
            memory_disabled_since: None,
            memory_reenabled_at: None,
            soul_id: None,
            persona_id: None,
            active_skill_ids: Vec::new(),
            session_totals: None,
        }
    }

    /// Logical message count, accounting for messages compacted out of
    /// the persisted transcript: returns `messages.len()` when no
    /// `message_count` was previously recorded, otherwise the larger of
    /// the two so a session whose history was truncated still reports
    /// the original total.
    #[must_use]
    pub fn logical_message_count(&self) -> usize {
        if self.message_count == 0 {
            self.messages.len()
        } else {
            self.message_count.max(self.messages.len())
        }
    }

    // ── M4: claw-cli-compatible typed accessors ──
    //
    // The on-disk schema keeps `project_id: String` (empty == "no project"),
    // `created_at` / `updated_at: String` (RFC3339), and `token_count: u64`
    // for stable JSON serialisation across versions.  These helpers expose the
    // claw-cli-equivalent typed views (`Option<String>`, `DateTime<Utc>`,
    // `usize`) without requiring a breaking schema migration.

    /// Project identifier as `Option<String>` — `None` for legacy sessions
    /// without a parent project (stored as the empty string on disk).
    #[must_use]
    pub fn project_id_opt(&self) -> Option<&str> {
        if self.project_id.is_empty() {
            None
        } else {
            Some(self.project_id.as_str())
        }
    }

    /// Parsed `created_at` as `chrono::DateTime<Utc>`. Returns `None` when the
    /// stored RFC3339 string fails to parse (corrupted file or future schema).
    #[must_use]
    pub fn created_at_utc(&self) -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(&self.created_at)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    }

    /// Parsed `updated_at` as `chrono::DateTime<Utc>`. Returns `None` on parse
    /// failure (see [`Session::created_at_utc`]).
    #[must_use]
    pub fn updated_at_utc(&self) -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(&self.updated_at)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    }

    /// Token count as `usize` — the in-memory unit used by the budget /
    /// compaction layer.  Saturates if the on-disk `u64` exceeds `usize::MAX`
    /// on 32-bit targets (in practice unreachable for conversation transcripts).
    #[must_use]
    pub fn token_count_usize(&self) -> usize {
        usize::try_from(self.token_count).unwrap_or(usize::MAX)
    }
}

/// SessionManager handles session persistence to JSON files with dual-path support.
///
/// Path rules:
/// - `project_id == ""` → `~/.if2ai/sessions/<id>.json` (legacy path, read-only)
/// - `project_id != ""` → `~/.if2ai/projects/<project_id>/sessions/<id>.json` (new path)
///
/// The optional `active_retrieval` field enables memory injection into new
/// sessions when an `ActiveRetrievalManager` is provided via
/// [`SessionManager::with_active_retrieval`].
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SessionManager {
    /// Legacy sessions directory: ~/.if2ai/sessions/ (for backward compatibility)
    sessions_dir: PathBuf,
    /// Projects base directory: ~/.if2ai/projects/
    projects_base_dir: PathBuf,
    /// Optional active retrieval manager for injecting memory context at
    /// session creation time.  `None` when active retrieval is disabled.
    active_retrieval: Option<Arc<crate::modules::memory::retrieval::ActiveRetrievalManager>>,
    /// In-memory undo/redo stacks (transcript snapshots). Shared across clones.
    session_undo: Arc<SessionUndoRegistry>,
}

#[allow(dead_code)]
impl SessionManager {
    /// Creates a new `SessionManager` with the specified directories and no
    /// active retrieval attached.
    ///
    /// # Arguments
    /// * `sessions_dir` - Legacy sessions directory (`~/.if2ai/sessions/`)
    /// * `projects_base_dir` - Projects base directory (`~/.if2ai/projects/`)
    ///
    /// # Notes
    /// Directory creation is deferred to the first write via [`Self::init`];
    /// this constructor itself does not perform any I/O.
    #[must_use]
    pub fn new(sessions_dir: PathBuf, projects_base_dir: PathBuf) -> Self {
        Self {
            sessions_dir,
            projects_base_dir,
            active_retrieval: None,
            session_undo: Arc::new(SessionUndoRegistry::new()),
        }
    }

    /// Attach an `ActiveRetrievalManager` to this manager.
    ///
    /// When set, newly created sessions will be tagged with the retrieval
    /// configuration so that the agent loop can inject retrieved memory
    /// context before the first LLM call.
    ///
    /// # Example
    /// ```ignore
    /// use std::sync::Arc;
    /// use std::path::PathBuf;
    /// use if2ai_backend::modules::session::manager::SessionManager;
    /// use if2ai_backend::modules::memory::retrieval::ActiveRetrievalManager;
    /// let mgr = SessionManager::new(PathBuf::from("/tmp/sessions"), PathBuf::from("/tmp/projects"))
    ///     .with_active_retrieval(Arc::new(ActiveRetrievalManager::with_defaults()));
    /// ```
    #[must_use]
    pub fn with_active_retrieval(
        mut self,
        manager: Arc<crate::modules::memory::retrieval::ActiveRetrievalManager>,
    ) -> Self {
        self.active_retrieval = Some(manager);
        self
    }

    /// Returns a reference to the active retrieval manager, if one is set.
    #[must_use]
    pub fn active_retrieval(
        &self,
    ) -> Option<&Arc<crate::modules::memory::retrieval::ActiveRetrievalManager>> {
        self.active_retrieval.as_ref()
    }

    /// Initialize the sessions directory (legacy path).
    async fn init(&self) -> Result<(), SessionError> {
        fs::create_dir_all(&self.sessions_dir)
            .await
            .map_err(|e| SessionError::WriteError(format!("failed to create sessions dir: {e}")))?;
        Ok(())
    }

    /// Get the path to a session file based on project_id.
    ///
    /// Path rules:
    /// - `project_id == ""` → `sessions_dir/<id>.json` (legacy path)
    /// - `project_id != ""` → `projects_base_dir/<project_id>/sessions/<id>.json` (new path)
    fn session_path(&self, id: &str, project_id: &str) -> PathBuf {
        if project_id.is_empty() {
            // Legacy path
            self.sessions_dir.join(format!("{id}.json"))
        } else {
            // New project-scoped path
            self.projects_base_dir
                .join(project_id)
                .join("sessions")
                .join(format!("{id}.json"))
        }
    }

    /// Create a new session with the given title (legacy, no project).
    pub async fn create_session(&self, title: impl Into<String>) -> Result<Session, SessionError> {
        self._create_session_impl(title, String::new(), None).await
    }

    /// Create a new session within a specific project.
    pub async fn create_session_for_project(
        &self,
        project_id: &str,
        title: String,
    ) -> Result<Session, SessionError> {
        self._create_session_impl(title, project_id.to_string(), None)
            .await
    }

    /// Create a new session with an optional identity override.
    pub async fn create_session_with_identity(
        &self,
        title: impl Into<String>,
        project_id: String,
        soul_id: Option<String>,
        persona_id: Option<String>,
    ) -> Result<Session, SessionError> {
        self._create_session_impl(title, project_id, Some((soul_id, persona_id)))
            .await
    }

    /// Internal implementation for creating a session.
    async fn _create_session_impl(
        &self,
        title: impl Into<String>,
        project_id: String,
        identity: Option<(Option<String>, Option<String>)>,
    ) -> Result<Session, SessionError> {
        if project_id.is_empty() {
            self.init().await?;
        } else {
            // Ensure project sessions directory exists
            let sessions_dir = self.projects_base_dir.join(&project_id).join("sessions");
            fs::create_dir_all(&sessions_dir).await.map_err(|e| {
                SessionError::WriteError(format!("failed to create sessions dir: {e}"))
            })?;
        }

        let title = title.into();
        let mut session = Session::new(title, project_id.clone());
        if let Some((soul_id, persona_id)) = identity {
            session.soul_id = soul_id;
            session.persona_id = persona_id;
        }

        if self.session_path(&session.id, &project_id).exists() {
            return Err(SessionError::AlreadyExists(session.id.clone()));
        }

        self.save_session(&session).await?;
        Ok(session)
    }

    /// List sessions within a specific project (new path).
    pub async fn list_project_sessions(
        &self,
        project_id: &str,
    ) -> Result<Vec<SessionMeta>, SessionError> {
        let sessions_dir = self.projects_base_dir.join(project_id).join("sessions");

        if !sessions_dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = fs::read_dir(&sessions_dir)
            .await
            .map_err(|e| SessionError::ReadError(format!("failed to read sessions dir: {e}")))?;

        let mut sessions = Vec::new();

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| SessionError::ReadError(format!("failed to read directory entry: {e}")))?
        {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                // Walked from `read_dir` over a `.json` filter immediately
                // above; both unwraps are infallible for a valid JSON
                // filename produced by `Self::session_path`.
                #[allow(clippy::unwrap_used)]
                let id = path.file_stem().unwrap().to_str().unwrap();
                match self.restore_session_internal(id, project_id).await {
                    Ok(session) => {
                        let mut meta = SessionMeta::from_session(&session);
                        // Backward compatibility for legacy bugged timestamps:
                        // if a session has message history but updated_at never moved
                        // from created_at, use file mtime as a better "last active" proxy.
                        if session.messages.len() > 1 && meta.updated_at == meta.created_at {
                            if let Ok(stat) = fs::metadata(&path).await {
                                if let Ok(modified) = stat.modified() {
                                    meta.updated_at = format_time(modified);
                                }
                            }
                        }
                        sessions.push(meta);
                    }
                    Err(_) => continue, // Skip invalid files
                }
            }
        }

        // Sort pinned first, then by last update date, newest first
        sessions.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then_with(|| b.updated_at.cmp(&a.updated_at))
        });

        Ok(sessions)
    }

    /// Restore a session from storage by ID (internal, project_id aware).
    async fn restore_session_internal(
        &self,
        id: &str,
        project_id: &str,
    ) -> Result<Session, SessionError> {
        let path = self.session_path(id, project_id);

        if !path.exists() {
            return Err(SessionError::NotFound(id.to_string()));
        }

        let mut file = fs::File::open(&path).await.map_err(|e| {
            SessionError::ReadError(format!("failed to open {}: {e}", path.display()))
        })?;

        let mut contents = String::new();
        file.read_to_string(&mut contents).await.map_err(|e| {
            SessionError::ReadError(format!("failed to read {}: {e}", path.display()))
        })?;

        let session: Session = serde_json::from_str(&contents).map_err(|e| {
            SessionError::InvalidData(format!("failed to parse session {}: {e}", path.display()))
        })?;

        Ok(session)
    }

    /// Restore a session from storage by ID (auto-detects project_id from session).
    pub async fn restore_session(&self, id: &str) -> Result<Session, SessionError> {
        // First try legacy path
        let legacy_path = self.session_path(id, "");
        if legacy_path.exists() {
            return self.restore_session_internal(id, "").await;
        }

        // Then try new project-scoped paths by scanning projects
        let mut entries = fs::read_dir(&self.projects_base_dir)
            .await
            .map_err(|e| SessionError::ReadError(format!("failed to read projects dir: {e}")))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| SessionError::ReadError(format!("failed to read directory entry: {e}")))?
        {
            let path = entry.path();
            if path.is_dir() {
                // `path` came directly from `read_dir`, so `file_name()`
                // is always `Some` and the on-disk name is guaranteed UTF-8
                // by our own writers.
                #[allow(clippy::unwrap_used)]
                let project_id = path.file_name().unwrap().to_str().unwrap();
                let sessions_dir = path.join("sessions");
                if sessions_dir.exists() {
                    let session_path = sessions_dir.join(format!("{id}.json"));
                    if session_path.exists() {
                        return self.restore_session_internal(id, project_id).await;
                    }
                }
            }
        }

        Err(SessionError::NotFound(id.to_string()))
    }

    /// Save a session to storage (uses session.project_id for path).
    pub async fn save_session(&self, session: &Session) -> Result<(), SessionError> {
        if session.project_id.is_empty() {
            self.init().await?;
        } else {
            // Ensure project sessions directory exists
            let sessions_dir = self
                .projects_base_dir
                .join(&session.project_id)
                .join("sessions");
            fs::create_dir_all(&sessions_dir).await.map_err(|e| {
                SessionError::WriteError(format!("failed to create sessions dir: {e}"))
            })?;
        }

        // Always refresh updated_at on persistence so session list shows
        // the true "last active" time even when callers only mutate messages.
        let mut session_for_save = session.clone();
        session_for_save.updated_at = format_time(SystemTime::now());
        session_for_save.message_count = session_for_save.logical_message_count();

        let path = self.session_path(&session_for_save.id, &session_for_save.project_id);
        let contents = serde_json::to_string_pretty(&session_for_save)
            .map_err(|e| SessionError::WriteError(format!("failed to serialize session: {e}")))?;

        let mut file = fs::File::create(&path).await.map_err(|e| {
            SessionError::WriteError(format!("failed to create {}: {e}", path.display()))
        })?;

        file.write_all(contents.as_bytes()).await.map_err(|e| {
            SessionError::WriteError(format!("failed to write {}: {e}", path.display()))
        })?;

        Ok(())
    }

    /// Record transcript state **before** appending the next user turn (non-stream
    /// and streaming finalize paths). No-op when undo is disabled via env.
    pub fn push_conversation_undo_checkpoint(&self, session_id: &str, session: &Session) {
        self.session_undo.push_checkpoint(session_id, session);
    }

    /// Whether undo/redo actions are available for this session id.
    #[must_use]
    pub fn conversation_undo_status(&self, session_id: &str) -> ConversationUndoStatus {
        self.session_undo.status(session_id)
    }

    /// Restore the previous transcript snapshot and persist. Err when stack empty.
    pub async fn apply_conversation_undo(&self, session_id: &str) -> Result<Session, SessionError> {
        let mut session = self.restore_session(session_id).await?;
        if !self.session_undo.undo_into(session_id, &mut session) {
            return Err(SessionError::InvalidData(
                "nothing to undo for this session".to_string(),
            ));
        }
        self.save_session(&session).await?;
        Ok(session)
    }

    /// Re-apply a transcript snapshot popped from the redo stack.
    pub async fn apply_conversation_redo(&self, session_id: &str) -> Result<Session, SessionError> {
        let mut session = self.restore_session(session_id).await?;
        if !self.session_undo.redo_into(session_id, &mut session) {
            return Err(SessionError::InvalidData(
                "nothing to redo for this session".to_string(),
            ));
        }
        self.save_session(&session).await?;
        Ok(session)
    }

    /// Add a message to a session.
    pub async fn add_message(
        &self,
        session_id: &str,
        msg: ConversationMessage,
    ) -> Result<(), SessionError> {
        let mut session = self.restore_session(session_id).await?;

        // Update token count if available
        if let Some(usage) = &msg.usage {
            session.token_count += u64::from(usage.input_tokens + usage.output_tokens);
        }

        session.messages.push(msg);
        session.updated_at = format_time(SystemTime::now());
        session.message_count = session.messages.len().max(session.message_count);

        self.save_session(&session).await
    }

    /// Set the pinned state of a session.
    pub async fn set_session_pinned(
        &self,
        session_id: &str,
        pinned: bool,
    ) -> Result<Session, SessionError> {
        let mut session = self.restore_session(session_id).await?;
        session.pinned = pinned;
        session.updated_at = format_time(SystemTime::now());
        self.save_session(&session).await?;
        Ok(session)
    }

    /// Toggle per-session memory on or off and persist.
    ///
    /// Sets `session.memory_enabled = Some(enabled)` and stamps
    /// `memory_disabled_since` (when `enabled=false`) or
    /// `memory_reenabled_at` (when `enabled=true`) with the current UTC
    /// wall-clock so compile pipelines can later filter out the silenced
    /// window.  Phase 8A.4 / v2 §Sprint 1 / T-A4.
    pub async fn set_session_memory_enabled(
        &self,
        session_id: &str,
        enabled: bool,
    ) -> Result<Session, SessionError> {
        let mut session = self.restore_session(session_id).await?;
        let now = Utc::now();
        session.memory_enabled = Some(enabled);
        if enabled {
            session.memory_reenabled_at = Some(now);
        } else {
            session.memory_disabled_since = Some(now);
        }
        session.updated_at = format_time(SystemTime::now());
        self.save_session(&session).await?;
        Ok(session)
    }

    /// Update session-scoped Soul / Persona override and persist.
    pub async fn set_session_identity(
        &self,
        session_id: &str,
        soul_id: Option<String>,
        persona_id: Option<String>,
    ) -> Result<Session, SessionError> {
        let mut session = self.restore_session(session_id).await?;
        session.soul_id = soul_id;
        session.persona_id = persona_id;
        session.updated_at = format_time(SystemTime::now());
        self.save_session(&session).await?;
        Ok(session)
    }

    /// Replace session-scoped active skill ids and persist.
    pub async fn set_session_active_skill_ids(
        &self,
        session_id: &str,
        active_skill_ids: Vec<String>,
    ) -> Result<Session, SessionError> {
        let mut session = self.restore_session(session_id).await?;
        session.active_skill_ids = active_skill_ids;
        session.updated_at = format_time(SystemTime::now());
        self.save_session(&session).await?;
        Ok(session)
    }

    /// Rename a session and persist the updated title.
    pub async fn rename_session(
        &self,
        session_id: &str,
        title: impl Into<String>,
    ) -> Result<Session, SessionError> {
        let mut session = self.restore_session(session_id).await?;
        let next_title = title.into().trim().to_string();
        if next_title.is_empty() {
            return Err(SessionError::InvalidData(
                "session title cannot be empty".to_string(),
            ));
        }

        session.title = next_title;
        session.updated_at = format_time(SystemTime::now());
        self.save_session(&session).await?;
        Ok(session)
    }

    /// List all sessions (legacy path only, for backward compatibility).
    pub async fn list_sessions(&self) -> Result<Vec<SessionMeta>, SessionError> {
        self.init().await?;

        let mut entries = fs::read_dir(&self.sessions_dir)
            .await
            .map_err(|e| SessionError::ReadError(format!("failed to read sessions dir: {e}")))?;

        let mut sessions = Vec::new();

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| SessionError::ReadError(format!("failed to read directory entry: {e}")))?
        {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                // Same invariants as the project-scoped iterator above:
                // a `.json` filename produced by our own writer.
                #[allow(clippy::unwrap_used)]
                let id = path.file_stem().unwrap().to_str().unwrap();
                match self.restore_session_internal(id, "").await {
                    Ok(session) => {
                        let mut meta = SessionMeta::from_session(&session);
                        if session.messages.len() > 1 && meta.updated_at == meta.created_at {
                            if let Ok(stat) = fs::metadata(&path).await {
                                if let Ok(modified) = stat.modified() {
                                    meta.updated_at = format_time(modified);
                                }
                            }
                        }
                        sessions.push(meta);
                    }
                    Err(_) => continue, // Skip invalid files
                }
            }
        }

        // Sort pinned first, then by last update date, newest first
        sessions.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then_with(|| b.updated_at.cmp(&a.updated_at))
        });

        Ok(sessions)
    }

    /// Delete a session (attempts legacy path first, then new paths).
    pub async fn delete_session(&self, id: &str) -> Result<(), SessionError> {
        // First try legacy path
        let legacy_path = self.session_path(id, "");
        if legacy_path.exists() {
            fs::remove_file(&legacy_path).await.map_err(|e| {
                SessionError::WriteError(format!("failed to delete {}: {e}", legacy_path.display()))
            })?;
            self.session_undo.remove_session(id);
            return Ok(());
        }

        // Then try new project-scoped paths
        let mut entries = fs::read_dir(&self.projects_base_dir)
            .await
            .map_err(|e| SessionError::ReadError(format!("failed to read projects dir: {e}")))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| SessionError::ReadError(format!("failed to read directory entry: {e}")))?
        {
            let path = entry.path();
            if path.is_dir() {
                let sessions_dir = path.join("sessions");
                if sessions_dir.exists() {
                    let session_path = sessions_dir.join(format!("{id}.json"));
                    if session_path.exists() {
                        fs::remove_file(&session_path).await.map_err(|e| {
                            SessionError::WriteError(format!(
                                "failed to delete {}: {e}",
                                session_path.display()
                            ))
                        })?;
                        self.session_undo.remove_session(id);
                        return Ok(());
                    }
                }
            }
        }

        Err(SessionError::NotFound(id.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::runtime::session::{ContentBlock, MessageRole};
    use std::env::temp_dir;

    fn make_msg(role: MessageRole, text: &str) -> ConversationMessage {
        ConversationMessage {
            role,
            blocks: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
            thinking: None,
            usage: None,
            task_outcome: None,
            degraded_reason: None,
            resume_available: None,
            resume_cursor: None,
            request_id: None,
        }
    }

    #[test]
    fn claw_cli_typed_accessors() {
        // Legacy (no project) session: project_id_opt should be None.
        let s = Session::new("title".into(), String::new());
        assert_eq!(s.project_id_opt(), None);
        // Timestamps round-trip RFC3339 → DateTime<Utc> → back.
        let created = s.created_at_utc().expect("created_at parses");
        let updated = s.updated_at_utc().expect("updated_at parses");
        assert!(updated >= created);
        // token_count_usize returns 0 for fresh sessions.
        assert_eq!(s.token_count_usize(), 0);

        // Project-scoped session.
        let s2 = Session::new("title".into(), "proj-42".into());
        assert_eq!(s2.project_id_opt(), Some("proj-42"));
    }

    #[test]
    fn claw_cli_accessors_handle_corrupt_timestamps() {
        let mut s = Session::new("t".into(), String::new());
        s.created_at = "not-a-date".into();
        assert!(s.created_at_utc().is_none());
        // Other accessors remain functional.
        assert_eq!(s.token_count_usize(), 0);
    }

    #[tokio::test]
    async fn create_and_restore_session() {
        let temp_dir = temp_dir().join(format!("if2ai_test_{}", Uuid::new_v4()));
        let projects_dir = temp_dir.join("projects");
        let manager = SessionManager::new(temp_dir.clone(), projects_dir);

        let session = manager.create_session("Test Session").await.unwrap();
        assert_eq!(session.title, "Test Session");
        assert!(!session.id.is_empty());

        let restored = manager.restore_session(&session.id).await.unwrap();
        assert_eq!(restored.id, session.id);
        assert_eq!(restored.title, session.title);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn add_message_to_session() {
        let temp_dir = temp_dir().join(format!("if2ai_test_{}", Uuid::new_v4()));
        let projects_dir = temp_dir.join("projects");
        let manager = SessionManager::new(temp_dir.clone(), projects_dir);

        let session = manager.create_session("Test Session").await.unwrap();
        let msg = make_msg(MessageRole::User, "Hello, world!");
        manager.add_message(&session.id, msg.clone()).await.unwrap();

        let restored = manager.restore_session(&session.id).await.unwrap();
        assert_eq!(restored.messages.len(), 1);
        if let ContentBlock::Text { text } = &restored.messages[0].blocks[0] {
            assert_eq!(text, "Hello, world!");
        } else {
            panic!("Expected Text block");
        }

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn list_sessions() {
        let temp_dir = temp_dir().join(format!("if2ai_test_{}", Uuid::new_v4()));
        let projects_dir = temp_dir.join("projects");
        let manager = SessionManager::new(temp_dir.clone(), projects_dir);

        manager.create_session("Session 1").await.unwrap();
        manager.create_session("Session 2").await.unwrap();
        manager.create_session("Session 3").await.unwrap();

        let sessions = manager.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 3);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn delete_session() {
        let temp_dir = temp_dir().join(format!("if2ai_test_{}", Uuid::new_v4()));
        let projects_dir = temp_dir.join("projects");
        let manager = SessionManager::new(temp_dir.clone(), projects_dir);

        let session = manager.create_session("Test Session").await.unwrap();
        assert!(manager.restore_session(&session.id).await.is_ok());

        manager.delete_session(&session.id).await.unwrap();
        assert!(manager.restore_session(&session.id).await.is_err());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir).await;
    }

    fn make_meta(memory_enabled: Option<bool>) -> SessionMeta {
        SessionMeta {
            id: "s1".into(),
            title: "t".into(),
            created_at: "2024-01-01T00:00:00Z".into(),
            updated_at: "2024-01-01T00:00:00Z".into(),
            pinned: false,
            message_count: 0,
            memory_enabled,
            memory_disabled_since: None,
            memory_reenabled_at: None,
            soul_id: None,
            persona_id: None,
            active_skill_ids: Vec::new(),
        }
    }

    #[test]
    fn is_session_memory_on_master_off_is_always_false() {
        // Even an explicitly-enabled session must yield false when the
        // master switch is off (operator-level kill-switch wins).
        assert!(!is_session_memory_on(&make_meta(Some(true)), false));
        assert!(!is_session_memory_on(&make_meta(None), false));
        assert!(!is_session_memory_on(&make_meta(Some(false)), false));
    }

    #[test]
    fn is_session_memory_on_master_on_session_default_is_true() {
        // None means "follow master" → true when master is on.
        assert!(is_session_memory_on(&make_meta(None), true));
    }

    #[test]
    fn is_session_memory_on_master_on_session_disabled_is_false() {
        assert!(!is_session_memory_on(&make_meta(Some(false)), true));
    }

    #[test]
    fn is_session_memory_on_master_on_session_enabled_is_true() {
        assert!(is_session_memory_on(&make_meta(Some(true)), true));
    }

    #[test]
    fn session_meta_deserialises_legacy_json_without_memory_fields() {
        // Pre-8A.4 sessions on disk lack the three memory_* keys; the
        // #[serde(default)] guards must keep them at None instead of
        // failing the round-trip (= silently corrupting the user's
        // session list).
        let legacy = r#"{
            "id": "s1",
            "title": "t",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }"#;
        let meta: SessionMeta = serde_json::from_str(legacy).expect("legacy meta deserialises");
        assert_eq!(meta.memory_enabled, None);
        assert!(meta.memory_disabled_since.is_none());
        assert!(meta.memory_reenabled_at.is_none());
        assert_eq!(meta.soul_id, None);
        assert_eq!(meta.persona_id, None);
        assert!(meta.active_skill_ids.is_empty());
        assert!(!meta.pinned);
    }

    #[test]
    fn session_meta_round_trips_disabled_since_as_rfc3339() {
        let when = chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 4, 18, 12, 34, 56).unwrap();
        let meta = SessionMeta {
            id: "s1".into(),
            title: "t".into(),
            created_at: "2026-04-18T00:00:00Z".into(),
            updated_at: "2026-04-18T00:00:00Z".into(),
            pinned: false,
            message_count: 0,
            memory_enabled: Some(false),
            memory_disabled_since: Some(when),
            memory_reenabled_at: None,
            soul_id: Some("if2ai-core".into()),
            persona_id: Some("staff-architect".into()),
            active_skill_ids: Vec::new(),
        };
        let json = serde_json::to_string(&meta).expect("serialise");
        // chrono serialises DateTime<Utc> as RFC3339 by default.
        assert!(
            json.contains("2026-04-18T12:34:56"),
            "expected RFC3339 timestamp in {json}"
        );
        let back: SessionMeta = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(back.memory_enabled, Some(false));
        assert_eq!(back.memory_disabled_since, Some(when));
        assert_eq!(back.soul_id.as_deref(), Some("if2ai-core"));
        assert_eq!(back.persona_id.as_deref(), Some("staff-architect"));
    }

    #[tokio::test]
    async fn create_session_with_identity_persists_fields() {
        let temp_dir = temp_dir().join(format!("if2ai_test_{}", Uuid::new_v4()));
        let projects_dir = temp_dir.join("projects");
        let manager = SessionManager::new(temp_dir.clone(), projects_dir);

        let session = manager
            .create_session_with_identity(
                "Identity Session",
                "project-1".to_string(),
                Some("if2ai-core".to_string()),
                Some("staff-architect".to_string()),
            )
            .await
            .expect("create session with identity");

        assert_eq!(session.soul_id.as_deref(), Some("if2ai-core"));
        assert_eq!(session.persona_id.as_deref(), Some("staff-architect"));

        let restored = manager.restore_session(&session.id).await.expect("restore");
        assert_eq!(restored.soul_id.as_deref(), Some("if2ai-core"));
        assert_eq!(restored.persona_id.as_deref(), Some("staff-architect"));

        let _ = fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn set_session_identity_updates_metadata() {
        let temp_dir = temp_dir().join(format!("if2ai_test_{}", Uuid::new_v4()));
        let projects_dir = temp_dir.join("projects");
        let manager = SessionManager::new(temp_dir.clone(), projects_dir);

        let session = manager
            .create_session("Identity Session")
            .await
            .expect("create");
        let updated = manager
            .set_session_identity(
                &session.id,
                Some("if2ai-core".to_string()),
                Some("execution-partner".to_string()),
            )
            .await
            .expect("set session identity");

        assert_eq!(updated.soul_id.as_deref(), Some("if2ai-core"));
        assert_eq!(updated.persona_id.as_deref(), Some("execution-partner"));

        let restored = manager.restore_session(&session.id).await.expect("restore");
        assert_eq!(restored.soul_id.as_deref(), Some("if2ai-core"));
        assert_eq!(restored.persona_id.as_deref(), Some("execution-partner"));

        let _ = fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn set_session_memory_enabled_disables_and_stamps_disabled_since() {
        let temp_dir = temp_dir().join(format!("if2ai_test_{}", Uuid::new_v4()));
        let projects_dir = temp_dir.join("projects");
        let manager = SessionManager::new(temp_dir.clone(), projects_dir);

        let session = manager.create_session("Test").await.expect("create");
        let updated = manager
            .set_session_memory_enabled(&session.id, false)
            .await
            .expect("disable");
        assert_eq!(updated.memory_enabled, Some(false));
        assert!(updated.memory_disabled_since.is_some());
        assert!(updated.memory_reenabled_at.is_none());

        // Re-enable: memory_enabled flips to Some(true) and reenabled_at is stamped.
        let reenabled = manager
            .set_session_memory_enabled(&session.id, true)
            .await
            .expect("enable");
        assert_eq!(reenabled.memory_enabled, Some(true));
        assert!(reenabled.memory_reenabled_at.is_some());

        let _ = fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn restore_nonexistent_returns_error() {
        let temp_dir = temp_dir().join(format!("if2ai_test_{}", Uuid::new_v4()));
        let projects_dir = temp_dir.join("projects");
        let manager = SessionManager::new(temp_dir, projects_dir);

        let result = manager.restore_session("nonexistent-id").await;
        assert!(result.is_err());
    }
}
