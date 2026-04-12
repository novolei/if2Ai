//! Session Manager - JSON file-based session persistence
//!
//! Provides session creation, restoration, and management with JSON file storage.

use std::path::PathBuf;
use std::time::SystemTime;

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
    let secs = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Format as ISO 8601 / RFC3339
    // 2026-04-12T00:00:00Z
    let secs_in_day = secs % 86400;
    let hours = secs_in_day / 3600;
    let mins = (secs_in_day % 3600) / 60;
    let secs_in_min = secs_in_day % 60;
    format!("2026-04-12T{:02}:{:02}:{:02}Z", hours, mins, secs_in_min)
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
    /// Whether the session is pinned.
    #[serde(default)]
    pub pinned: bool,
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
            pinned: session.pinned,
        }
    }
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
        }
    }
}

/// SessionManager handles session persistence to JSON files with dual-path support.
///
/// Path rules:
/// - `project_id == ""` → `~/.if2ai/sessions/<id>.json` (legacy path, read-only)
/// - `project_id != ""` → `~/.if2ai/projects/<project_id>/sessions/<id>.json` (new path)
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SessionManager {
    /// Legacy sessions directory: ~/.if2ai/sessions/ (for backward compatibility)
    sessions_dir: PathBuf,
    /// Projects base directory: ~/.if2ai/projects/
    projects_base_dir: PathBuf,
}

#[allow(dead_code)]
impl SessionManager {
    /// Creates a new SessionManager with the specified directories.
    ///
    /// # Arguments
    /// * `sessions_dir` - Legacy sessions directory (~/.if2ai/sessions/)
    /// * `projects_base_dir` - Projects base directory (~/.if2ai/projects/)
    ///
    /// # Panics
    ///
    /// Panics if the sessions directory cannot be created.
    #[must_use]
    pub fn new(sessions_dir: PathBuf, projects_base_dir: PathBuf) -> Self {
        Self {
            sessions_dir,
            projects_base_dir,
        }
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
        self._create_session_impl(title, String::new()).await
    }

    /// Create a new session within a specific project.
    pub async fn create_session_for_project(
        &self,
        project_id: &str,
        title: String,
    ) -> Result<Session, SessionError> {
        self._create_session_impl(title, project_id.to_string())
            .await
    }

    /// Internal implementation for creating a session.
    async fn _create_session_impl(
        &self,
        title: impl Into<String>,
        project_id: String,
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
        let session = Session::new(title, project_id.clone());

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
                let id = path.file_stem().unwrap().to_str().unwrap();
                match self.restore_session_internal(id, project_id).await {
                    Ok(session) => sessions.push(SessionMeta::from_session(&session)),
                    Err(_) => continue, // Skip invalid files
                }
            }
        }

        // Sort pinned first, then by creation date, newest first
        sessions.sort_by(|a, b| b.pinned.cmp(&a.pinned).then_with(|| b.created_at.cmp(&a.created_at)));

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

        let path = self.session_path(&session.id, &session.project_id);
        let contents = serde_json::to_string_pretty(session)
            .map_err(|e| SessionError::WriteError(format!("failed to serialize session: {e}")))?;

        let mut file = fs::File::create(&path).await.map_err(|e| {
            SessionError::WriteError(format!("failed to create {}: {e}", path.display()))
        })?;

        file.write_all(contents.as_bytes()).await.map_err(|e| {
            SessionError::WriteError(format!("failed to write {}: {e}", path.display()))
        })?;

        Ok(())
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
                let id = path.file_stem().unwrap().to_str().unwrap();
                match self.restore_session_internal(id, "").await {
                    Ok(session) => sessions.push(SessionMeta::from_session(&session)),
                    Err(_) => continue, // Skip invalid files
                }
            }
        }

        // Sort pinned first, then by creation date, newest first
        sessions.sort_by(|a, b| b.pinned.cmp(&a.pinned).then_with(|| b.created_at.cmp(&a.created_at)));

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
            usage: None,
        }
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

    #[tokio::test]
    async fn restore_nonexistent_returns_error() {
        let temp_dir = temp_dir().join(format!("if2ai_test_{}", Uuid::new_v4()));
        let projects_dir = temp_dir.join("projects");
        let manager = SessionManager::new(temp_dir, projects_dir);

        let result = manager.restore_session("nonexistent-id").await;
        assert!(result.is_err());
    }
}
