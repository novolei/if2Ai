//! Memory access control — category-based permissions
//!
//! Controls which categories a session or project can read/write,
//! limiting the blast radius of unauthorized access.

use crate::modules::memory::MemoryCategory;

/// Memory access context with permission checks
pub struct MemoryAccessContext {
    pub session_id: Option<String>,
    pub project_id: Option<String>,
    pub read_categories: Vec<MemoryCategory>,
    pub write_categories: Vec<MemoryCategory>,
}

impl MemoryAccessContext {
    /// Create access context for a specific session
    /// Sessions can read conversation + daily, write conversation
    pub fn for_session(session_id: &str) -> Self {
        Self {
            session_id: Some(session_id.to_string()),
            project_id: None,
            read_categories: vec![MemoryCategory::Conversation, MemoryCategory::Daily],
            write_categories: vec![MemoryCategory::Conversation],
        }
    }

    /// Create access context for a specific project
    /// Projects have broader access: core, daily, conversation, custom
    pub fn for_project(project_id: &str) -> Self {
        Self {
            session_id: None,
            project_id: Some(project_id.to_string()),
            read_categories: vec![
                MemoryCategory::Core,
                MemoryCategory::Daily,
                MemoryCategory::Conversation,
                MemoryCategory::Custom("project".to_string()),
            ],
            write_categories: vec![
                MemoryCategory::Core,
                MemoryCategory::Daily,
                MemoryCategory::Conversation,
                MemoryCategory::Custom("project".to_string()),
            ],
        }
    }

    /// Check if this context can read the given category
    pub fn can_read(&self, category: &MemoryCategory) -> bool {
        self.read_categories.iter().any(|c| c == category)
    }

    /// Check if this context can write the given category
    pub fn can_write(&self, category: &MemoryCategory) -> bool {
        self.write_categories.iter().any(|c| c == category)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_can_read_conversation() {
        let ctx = MemoryAccessContext::for_session("session-1");
        assert!(ctx.can_read(&MemoryCategory::Conversation));
        assert!(ctx.can_read(&MemoryCategory::Daily));
    }

    #[test]
    fn session_cannot_read_core() {
        let ctx = MemoryAccessContext::for_session("session-1");
        assert!(!ctx.can_read(&MemoryCategory::Core));
    }

    #[test]
    fn session_can_write_conversation() {
        let ctx = MemoryAccessContext::for_session("session-1");
        assert!(ctx.can_write(&MemoryCategory::Conversation));
    }

    #[test]
    fn session_cannot_write_core() {
        let ctx = MemoryAccessContext::for_session("session-1");
        assert!(!ctx.can_write(&MemoryCategory::Core));
    }

    #[test]
    fn project_can_read_and_write_core() {
        let ctx = MemoryAccessContext::for_project("project-1");
        assert!(ctx.can_read(&MemoryCategory::Core));
        assert!(ctx.can_write(&MemoryCategory::Core));
    }

    #[test]
    fn project_can_access_custom_project() {
        let ctx = MemoryAccessContext::for_project("project-1");
        let custom = MemoryCategory::Custom("project".to_string());
        assert!(ctx.can_read(&custom));
        assert!(ctx.can_write(&custom));
    }
}
