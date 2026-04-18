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
use crate::modules::memory::security::ThreatScanner;
use crate::modules::memory::JobRunner;
use crate::modules::memory::PinnedStore;
use crate::modules::memory::SessionSummaryStore;
use crate::modules::memory::SharedMemoryProvider;
use crate::modules::memory::UtilityLlm;
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

    /// Phase 8A: shared `ThreatScanner` for PII / secret detection on every
    /// memory write path.  Held as `Arc` so the same compiled regex set is
    /// reused across the SQLite + Vector providers, the `pin_memory` /
    /// `summary` modules, and any future Tauri command that ingests
    /// user-supplied content (see v2 §0.5 Δ-2).
    pub threat_scanner: Arc<ThreatScanner>,

    /// Phase 8A T-A2: shared [`JobRunner`] backed by `<memory_root>/jobs.db`.
    /// Every background memory job (rolling summary, compile, fact extract,
    /// experience extract, diary writer) goes through `job_runner.run(...)`
    /// so failures are counted, retry budget is enforced, and the LLM
    /// provider is rate-limited by a single semaphore (v2 §0.5 Δ-10 +
    /// §Sprint 1 / T-A2 + §0.7 rule 5).
    ///
    /// `allow(dead_code)`: producers land in 8A.7 (RollingSummarizer) and
    /// 8B/8C/8D (compile / facts / experience / diary).  Held on AppState
    /// from 8A.2 so subsequent slices only need to wire `state.job_runner`,
    /// not re-thread construction through `main.rs`.
    #[allow(dead_code)]
    pub job_runner: Arc<JobRunner>,

    /// Phase 8A.5 — single shared `UtilityLlm` shim (v2 §0.5 Δ-1).
    /// Every memory subsystem (rolling summary, compile_today/week,
    /// fact / experience / diary extractors) MUST dispatch its LLM
    /// calls through this `Arc<dyn UtilityLlm>` so the memory modules
    /// stay decoupled from `crate::modules::api::providers::*`.
    ///
    /// `allow(dead_code)`: first consumer lands in 8A.7
    /// (`RollingSummarizer`); held here from 8A.5 so subsequent slices
    /// only need to read `state.utility_llm`.
    #[allow(dead_code)]
    pub utility_llm: Arc<dyn UtilityLlm>,

    /// Phase 8A.5 — session-summary store backed by SQLite + JSON
    /// sidecar dual-write (v2 §Sprint 1 / T-B1 + §0.5 Δ-6).  Consumed
    /// by 8A.6+ for rolling summary persistence, by 8A.8+ for
    /// deep-memory dirty-session sweeping, and by the future
    /// `MemoryBrowser` "session summaries" tab.
    ///
    /// `allow(dead_code)`: first reader lands in 8A.7.
    #[allow(dead_code)]
    pub summary_store: Arc<dyn SessionSummaryStore>,

    /// Phase 8A.7 + 8A.8 — shared rolling-summary orchestrator.
    /// Constructed once in `main.rs::run` after `summary_store`,
    /// `utility_llm`, `job_runner`, and `threat_scanner` are all
    /// available, then handed to the future Phase 8B `TurnHook`
    /// implementation that calls
    /// [`crate::modules::memory::summary::RollingSummarizer::rolling_summary`]
    /// from `on_turn_complete` (currently no in-tree consumer; the
    /// 8B ticker slice will read this field).
    ///
    /// `allow(dead_code)`: first runtime caller lands in 8B.x; held
    /// here from 8A.8 so the next slice only needs to read
    /// `state.rolling_summarizer` instead of re-threading the
    /// constructor.
    #[allow(dead_code)]
    pub rolling_summarizer: Arc<crate::modules::memory::summary::RollingSummarizer>,

    /// Phase 8A.9 — pinned-memory store (Phase F).  Backs the
    /// `pin_memory` / `unpin_memory` tools (8A.10), the pinned section
    /// of the system-prompt injector (8A.11), and the
    /// PinnedMemoryEditor UI (8A.12).
    ///
    /// `allow(dead_code)`: first reader lands in 8A.10; held here from
    /// 8A.9 so the next slice only needs to read `state.pinned_store`.
    #[allow(dead_code)]
    pub pinned_store: Arc<dyn PinnedStore>,

    /// Phase 8B.1 — `MemoryCompiler` skeleton (Sprint 2 / T-C1).
    /// Held here so the future Phase 8B ticker (8B.6+) and the
    /// `memory_compile_now` Tauri command (8B.5) can grab a ready
    /// `Arc` instead of re-threading the four collaborators
    /// (`summary_store`, `utility_llm`, `job_runner`, `CompilerConfig`)
    /// through every call site.
    ///
    /// `allow(dead_code)`: first reader lands in 8B.3 (compile_today
    /// real implementation) / 8B.5 (memory_compile_now command).
    #[allow(dead_code)]
    pub memory_compiler: Arc<crate::modules::memory::MemoryCompiler>,

    /// Phase 8B.6 — turn-based memory scheduler (Sprint 2 / T-D1).
    /// Constructed in `main.rs::run` once
    /// [`AppState::rolling_summarizer`], [`AppState::memory_compiler`],
    /// and [`AppState::summary_store`] are all available.  The
    /// `TurnHook` impl is a no-op stub from 8B.6 — real `notify_turn`
    /// and `notify_session_end` wiring lands in 8B.7, at which point
    /// `commands/agent.rs` will install it via
    /// `ConversationRuntime::with_turn_hook(state.memory_ticker.clone())`.
    ///
    /// `allow(dead_code)`: first reader lands in 8B.7.
    #[allow(dead_code)]
    pub memory_ticker: Arc<crate::modules::memory::MemoryTicker>,
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
    /// Shared PII / secret scanner; see [`AppState::threat_scanner`].
    pub threat_scanner: Arc<ThreatScanner>,
    /// Shared background-job coordinator; see [`AppState::job_runner`].
    pub job_runner: Arc<JobRunner>,
    /// Shared utility-LLM shim; see [`AppState::utility_llm`].
    pub utility_llm: Arc<dyn UtilityLlm>,
    /// Shared session-summary store; see [`AppState::summary_store`].
    pub summary_store: Arc<dyn SessionSummaryStore>,
    /// Shared rolling-summary orchestrator; see
    /// [`AppState::rolling_summarizer`].
    pub rolling_summarizer: Arc<crate::modules::memory::summary::RollingSummarizer>,
    /// Shared pinned-memory store; see [`AppState::pinned_store`].
    pub pinned_store: Arc<dyn PinnedStore>,
    /// Shared `MemoryCompiler` skeleton; see [`AppState::memory_compiler`].
    pub memory_compiler: Arc<crate::modules::memory::MemoryCompiler>,
    /// Shared turn-based memory scheduler; see
    /// [`AppState::memory_ticker`].
    pub memory_ticker: Arc<crate::modules::memory::MemoryTicker>,
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
            threat_scanner: cfg.threat_scanner,
            job_runner: cfg.job_runner,
            utility_llm: cfg.utility_llm,
            summary_store: cfg.summary_store,
            rolling_summarizer: cfg.rolling_summarizer,
            pinned_store: cfg.pinned_store,
            memory_compiler: cfg.memory_compiler,
            memory_ticker: cfg.memory_ticker,
        }
    }
}

