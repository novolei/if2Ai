//! Commands module - Tauri command handlers
//!
//! Provides IPC commands for the frontend to interact with the backend.

use std::sync::Arc;

use crate::modules::learning::trajectory::TrajectoryManager;
use crate::modules::learning::LearningModule;
use crate::modules::memory::retrieval::ActiveRetrievalManager;
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
    pub memory_provider: SharedMemoryProvider,
    /// Context budget configuration (default: 4000 tokens, 10/20/30/40%).
    pub context_budget: ContextBudget,
    /// Trajectory manager for ShareGPT JSONL persistence across turns.
    /// `None` when the trajectories directory cannot be initialised (graceful fallback).
    pub trajectory_manager: Option<Arc<TrajectoryManager>>,
    /// Learning module: self-model + reflection engine + trust tracker.
    /// Shared across turns so `SelfModel` state accumulates over time.
    /// `None` when initialisation fails (graceful fallback).
    pub learning_module: Option<Arc<tokio::sync::Mutex<LearningModule>>>,
    /// Active retrieval manager: classifies intent and fuses multi-layer memories.
    /// Shared so config is not re-created per turn.
    /// Always `Some` with default config; wrapped in `Option` for consistency.
    pub active_retrieval_manager: Option<Arc<ActiveRetrievalManager>>,

    // ── Onboarding & Configuration Platform (Phase 6G) ──
    /// Onboarding flow state machine for first-time setup.
    /// Wired into onboarding commands in Phase 6G; not yet read by other code paths.
    #[allow(dead_code)]
    pub onboarding_flow: Arc<crate::modules::onboarding::flow::OnboardingFlow>,

    // ── Agent Loop Harness (Phase 6E) ──
    /// Harness state: event bus + telemetry collector + session recorder.
    /// `None` when harness is disabled (default production mode).
    /// `Some(...)` when developer/eval recording mode is active.
    pub harness: Option<Arc<crate::modules::harness::HarnessState>>,
}

/// Constructor arguments for [`AppState`].
///
/// Bundled into a struct so that `AppState::new` does not exceed the
/// clippy `too_many_arguments` limit (7).
pub struct AppStateConfig {
    /// Session manager for conversation persistence.
    pub session_manager: SessionManager,
    /// Tool registry for available tools.
    pub tool_registry: ToolRegistry,
    /// Project manager for multi-project support.
    pub project_manager: ProjectManager,
    /// Shared memory provider (SQLite / Vector / Hybrid).
    pub memory_provider: SharedMemoryProvider,
    /// Context budget configuration (default: 4000 tokens, 10/20/30/40%).
    pub context_budget: ContextBudget,
    /// Onboarding flow state machine for first-time setup (Phase 6G).
    pub onboarding_flow: crate::modules::onboarding::flow::OnboardingFlow,
    /// Trajectory manager for ShareGPT JSONL persistence across turns.
    /// `None` when the trajectories directory cannot be initialised (graceful fallback).
    pub trajectory_manager: Option<Arc<TrajectoryManager>>,
    /// Learning module: self-model + reflection engine + trust tracker.
    /// Shared across turns so `SelfModel` state accumulates over time.
    /// `None` when initialisation fails (graceful fallback).
    pub learning_module: Option<Arc<tokio::sync::Mutex<LearningModule>>>,
    /// Active retrieval manager: classifies intent and fuses multi-layer memories.
    /// Shared so config is not re-created per turn.
    pub active_retrieval_manager: Option<Arc<ActiveRetrievalManager>>,
    /// Harness state for agent loop observability.
    /// `None` disables all harness overhead (default production mode).
    pub harness: Option<Arc<crate::modules::harness::HarnessState>>,
}

impl AppState {
    /// Create a new `AppState` from the given configuration.
    ///
    /// The three memory/learning fields are `Option` so the app starts gracefully
    /// even when their backing stores or init routines are unavailable.
    #[must_use]
    pub fn new(cfg: AppStateConfig) -> Self {
        Self {
            session_manager: Arc::new(cfg.session_manager),
            tool_registry: Arc::new(cfg.tool_registry),
            project_manager: Arc::new(cfg.project_manager),
            permission_senders: Arc::new(Mutex::new(HashMap::new())),
            permission_overrides: Arc::new(Mutex::new(HashMap::new())),
            stream_cancel_senders: Arc::new(Mutex::new(HashMap::new())),
            memory_provider: cfg.memory_provider,
            context_budget: cfg.context_budget,
            trajectory_manager: cfg.trajectory_manager,
            learning_module: cfg.learning_module,
            active_retrieval_manager: cfg.active_retrieval_manager,
            onboarding_flow: Arc::new(cfg.onboarding_flow),
            harness: cfg.harness,
        }
    }
}

pub mod agent;
pub mod browser;
pub mod harness;
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
pub use browser::{
    close_browser_session, get_browser_sessions, get_chrome_status, ChromeStatusPayload,
};
#[allow(unused_imports)]
pub use harness::{
    get_all_session_telemetry, get_harness_status, get_session_telemetry, start_harness_recording,
    stop_harness_recording, HarnessStatusResponse, HarnessTelemetryResponse,
};
#[allow(unused_imports)]
pub use memory::{memory_delete, memory_export, memory_purge, memory_recall, MemoryEntryDto};
#[allow(unused_imports)]
pub use project::{
    create_permanent_worktree, create_project, delete_project, ensure_default_workdir, get_project,
    list_directory_preview, list_projects, open_directory_path, open_project_in_finder,
    pick_folder_dialog, read_file_preview, rename_project, write_file_contents,
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
    close_settings_window, focus_main_window_and_prefill_prompt, open_browser_viewer_window,
    open_settings_window,
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
    model_get_active, model_get_role_config, model_list_available, model_select, model_set_active,
    model_set_role_config, model_test, provider_configure, provider_configure_with_models,
    provider_get_all_configured_models, provider_get_config, provider_get_configured_models,
    provider_list, provider_list_configured, provider_list_models, provider_test,
};
#[allow(unused_imports)]
pub use system_check::{
    embedded_model_download, embedded_model_progress, get_model_config, set_model_config,
    system_check_run,
};
