//! Commands module - Tauri command handlers
//!
//! Provides IPC commands for the frontend to interact with the backend.

use std::sync::Arc;

use crate::modules::projects::ProjectManager;
use crate::modules::session::SessionManager;
use crate::modules::tools::ToolRegistry;
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::Mutex;

use crate::modules::runtime::permissions::PermissionPromptDecision;

// Memory and learning infrastructure
use crate::modules::memory::SharedMemoryProvider;
use crate::modules::runtime::budget::ContextBudget;

/// Application state shared across all Tauri commands.
pub struct AppState {
    /// Session manager for conversation persistence.
    pub session_manager: Arc<SessionManager>,
    /// Tool registry for available tools.
    pub tool_registry: Arc<ToolRegistry>,
    /// Project manager for multi-project support.
    pub project_manager: Arc<ProjectManager>,
    /// Permission prompt senders keyed by session_id.
    /// Used by respond_permission to send user decisions back to waiting prompters.
    pub permission_senders: Arc<Mutex<HashMap<String, Sender<PermissionPromptDecision>>>>,
    /// Session-scoped permission overrides keyed by session_id -> tool_name.
    /// Used for "remember in this session" decisions from the permission dialog.
    pub permission_overrides:
        Arc<Mutex<HashMap<String, HashMap<String, PermissionPromptDecision>>>>,
    /// Stream cancel senders keyed by stream_id.
    /// Used by stop_agent_stream to cancel an in-flight streaming response.
    pub stream_cancel_senders: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>>,

    // ── Memory & Learning Infrastructure (Phase 6BW) ──
    /// Shared memory provider (SQLite / Vector / Hybrid).
    /// Wired into agent loop in slices 6bw.2-6bw.7.
    #[allow(dead_code)]
    pub memory_provider: SharedMemoryProvider,
    /// Context budget configuration (default: 4000 tokens, 10/20/30/40%).
    /// Wired into agent loop in slice 6bw.2.
    #[allow(dead_code)]
    pub context_budget: ContextBudget,

    // ── Onboarding & Configuration Platform (Phase 6G) ──
    /// Onboarding flow state machine for first-time setup.
    /// Used to enforce step ordering and prevent Settings access during onboarding.
    #[allow(dead_code)]
    pub onboarding_flow: Arc<crate::modules::onboarding::flow::OnboardingFlow>,
}

impl AppState {
    /// Create a new AppState with the given managers.
    #[must_use]
    pub fn new(
        session_manager: SessionManager,
        tool_registry: ToolRegistry,
        project_manager: ProjectManager,
        memory_provider: SharedMemoryProvider,
        context_budget: ContextBudget,
        onboarding_flow: crate::modules::onboarding::flow::OnboardingFlow,
    ) -> Self {
        Self {
            session_manager: Arc::new(session_manager),
            tool_registry: Arc::new(tool_registry),
            project_manager: Arc::new(project_manager),
            permission_senders: Arc::new(Mutex::new(HashMap::new())),
            permission_overrides: Arc::new(Mutex::new(HashMap::new())),
            stream_cancel_senders: Arc::new(Mutex::new(HashMap::new())),
            memory_provider,
            context_budget,
            onboarding_flow: Arc::new(onboarding_flow),
        }
    }
}

pub mod agent;
pub mod memory;
pub mod project;
pub mod session;
pub mod settings;
pub mod skills_hub;
pub mod slash;
pub mod stream_outcome;
pub mod tools;
pub mod web_search;
pub mod window;

// Onboarding & Configuration Platform (Phase 6G)
pub mod activation;
pub mod channel;
pub mod config;
pub mod onboarding;
pub mod provider;
pub mod system_check;

#[allow(unused_imports)]
pub use agent::{
    respond_permission, run_agent_turn, start_agent_stream, stop_agent_stream, RunAgentTurnResponse,
};
#[allow(unused_imports)]
pub use memory::{memory_delete, memory_export, memory_purge, memory_recall, MemoryEntryDto};
#[allow(unused_imports)]
pub use project::{
    create_permanent_worktree, create_project, delete_project, get_project, list_directory_preview,
    list_projects, open_directory_path, open_project_in_finder, read_file_preview, rename_project,
    write_file_contents,
};
#[allow(unused_imports)]
pub use session::{
    create_session, delete_session, get_session, list_project_sessions, list_sessions,
    rename_session, set_session_pinned,
};
#[allow(unused_imports)]
pub use settings::{
    export_trajectories, get_memory_config, set_memory_config, MemoryConfig, MemoryConfigInput,
};
#[allow(unused_imports)]
pub use skills_hub::{
    hub_audit, hub_browse, hub_check, hub_inspect, hub_install, hub_publish, hub_search,
    hub_snapshot_export, hub_snapshot_import, hub_tap_add, hub_tap_list, hub_tap_remove,
    hub_uninstall, hub_update,
};
#[allow(unused_imports)]
pub use slash::{
    execute_slash_command, list_agents, list_skills, list_slash_commands, parse_slash_command,
    resolve_skill_slash, suggest_slash_commands,
};
#[allow(unused_imports)]
pub use tools::{
    execute_tool, fetch_skills_market_audits, get_tool_definitions, list_tools, list_toolsets,
    ToolCallResult, ToolDefinition,
};
#[allow(unused_imports)]
pub use web_search::{
    get_web_search_config, remove_web_search_provider, reorder_web_search_providers,
    upsert_web_search_provider, validate_web_search_key, ProviderEntry,
};
#[allow(unused_imports)]
pub use window::{
    close_settings_window, focus_main_window_and_prefill_prompt, open_settings_window,
};

// Onboarding commands (Phase 6G)
#[allow(unused_imports)]
pub use activation::{
    activation_complete, activation_start, activation_test_message, activation_validate,
    ActivationChecklist, ActivationResult,
};
#[allow(unused_imports)]
pub use channel::{channel_configure, channel_list, channel_list_configured, channel_test};
#[allow(unused_imports)]
pub use config::{config_load, config_reset_onboarding, config_save, config_validate};
#[allow(unused_imports)]
pub use onboarding::{
    onboarding_complete, onboarding_get_state, onboarding_next_step, onboarding_prev_step,
    security_confirm,
};
#[allow(unused_imports)]
pub use provider::{
    model_select, model_test, provider_configure, provider_list, provider_list_models,
    provider_test,
};
#[allow(unused_imports)]
pub use system_check::{embedded_model_download, embedded_model_progress, system_check_run};
