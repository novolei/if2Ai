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
        }
    }
}

/// A conversation session with messages and metadata.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub struct Session {
    /// Session ID (UUID v4).
    pub id: String,
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
}

#[allow(dead_code)]
impl Session {
    /// Create a new session with a title.
    pub fn new(title: String) -> Self {
        let now = SystemTime::now();
        let now_str = format_time(now);

        Self {
            id: Uuid::new_v4().to_string(),
            title,
            messages: Vec::new(),
            created_at: now_str.clone(),
            updated_at: now_str,
            token_count: 0,
        }
    }
}

/// SessionManager handles session persistence to JSON files.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SessionManager {
    sessions_dir: PathBuf,
}

#[allow(dead_code)]
impl SessionManager {
    /// Creates a new SessionManager with the specified sessions directory.
    ///
    /// # Panics
    ///
    /// Panics if the sessions directory cannot be created.
    #[must_use]
    pub fn new(sessions_dir: PathBuf) -> Self {
        Self { sessions_dir }
    }

    /// Initialize the sessions directory.
    async fn init(&self) -> Result<(), SessionError> {
        fs::create_dir_all(&self.sessions_dir)
            .await
            .map_err(|e| SessionError::WriteError(format!("failed to create sessions dir: {e}")))?;
        Ok(())
    }

    /// Get the path to a session file.
    fn session_path(&self, id: &str) -> PathBuf {
        self.sessions_dir.join(format!("{id}.json"))
    }

    /// Create a new session with the given title.
    pub async fn create_session(&self, title: impl Into<String>) -> Result<Session, SessionError> {
        self.init().await?;

        let title = title.into();
        let session = Session::new(title);

        if self.session_path(&session.id).exists() {
            return Err(SessionError::AlreadyExists(session.id.clone()));
        }

        self.save_session(&session).await?;
        Ok(session)
    }

    /// Restore a session from storage by ID.
    pub async fn restore_session(&self, id: &str) -> Result<Session, SessionError> {
        let path = self.session_path(id);

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

    /// Save a session to storage.
    pub async fn save_session(&self, session: &Session) -> Result<(), SessionError> {
        self.init().await?;

        let path = self.session_path(&session.id);
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

    /// List all sessions.
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
                match self
                    .restore_session(path.file_stem().unwrap().to_str().unwrap())
                    .await
                {
                    Ok(session) => sessions.push(SessionMeta::from_session(&session)),
                    Err(_) => continue, // Skip invalid files
                }
            }
        }

        // Sort by creation date, newest first
        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(sessions)
    }

    /// Delete a session.
    pub async fn delete_session(&self, id: &str) -> Result<(), SessionError> {
        let path = self.session_path(id);

        if !path.exists() {
            return Err(SessionError::NotFound(id.to_string()));
        }

        fs::remove_file(&path).await.map_err(|e| {
            SessionError::WriteError(format!("failed to delete {}: {e}", path.display()))
        })?;

        Ok(())
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
        let manager = SessionManager::new(temp_dir.clone());

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
        let manager = SessionManager::new(temp_dir.clone());

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
        let manager = SessionManager::new(temp_dir.clone());

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
        let manager = SessionManager::new(temp_dir.clone());

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
        let manager = SessionManager::new(temp_dir);

        let result = manager.restore_session("nonexistent-id").await;
        assert!(result.is_err());
    }
}
