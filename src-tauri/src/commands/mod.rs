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
    /// Application service registry (GAP-004 T-009).
    /// New commands route through services in this registry rather than
    /// accessing domain internals directly.  Existing direct fields are
    /// retained for backward compatibility during the transition.
    pub service_registry: Arc<crate::modules::application::ServiceRegistry>,
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
    /// As of truth-loop iter-7 (DW-002 plumbing), this handle is also
    /// threaded through `TurnServiceDeps` → `StreamTaskInputs` so the
    /// preflight digester reuses one shared `Arc<dyn UtilityLlm>`
    /// instead of constructing a new `ChatProviderUtilityLlm` per
    /// outer-loop iteration. The `#[allow(dead_code)]` was removed in
    /// the same change because the field now has live consumers in
    /// both the memory subsystems and the streaming turn pipeline.
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

    /// MEM-MOD-P7 — cross-session learned-traits store.  `None` until
    /// bootstrap calls [`AppState::with_learned_traits`].  IPCs that
    /// touch this field degrade to "no traits" while it is `None`.
    pub learned_traits: Option<crate::modules::memory::learned_traits::LearnedTraitsStore>,
}

/// Constructor arguments for [`AppState`].
///
/// Bundled into a struct so that `AppState::new` does not exceed the
/// clippy `too_many_arguments` limit (7).
pub struct AppStateConfig {
    /// Application service registry (GAP-004 T-009).
    /// Owns the core business services (SessionManager, ToolRegistry,
    /// ProjectManager).  [`AppState::new`] extracts individual `Arc<>`
    /// references from the registry for backward-compatible field access.
    pub service_registry: crate::modules::application::ServiceRegistry,
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
        // GAP-004 T-009: service_registry is the canonical container;
        // individual fields are extracted here for backward-compatible
        // `state.session_manager` / `state.tool_registry` /
        // `state.project_manager` access.
        let registry = Arc::new(cfg.service_registry);
        Self {
            session_manager: registry.session_manager.clone(),
            tool_registry: registry.tool_registry.clone(),
            project_manager: registry.project_manager.clone(),
            service_registry: registry,
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
            learned_traits: None,
        }
    }

    /// MEM-MOD-P7 — opt-in: attach the cross-session learned-traits
    /// store after construction.  Builder method (consumes `self`)
    /// so the existing `AppStateConfig` stays unchanged and the
    /// in-flight bootstrap god-file refactor does not need to learn
    /// about this field today.  Bootstrap will start calling this
    /// once the refactor stream lands; until then the IPCs degrade
    /// gracefully to "no traits".
    #[must_use]
    pub fn with_learned_traits(
        mut self,
        store: crate::modules::memory::learned_traits::LearnedTraitsStore,
    ) -> Self {
        self.learned_traits = Some(store);
        self
    }
}

pub mod agent;
pub mod browser;
pub mod command_surface;
pub mod gateway;
pub mod git;
pub mod harness;
pub mod host_composition;
pub mod jiaochang_audio;
pub mod learning;
pub mod memory;
pub mod pinned;
pub mod project;
pub mod request_intelligence;
pub mod session;
pub mod settings;
pub mod skills_hub;
pub mod slash;
// stream_outcome moved to crate::modules::runtime::stream_outcome (MIG-001-c)
// to satisfy the application/* layering constraint (CHARTER §2.1).
// No re-export here — call sites import via the runtime module
// path directly.
pub mod chat_compact;
pub mod stt;
pub mod tools;
pub mod tts;
pub mod tts_download;
pub mod updater;
pub mod usage;
pub mod web_search;

