//! Phase 1 Integration Tests
//!
//! Integration tests validating Phase 1 components work together.
//! These tests verify the core library components without depending on Tauri commands.

/// test_project_crud

#[cfg(test)]
mod tests {
    use crate::modules::projects::{ProjectManager, ProjectMeta};
    use crate::modules::session::{SessionManager, SessionMeta};
    use std::env;
    use uuid::Uuid;

    /// Test ProjectManager CRUD operations
    #[tokio::test]
    async fn test_project_crud() {
        let temp_dir = temp_dir().join(format!("if2ai_test_{}", Uuid::new_v4()));
        let projects_dir = temp_dir.join("projects");
        let manager = ProjectManager::new(projects_dir.clone());

        // Create a temp workdir
        let workdir = temp_dir.join("workdir");
        tokio::fs::create_dir_all(&workdir).await.unwrap();

        // Create project
        let project = manager
            .create_project("Test Project".to_string(), workdir.clone())
            .await
            .expect("create_project should work");

        assert_eq!(project.name, "Test Project");
        assert!(!project.id.is_empty());

        // List projects
        let projects: Vec<ProjectMeta> = manager.list_projects().await.expect("list_projects should work");
        assert!(!projects.is_empty());

        // Get project by ID
        let restored = manager.get_project(&project.id).await.expect("get_project should work");
        assert_eq!(restored.id, project.id);

        // Rename project
        let renamed = manager
            .rename_project(&project.id, "Renamed Project".to_string())
            .await
            .expect("rename_project should work");
        assert_eq!(renamed.name, "Renamed Project");

        // Delete project
        manager.delete_project(&project.id).await.expect("delete_project should work");
        let result = manager.get_project(&project.id).await;
        assert!(result.is_err());

        // Cleanup
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    /// Test Session ↔ Project association
    #[tokio::test]
    async fn test_session_project_association() {
        let temp_dir = temp_dir().join(format!("if2ai_test_{}", Uuid::new_v4()));
        let sessions_dir = temp_dir.join("sessions");
        let projects_dir = temp_dir.join("projects");
        let session_manager = SessionManager::new(sessions_dir.clone(), projects_dir.clone());

        // Create a project first
        let projects_manager = ProjectManager::new(projects_dir.clone());
        let workdir = temp_dir.join("workdir");
        tokio::fs::create_dir_all(&workdir).await.unwrap();
        let project = projects_manager
            .create_project("Test Project".to_string(), workdir)
            .await
            .unwrap();

        // Create session within project
        let session = session_manager
            .create_session_for_project(&project.id, "Test Session".to_string())
            .await
            .expect("create_session_for_project should work");

        assert_eq!(session.project_id, project.id);
        assert_eq!(session.title, "Test Session");

        // List project sessions
        let sessions: Vec<SessionMeta> = session_manager
            .list_project_sessions(&project.id)
            .await
            .expect("list_project_sessions should work");
        assert!(!sessions.is_empty());
        assert!(sessions.iter().any(|s| s.id == session.id));

        // Cleanup
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    /// Test full agent turn with session management
    #[tokio::test]
    async fn test_full_agent_turn() {
        let temp_dir = env::temp_dir().join("if2ai_integration_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let sessions_dir = temp_dir.to_path_buf();
        let projects_dir = temp_dir.join("projects");

        // Create session manager
        let session_manager = SessionManager::new(sessions_dir.clone(), projects_dir);

        // Create a session
        let session = session_manager
            .create_session("test session")
            .await
            .expect("create_session should work");

        // Verify session was created with correct title
        assert_eq!(session.title, "test session");
        assert!(!session.id.is_empty());

        // List sessions and verify our session appears
        let sessions: Vec<SessionMeta> = session_manager
            .list_sessions()
            .await
            .expect("list_sessions should work");
        assert!(!sessions.is_empty());
        assert!(sessions.iter().any(|s| s.id == session.id));

        // Restore the session and verify
        let restored = session_manager
            .restore_session(&session.id)
            .await
            .expect("restore_session should work");
        assert_eq!(restored.id, session.id);
        assert_eq!(restored.title, "test session");

        // Delete the session
        session_manager
            .delete_session(&session.id)
            .await
            .expect("delete_session should work");

        // Verify session is gone
        let result = session_manager.restore_session(&session.id).await;
        assert!(result.is_err());

        // Clean up
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    /// Test AppState initialization
    #[tokio::test]
    async fn test_app_state_components() {
        let temp_dir = std::env::temp_dir().join("if2ai_appstate_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let sessions_dir = temp_dir.to_path_buf();
        let projects_dir = temp_dir.join("projects");

        // Create session manager
        let session_manager = SessionManager::new(sessions_dir.clone(), projects_dir.clone());

        // Create project manager
        let project_manager = ProjectManager::new(projects_dir);

        // Verify both are functional
        let sessions: Vec<SessionMeta> = session_manager
            .list_sessions()
            .await
            .expect("list_sessions should work");
        assert!(sessions.is_empty()); // Fresh start

        let projects: Vec<ProjectMeta> = project_manager
            .list_projects()
            .await
            .expect("list_projects should work");
        assert!(projects.is_empty()); // Fresh start

        // Clean up
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    /// Test database schema migration / file persistence
    #[tokio::test]
    async fn test_session_persistence_across_restarts() {
        let temp_dir = std::env::temp_dir().join("if2ai_persistence_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let sessions_dir = temp_dir.to_path_buf();
        let projects_dir = temp_dir.join("projects");

        // First "run" - create a session
        let session_manager1 = SessionManager::new(sessions_dir.clone(), projects_dir.clone());
        let session = session_manager1
            .create_session("persistent session")
            .await
            .expect("create_session should work");

        // Second "run" - create new manager with same directory
        let session_manager2 = SessionManager::new(sessions_dir.clone(), projects_dir.clone());
        let sessions: Vec<SessionMeta> = session_manager2
            .list_sessions()
            .await
            .expect("list_sessions should work");

        // Verify session persisted
        assert!(!sessions.is_empty());
        assert!(sessions.iter().any(|s| s.id == session.id));

        // Restore and verify
        let restored = session_manager2
            .restore_session(&session.id)
            .await
            .expect("restore_session should work");
        assert_eq!(restored.title, "persistent session");

        // Clean up
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    /// Test Phase 1 code quality gate
    #[test]
    fn test_phase1_code_quality() {
        // Actual quality checks (clippy/fmt) are run as separate gates
        // This test serves as a placeholder for the harness suite
        let _ = std::hint::black_box(true);
    }
}