pub mod agent;
pub mod browser;
pub mod harness;
pub mod memory;
pub mod pinned;
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
    close_browser_session, get_browser_sessions, get_chrome_status, request_browser_status,
    ChromeStatusPayload,
};
#[allow(unused_imports)]
pub use harness::{
    get_all_session_telemetry, get_harness_status, get_session_telemetry, start_harness_recording,
    stop_harness_recording, HarnessStatusResponse, HarnessTelemetryResponse,
};
#[allow(unused_imports)]
pub use memory::{
    memory_clear_all, memory_compile_now, memory_compiled_clear, memory_compiled_read,
    memory_delete, memory_demote, memory_export, memory_promote, memory_promotion_candidates,
    memory_purge, memory_recall, memory_summaries_list, CompileReport, CompiledMemoryDto,
    CompiledSection, MemoryEntryDto, MemoryPromotionCandidateDto, SessionSummaryDto,
};
#[allow(unused_imports)]
pub use pinned::{pinned_add, pinned_delete, pinned_get, pinned_reorder, PinnedItemDto};
#[allow(unused_imports)]
pub use project::{
    create_permanent_worktree, create_project, delete_project, ensure_default_workdir, get_project,
    list_directory_preview, list_projects, open_directory_path, open_project_in_finder,
    pick_folder_dialog, read_file_preview, rename_project, write_file_contents,
};
#[allow(unused_imports)]
pub use session::{
    create_session, delete_session, get_session, list_project_sessions, list_sessions,
    memory_session_set_enabled, rename_session, set_session_pinned,
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
    browser_viewer_go_back, browser_viewer_go_forward, browser_viewer_reload,
    close_settings_window, focus_main_window_and_prefill_prompt, navigate_viewer_window,
    open_browser_viewer_window, open_settings_window,
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