#[allow(unused_imports)]
pub use stt::{
    stt_download_openflow_model, stt_get_settings, stt_model_status, stt_save_settings,
    stt_transcribe,
};
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
    approve_smart_browser_cloud_escalation, clear_browser_profile, close_browser_session,
    deny_smart_browser_cloud_escalation, get_browser_action_log, get_browser_sessions,
    get_browser_settings, get_chrome_status, list_browser_profiles, release_browser_takeover,
    request_browser_status, request_browser_takeover, request_smart_browser_cloud_escalation,
    set_browser_settings, ChromeStatusPayload,
};
#[allow(unused_imports)]
pub use gateway::{get_gateway_health, get_gateway_url};
#[allow(unused_imports)]
pub use git::{
    gh_available, gh_create_issue, gh_create_pr, git_add_worktree, git_branches, git_commit,
    git_commit_push_pr, git_current_branch, git_default_branch, git_diff, git_prune_worktrees,
    git_remove_worktree, git_status, git_worktrees, CommitOutcome, CreatePrResponse,
};
#[allow(unused_imports)]
pub use harness::{
    get_all_session_telemetry, get_harness_status, get_session_telemetry,
    harness_aggregate_suite_report, harness_begin_run, harness_compare_reports,
    harness_current_run_id, harness_delete_report, harness_evaluate_compare,
    harness_evaluate_suite, harness_finalize_and_rotate_run, harness_finalize_run,
    harness_list_reports, harness_load_corpus, harness_load_report, harness_save_report,
    start_harness_recording, stop_harness_recording, HarnessStatusResponse,
    HarnessTelemetryResponse,
};
#[allow(unused_imports)]
pub use host_composition::{compose_desktop_host_state, DesktopHostComposition};
#[allow(unused_imports)]
pub use jiaochang_audio::{
    jiaochang_audio_plugin_cache_clear, jiaochang_audio_plugin_cache_info,
    jiaochang_audio_plugin_import_lx_ceru_js_file, jiaochang_audio_plugin_inspect_js_file,
    jiaochang_audio_plugin_install_authorized_cn_template, jiaochang_audio_plugin_list,
    jiaochang_audio_plugin_register, jiaochang_audio_plugin_remove,
    jiaochang_audio_plugin_resolve_track_url, jiaochang_audio_plugin_search_tracks,
    jiaochang_audio_plugin_set_enabled,
};
#[allow(unused_imports)]
pub use learning::{
    learning_activate_promoted_candidate, learning_apply_promotion_gate,
    learning_attach_compare_ref, learning_attach_recommendation, learning_delete_candidate,
    learning_evaluate_candidate, learning_evaluate_candidate_against_suite,
    learning_evaluate_candidate_with_policy, learning_generate_reflection_for_report,
    learning_get_active_strategies, learning_get_candidate, learning_inspect_evaluation_progress,
    learning_list_candidates, learning_mark_promoted_candidate,
    learning_reflect_session_and_register, learning_register_candidate_from_reflection,
    learning_register_candidate_manual, learning_resolve_active_overlay,
    learning_rollback_active_strategy, learning_score_run_report,
    learning_set_candidate_compare_target, learning_set_candidate_definition,
    learning_set_candidate_notes, learning_set_candidate_state, LearningActivateInput,
    LearningEvaluateCandidateAgainstSuiteInput, LearningEvaluateCandidateAgainstSuiteResponse,
    LearningEvaluateCandidateResponse, LearningPromotionGateResponse, LearningRollbackInput,
    ReflectionGenerationResponse,
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
    create_session, delete_session, drain_job_monitor_lines, generate_session_title, get_session,
    list_project_sessions, list_sessions, memory_session_set_enabled, rename_session, session_redo,
    session_undo, session_undo_status, set_session_active_skill_ids, set_session_identity,
    set_session_pinned,
};
#[allow(unused_imports)]
pub use settings::{
    export_trajectories, get_identity_customization_pack, get_mcp_service_config,
    get_memory_config, get_prompt_control_catalog, get_prompt_control_settings,
    mcp_workbench_activity, mcp_workbench_call_tool, mcp_workbench_discover,
    mcp_workbench_get_prompt, mcp_workbench_list_prompts, mcp_workbench_list_resources,
    mcp_workbench_list_servers, mcp_workbench_read_resource, set_identity_customization_pack,
    set_mcp_service_config, set_memory_config, set_prompt_control_settings,
    IdentityCustomizationPackDto, McpServiceConfig, McpServiceConfigInput, McpServiceEntry,
    McpServiceEntryInput, McpServiceTransportSetting, McpWorkbenchErrorDto,
    McpWorkbenchGetPromptRequest, McpWorkbenchReadResourceRequest, McpWorkbenchToolCallRequest,
    MemoryConfig, MemoryConfigInput, PromptControlCatalog, PromptControlSettings,
    PromptControlSettingsInput,
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
    activation_complete, activation_deactivate, activation_get_installation_id,
    activation_get_status, activation_poll_request_status, activation_redeem_by_invite_code,
    activation_redeem_with_request_id, activation_refresh, activation_request_license,
    activation_revoke_check, activation_start, activation_test_message, activation_validate,
    ActivationChecklist, ActivationErrorDto, ActivationPollResponseDto,
    ActivationRequestPayloadDto, ActivationResult, InstallationIdentity,
};
// Request intelligence (Phase M2.6)
#[allow(unused_imports)]
pub use channel::{channel_configure, channel_list, channel_list_configured, channel_test};
#[allow(unused_imports)]
pub use chat_compact::{chat_compact_session, CompactReport, COMPACT_COMPLETED_EVENT};
#[allow(unused_imports)]
pub use config::{config_load, config_reset_onboarding, config_save, config_validate};
#[allow(unused_imports)]
pub use onboarding::{
    onboarding_complete, onboarding_get_state, onboarding_next_step, onboarding_prev_step,
    security_confirm,
};
#[allow(unused_imports)]
pub use provider::{
    model_get_active, model_get_context_window, model_get_role_config, model_list_available,
    model_select, model_set_active, model_set_role_config, model_test, provider_configure,
    provider_configure_with_model_capabilities, provider_configure_with_models,
    provider_get_all_configured_models, provider_get_config, provider_get_configured_models,
    provider_list, provider_list_configured, provider_list_models, provider_probe_model_thinking,
    provider_test,
};
#[allow(unused_imports)]
pub use request_intelligence::{request_intelligence_classify, RequestIntelligenceClassifyInput};
#[allow(unused_imports)]
pub use system_check::{
    embedded_model_download, embedded_model_progress, get_model_config, set_model_config,
    system_check_run,
};
#[allow(unused_imports)]
pub use tts::{
    delete_tts_profile, get_tts_settings, list_tts_profiles, save_tts_profile,
    set_default_tts_profile, set_tts_settings, tts_cached_voice_preview, tts_delete_user_voice,
    tts_demo_audio, tts_health, tts_list_voice_assets, tts_list_voices, tts_preview_voice,
    tts_rename_user_voice, tts_split_text, tts_start_warmup, tts_stream_close, tts_stream_result,
    tts_stream_start, tts_stream_status, tts_synthesize, tts_upload_user_voice, tts_voice_audio,
    tts_warm_voice_preview, tts_warmup_status, ProviderHandle, ProviderState, TtsState,
};
#[allow(unused_imports)]
pub use usage::usage_summary;

// TTS model download commands
#[allow(unused_imports)]
pub use tts_download::{tts_model_download_start, tts_model_download_status, tts_model_status};
#[allow(unused_imports)]
pub use updater::{
    app_updater_check, app_updater_check_manifest, app_updater_download_and_install,
    app_updater_download_and_open, app_updater_get_state, app_updater_set_preferences,
};
