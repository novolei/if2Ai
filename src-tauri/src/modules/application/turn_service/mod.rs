//! Turn service — canonical chat turn orchestrator (MIG-001).
//!
//! `TurnService` is the **only** entry point that the IPC adapter
//! [`crate::commands::agent::run_agent_turn`] /
//! [`crate::commands::agent::start_agent_stream`] uses to compose
//! provider resolution + memory injection + prompt planning, and (in
//! later sub-packs of MIG-001) to drive the actual runtime
//! `prepare -> execute -> finalize` turn lifecycle.
//!
//! Ownership status (tracked by the MIG-001 sub-pack arc):
//!
//! - MIG-001-a — long-lived dependency surface expanded so the
//!   service can own runtime construction, the tool loop, stream
//!   emission, and finalize hooks. The deps struct carries
//!   `session_manager`, `project_manager`, `harness`,
//!   `learning_module`, `context_budget`, `memory_ticker`,
//!   `trajectory_manager`, and a Tauri `AppHandle` channel.
//! - MIG-001-b — owns the non-streaming turn lifecycle via
//!   [`TurnService::run_turn`] (see `run.rs`). The
//!   `run_agent_turn` IPC command collapsed into a thin adapter
//!   that parses args, delegates to `run_turn`, and returns.
//! - MIG-001-c — owns the streaming turn lifecycle via
//!   [`TurnService::stream_turn`] (see `stream.rs`). The
//!   `start_agent_stream` IPC command collapsed into a thin
//!   adapter that wires per-process cross-stream coordination
//!   state into [`stream::StreamTurnRequest`] and delegates.
//! - MIG-001-d — extracted the spawn-task closure body out of
//!   `stream.rs` into the sibling [`stream_task::run_stream_task`]
//!   free function. Captured state is bundled into
//!   [`stream_task::StreamTaskInputs`] so the `tokio::spawn(...)`
//!   call site is a single struct construction.
//! - MIG-001 follow-up cleanup (this commit) — extracted the
//!   ~460-LOC post-loop finalize block (guardrail rewrite +
//!   TaskOutcomeResolver projection + persisted-turn-outcome
//!   synthesis + timeline-message append + app-session save +
//!   trajectory record + after-turn dispatch + learning + emit
//!   stream_complete + harness TurnFinished + MemoryTicker
//!   on_turn_complete + diag log) out of `stream_task.rs` into
//!   the sibling [`stream_finalize::finalize_stream_task`] free
//!   function. `stream_task.rs` shrank from 1698 → 1249 LOC;
//!   `stream_finalize.rs` is a new ~640 LOC sibling.
//! - GAP-005 — extracted the inner SSE event-processing loop
//!   into [`stream_event_loop::run_stream_event_loop`], the per-iteration
//!   preflight/request builder into [`stream_preflight::build_iteration_request`],
//!   and the tool-batch execution loop into
//!   [`stream_tool_execution::execute_tool_batch`]. `stream_task.rs` now
//!   serves as the pure orchestrator under 800 LOC.
//!
//! Strict layering (CHARTER §2.1 hard constraint, also restated in
//! MIG-001 §4):
//!
//! - `application::turn_service` MUST NOT import from
//!   `crate::commands::*`. Every dependency is injected through
//!   [`TurnServiceDeps`] so the service stays decoupled from the
//!   `AppState` aggregate held by the IPC layer.
//!
//! Reference:
//! - [`docs/packs/feature/migration-core/MIG-001-canonical-chat-execution-spine.md`](../../../../../docs/packs/feature/migration-core/MIG-001-canonical-chat-execution-spine.md)

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Manager};

use crate::modules::harness::HarnessState;
use crate::modules::identity::{
    apply_identity_customization_pack, read_identity_customization_pack, resolve_identity,
    IdentityRegistry, SessionIdentityOverride,
};
use crate::modules::learning::trajectory::TrajectoryManager;
use crate::modules::learning::LearningModule;
use crate::modules::memory::retrieval::ActiveRetrievalManager;
use crate::modules::memory::MemoryTicker;
use crate::modules::memory::{PinnedStore, SharedMemoryProvider};
use crate::modules::projects::ProjectManager;
use crate::modules::runtime::budget::ContextBudget;
use crate::modules::runtime::config::ConfigLoader;
use crate::modules::session::SessionManager;
use crate::modules::tools::ToolRegistry;

mod agent_loop_delegate;
pub mod agentic_loop;
pub mod dk_lookup_hook;
pub mod finalize_hooks;
pub mod hook_registry;
mod loop_config;
pub mod preflight_hooks;
pub mod prompt_cache;
mod run;
mod stream;
mod stream_delegate;
mod stream_event_loop;
mod stream_finalize;
mod stream_iteration;
mod stream_loop_state;
mod stream_preflight;
pub mod stream_task;
mod stream_tool_execution;
mod todo_ledger;
mod work_loop;

#[cfg(test)]
mod tests;

pub use loop_config::AgenticLoopConfig;
pub use run::{RunTurnRequest, RunTurnResponse};
pub use stream::StreamTurnRequest;

use crate::modules::runtime::contracts::agent_loop::{SkillResolutionPlan, WorkLoopDecision};
use crate::modules::runtime::contracts::execution_mode::ExecutionModeDecision;

use super::memory_coordinator::{MemoryCoordinator, PrepareContextInput};
use super::memory_injection_service::{
    MemoryInjectionArtifacts, MemoryInjectionDeps, MemoryItemProjection,
};
use super::prompt_coordinator::{
    PromptAssemblyDecision, PromptCoordinator, PromptCoordinatorRequest,
};
use super::prompt_planner::{
    build_prompt_plan, BuildPromptPlanRequest, PromptPlanResult, PromptPlannerError,
};
use super::provider_service::{
    apply_complexity_model_routing, resolve_chat_runtime_provider, RuntimeProviderResolution,
};
use super::request_intelligence_service::{classify, RequestIntelligenceInput};

/// Long-lived dependencies the service holds on construction.
///
/// The first four fields (`tool_registry`, `pinned_store`,
/// `memory_provider`, `active_retrieval_manager`) feed the M1.1–M1.6
/// `prepare_chat_inputs` seam (provider + prompt + memory).
///
/// Visibility note: fields are `pub` (not `pub(crate)`) to match
/// the established `*Deps` precedent in this layer (see
/// [`crate::modules::application::memory_injection_service::MemoryInjectionDeps`])
/// and so external integration tests in `src-tauri/tests/**` can
/// construct a real [`TurnService`] without going through a
/// dedicated test-only constructor. The struct itself is the
/// intentional layering boundary; production callers outside the
/// IPC adapter should not be filling these fields by hand.
///
/// MIG-001-a/b expanded this struct so the service can own the
/// full chat turn lifecycle without the IPC adapter re-threading
/// `AppState` handles into every call:
///
/// - `session_manager` — restore + persist `AppSession`
/// - `project_manager` — feeds `SessionContextResolver` so a turn
///   resolves the same execution context the IPC layer used to
///   resolve in `commands/agent.rs`
/// - `harness` — emit `TurnStarted` / `TurnFinished` events
/// - `learning_module` — record per-turn outcomes + reflection
/// - `context_budget` — wire into `ConversationRuntime`
/// - `memory_ticker` — `TurnHook` for rolling summary + compile
/// - `trajectory_manager` — ShareGPT JSONL persistence after a turn
/// - `app_handle` — Tauri channel used by the after-turn dispatch
///   and (in MIG-001-c/d) the streaming emitter. `None` in unit
///   tests; the IPC adapter always passes `Some(handle)` in
///   production.
pub struct TurnServiceDeps {
    pub tool_registry: Arc<ToolRegistry>,
    pub pinned_store: Arc<dyn PinnedStore>,
    pub memory_provider: SharedMemoryProvider,
    pub active_retrieval_manager: Option<Arc<ActiveRetrievalManager>>,
    pub session_manager: Arc<SessionManager>,
    pub project_manager: Arc<ProjectManager>,
    pub harness: Option<Arc<HarnessState>>,
    pub learning_module: Option<Arc<tokio::sync::Mutex<LearningModule>>>,
    pub context_budget: ContextBudget,
    pub memory_ticker: Arc<MemoryTicker>,
    pub trajectory_manager: Option<Arc<TrajectoryManager>>,
    pub app_handle: Option<AppHandle>,
    /// MEM-MOD-P7 — cross-session learned-traits store (None when
    /// the SQLite-backed store is unavailable).  Read on every turn
    /// to materialise the `LearnedTraits` prompt block (priority 93).
    pub learned_traits: Option<crate::modules::memory::learned_traits::LearnedTraitsStore>,
    /// Rolling-summary orchestrator forwarded to `stream_finalize` so the
    /// auto-compact path can invoke `RollingSummarizer` without going
    /// through `AppState` (which is unavailable from inside the modules
    /// crate-half).
    pub rolling_summarizer: Arc<crate::modules::memory::summary::RollingSummarizer>,
    /// DW-002 (truth-loop iter-7) — process-wide utility LLM shim
    /// (`ChatProviderUtilityLlm` in production, `MockUtilityLlm` in
    /// tests).  The streaming turn forwards this into `StreamTaskInputs`
    /// so `stream_task::run_stream_task_body` can reuse one shared
    /// `Arc<dyn UtilityLlm>` for the WU-004 preflight digester instead
    /// of allocating a fresh wrapper per outer-loop iteration. Tests
    /// that build `TurnServiceDeps` by hand can pass
    /// `Arc::new(crate::modules::memory::MockUtilityLlm::empty())`.
    pub utility_llm: Arc<dyn crate::modules::memory::UtilityLlm>,
    /// Steward-aligned safety-valve configuration for the agent loop
    /// (S2-S1b). Defaults preserve current production behaviour and
    /// will gain a live consumer in S5 Task 5.1 when `run_agentic_loop`
    /// replaces the bespoke streaming loop. The field is wired through
    /// to `StreamTaskInputs::loop_config` today so future work only
    /// touches the loop body, not the dependency surface.
    pub loop_config: AgenticLoopConfig,
}

impl TurnServiceDeps {
    /// Build a [`MemoryInjectionDeps`] view from the same long-lived
    /// handles. The memory injection service owns its own deps
    /// struct so it stays usable independently of `TurnService`.
    fn memory_injection_deps(&self) -> MemoryInjectionDeps {
        MemoryInjectionDeps {
            pinned_store: self.pinned_store.clone(),
            memory_provider: self.memory_provider.clone(),
            active_retrieval_manager: self.active_retrieval_manager.clone(),
        }
    }
}

/// First-cut application service for one chat turn.
pub struct TurnService {
    pub(super) deps: TurnServiceDeps,
}

impl TurnService {
    /// Internal accessor for sibling modules in `turn_service::*`
    /// to read the long-lived dependency bundle without exposing
    /// it to outside callers.
    pub(super) fn deps(&self) -> &TurnServiceDeps {
        &self.deps
    }
}

impl TurnService {
    /// Construct a service from its long-lived dependencies.
    #[must_use]
    pub fn new(deps: TurnServiceDeps) -> Self {
        Self { deps }
    }

    /// Borrow the service-owned tool registry. The IPC adapter
    /// continues to need this for tool-loop wiring during the M1
    /// transition.
    #[must_use]
    pub fn tool_registry(&self) -> Arc<ToolRegistry> {
        self.deps.tool_registry.clone()
    }
}

/// Per-turn input bundle for [`TurnService::prepare_chat_inputs`].
pub struct PrepareChatInputsRequest {
    /// Working directory the turn runs against.
    pub workdir: PathBuf,
    /// Calendar date string (`%Y-%m-%d`) used by the prompt planner.
    pub current_date: String,
    /// `std::env::consts::OS`.
    pub os_name: String,
    /// `std::env::consts::FAMILY`.
    pub os_family: String,
    /// Optional session id, project id, workdir path string for
    /// memory scope resolution.
    pub session_id: Option<String>,
    pub project_id: Option<String>,
    pub workdir_str: Option<String>,
    /// User message for this turn — used as the memory retrieval
    /// query.
    pub user_message: String,
    /// Caller tag for tracing (`"run_agent_turn"` /
    /// `"start_agent_stream"`).
    pub caller: &'static str,
    /// Active skill ids for this turn (session-scoped); drives prompt block + trust attenuation.
    pub active_skill_ids: Vec<String>,
}

/// Composite output produced by [`TurnService::prepare_chat_inputs`].
pub struct PreparedChatInputs {
    /// Resolved provider client + model + timeout.
    pub provider: RuntimeProviderResolution,
    /// Structured prompt plan + rendered text.
    pub prompt: PromptPlanResult,
    /// Frontend memory items, ready to embed in
    /// `StreamTokenPayload.memory_context` on `stream_complete`.
    pub memory_items: Vec<MemoryItemProjection>,
    /// Full memory injection artefacts in case the IPC adapter
    /// needs the typed sections (e.g. for harness traces).
    pub memory_injection: MemoryInjectionArtifacts,
    /// Phase M1.6 — canonical request-intelligence decision for
    /// this turn. Currently advisory: `commands/agent.rs` logs it
    /// but the existing single execution path keeps running.
    /// M2 frontend projection will surface this to the chat-side
    /// explainer chip; M4 governance will consume it as a gate
    /// input.
    pub execution_mode_decision: ExecutionModeDecision,
    /// Internal work-loop route selected from the classifier decision.
    pub work_loop_decision: WorkLoopDecision,
    /// Deterministic skill-resolution plan for this turn.
    pub skill_resolution_plan: SkillResolutionPlan,
    /// FEAT-PCP-001 — explainable prompt control-plane decision for
    /// this turn.
    pub prompt_assembly_decision: PromptAssemblyDecision,
    /// Whether prompt diagnostics should be projected to the frontend
    /// for this turn.
    pub prompt_diagnostics_enabled: bool,
    /// Echo of active skill ids used for this prepared turn.
    pub active_skill_ids: Vec<String>,
}

/// Errors surfaced by [`TurnService`].
#[derive(Debug, thiserror::Error)]
pub enum TurnServiceError {
    #[error("{0}")]
    Provider(String),
    #[error(transparent)]
    Prompt(#[from] PromptPlannerError),
}

pub(super) fn build_prompt_plan_request_from_coordinator(
    session_id: String,
    user_message: String,
    workdir: PathBuf,
    current_date: String,
    os_name: String,
    os_family: String,
    registered_tool_names: Vec<String>,
    memory_injection: MemoryInjectionArtifacts,
    active_strategy_overlay: Option<String>,
    caller: &'static str,
    prompt_assembly_decision: PromptAssemblyDecision,
    coordinated_prompt: super::prompt_coordinator::CoordinatedPromptInputs,
    // MEM-MOD-P7 — pre-fetched active learned traits, ready to be
    // rendered as the `LearnedTraits` prompt block.  Empty vec when
    // the store is unavailable or holds nothing yet.
    learned_traits: Vec<crate::modules::memory::learned_traits::LearnedTrait>,
) -> BuildPromptPlanRequest {
    BuildPromptPlanRequest {
        session_id,
        user_message,
        workdir,
        current_date,
        os_name,
        os_family,
        registered_tool_names,
        memory_injection: Some(memory_injection),
        active_strategy_overlay,
        caller,
        mode: coordinated_prompt.mode,
        resolved_identity: coordinated_prompt.resolved_identity,
        scenario_profile: coordinated_prompt.scenario_profile,
        prompt_assembly_decision: Some(prompt_assembly_decision),
        active_skill_ids: coordinated_prompt.active_skill_ids,
        options: super::prompt_planner::PromptBuildOptions::default(),
        learned_traits,
    }
}

impl TurnService {
    /// Resolve provider + memory injection + prompt plan for one
    /// chat turn.
    ///
    /// Provider resolution happens before memory / prompt work so a
    /// misconfigured provider fails fast (preserves the legacy
    /// ordering in `commands/agent.rs`).
    pub async fn prepare_chat_inputs(
        &self,
        request: PrepareChatInputsRequest,
    ) -> Result<PreparedChatInputs, TurnServiceError> {
        // M1.6 — request intelligence (used for routing + future gates).
        let intelligence = classify(RequestIntelligenceInput {
            user_message: request.user_message.clone(),
            session_id: request.session_id.clone(),
            project_id: request.project_id.clone(),
            workdir: Some(request.workdir.clone()),
        });
        crate::modules::observability::emit(
            "request_classify",
            &format!(
                "mode={:?} risk={:?} complexity_level={:?} complexity_score={:.3}",
                intelligence.decision.execution_mode,
                intelligence.decision.risk_level,
                intelligence.decision.complexity_level,
                intelligence.decision.complexity_score
            ),
        );
        let restored_session_for_prepare = if let Some(session_id) = request.session_id.as_deref() {
            self.deps
                .session_manager
                .restore_session(session_id)
                .await
                .map_err(|error| {
                    tracing::warn!(
                        caller = request.caller,
                        session_id,
                        "[turn_service] failed to restore session for route context: {}",
                        error
                    );
                    error
                })
                .ok()
        } else {
            None
        };
        let mut work_loop_route_context = restored_session_for_prepare
            .as_ref()
            .map(|session| work_loop::route_context_from_messages(&session.messages))
            .unwrap_or_default();
        if let (Some(session_id), Some(app_handle)) =
            (request.session_id.as_deref(), self.deps.app_handle.as_ref())
        {
            if let Ok(app_data_dir) = app_handle.path().app_data_dir() {
                match crate::modules::runtime::history::read_session_history_event_page(
                    &app_data_dir,
                    session_id,
                    Some(500),
                    None,
                ) {
                    Ok(page) => work_loop::augment_route_context_from_run_log(
                        &mut work_loop_route_context,
                        &page.entries,
                    ),
                    Err(error) => tracing::debug!(
                        caller = request.caller,
                        session_id,
                        "[turn_service] durable route context replay unavailable: {}",
                        error
                    ),
                }
            }
        }

        let provider = resolve_chat_runtime_provider(&request.workdir)
            .await
            .map_err(TurnServiceError::Provider)?;
        let provider =
            apply_complexity_model_routing(provider, intelligence.decision.complexity_score);

        // M3.2 — per-turn memory orchestration now flows through
        // the canonical `MemoryCoordinator::prepare_context` seam.
        // The coordinator delegates to `memory_injection_service`
        // internally today (M3-A); M3.5 will route through the
        // upcoming `RecallAssembler` without changing this call
        // site.  TurnService itself no longer composes recall +
        // injection by hand.
        let coordinator = MemoryCoordinator::with_default_policy(self.deps.memory_injection_deps());
        let prepared_context = coordinator
            .prepare_context(PrepareContextInput {
                session_id: request.session_id.clone(),
                project_id: request.project_id.clone(),
                workdir: request.workdir_str.clone(),
                user_message: request.user_message.clone(),
                caller: request.caller,
            })
            .await;
        let memory_injection = prepared_context.artifacts;
        let memory_items = prepared_context.memory_items;

        let registered_tool_names = self.deps.tool_registry.tool_names();
        let work_loop_decision = work_loop::route_work_loop_with_context(
            &intelligence.decision,
            &request.user_message,
            &work_loop_route_context,
        );
        let mut skill_resolution_plan = work_loop::resolve_skill_plan(
            &request.workdir,
            &request.user_message,
            &request.active_skill_ids,
            &registered_tool_names,
        );
        let skill_prompt_contribution = work_loop::auto_load_trusted_skill_context(
            &request.workdir,
            &mut skill_resolution_plan,
        );
        let runtime_config = ConfigLoader::default_for(&request.workdir)
            .load()
            .unwrap_or_else(|error| {
                tracing::warn!(
                    caller = request.caller,
                    workdir = %request.workdir.display(),
                    "[turn_service] failed to load runtime config for identity resolution: {}",
                    error
                );
                crate::modules::runtime::config::RuntimeConfig::empty()
            });
        let session_identity_override = if request.session_id.is_some() {
            restored_session_for_prepare.as_ref().and_then(|session| {
                (session.soul_id.is_some() || session.persona_id.is_some()).then_some(
                    SessionIdentityOverride {
                        soul_id: session.soul_id.clone(),
                        persona_id: session.persona_id.clone(),
                    },
                )
            })
        } else {
            None
        };
        let builtin_identity_registry = IdentityRegistry::builtin();
        let identity_pack = read_identity_customization_pack().unwrap_or_else(|error| {
            tracing::warn!(
                caller = request.caller,
                "[turn_service] failed to read identity customization pack: {}",
                error
            );
            Default::default()
        });
        let effective_identity_registry =
            apply_identity_customization_pack(&builtin_identity_registry, &identity_pack);
        let identity_resolution = resolve_identity(
            &effective_identity_registry,
            runtime_config.identity(),
            session_identity_override.as_ref(),
        );
        let session_soul_id = session_identity_override
            .as_ref()
            .and_then(|identity| identity.soul_id.as_deref());
        let session_persona_id = session_identity_override
            .as_ref()
            .and_then(|identity| identity.persona_id.as_deref());
        tracing::info!(
            caller = request.caller,
            session_id = ?request.session_id,
            session_soul_id = ?session_soul_id,
            session_persona_id = ?session_persona_id,
            resolved_soul_id = %identity_resolution.resolved.soul_id,
            resolved_persona_id = ?identity_resolution.resolved.persona_id,
            resolved_source = ?identity_resolution.resolved.source,
            "[turn_service] resolved identity for turn"
        );
        for warning in &identity_resolution.warnings {
            tracing::warn!(
                caller = request.caller,
                "[turn_service] identity resolution warning: {}",
                warning
            );
        }

        // Phase M5 closeout — resolve any currently-Active
        // candidate strategy overlay so the prompt planner can
        // append it as a typed `ActiveStrategyOverlay` block.
        // The resolver walks the registry on each turn; this is
        // the single production hookpoint for the M5 active
        // flip.  Failure to resolve is non-fatal: an empty
        // overlay is rendered.
        let active_strategy_overlay = {
            let resolver =
                crate::modules::learning::ActiveStrategyOverlayResolver::with_default_root();
            let overlay = resolver.resolve().await;
            let text = overlay.render_prompt_block();
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        };
        let prompt_coordinator = PromptCoordinator;
        let coordinated_prompt = prompt_coordinator.coordinate(PromptCoordinatorRequest {
            resolved_identity: Some(identity_resolution.resolved),
            scenario_profile: intelligence.decision.scenario_profile_hint,
            default_scenario_profile: runtime_config.control_plane().default_scenario_profile(),
            execution_mode_decision: Some(intelligence.decision.clone()),
            registered_tool_names: registered_tool_names.clone(),
            memory_injection_present: !memory_injection.prompt_sections.is_empty(),
            active_strategy_overlay_present: active_strategy_overlay.is_some(),
            active_skill_ids: request.active_skill_ids.clone(),
        });

        // MEM-MOD-P7 — fetch the active learned-traits slice off the
        // hot path.  When the store is unavailable we degrade to an
        // empty vec; the planner will then skip the LearnedTraits
        // block entirely (no empty header).
        let learned_traits: Vec<crate::modules::memory::learned_traits::LearnedTrait> =
            match self.deps.learned_traits.as_ref() {
                Some(store) => {
                    let store = store.clone();
                    tokio::task::spawn_blocking(move || store.list_active(8))
                        .await
                        .ok()
                        .and_then(|r| r.ok())
                        .unwrap_or_default()
                }
                None => Vec::new(),
            };

        let mut external_contributions = coordinated_prompt
            .coordinated_inputs
            .external_contributions
            .clone();
        if let Some(contribution) =
            work_loop::memory_recall_prompt_contribution(&work_loop_decision)
        {
            external_contributions.push(contribution);
        }
        if let Some(contribution) =
            work_loop::tool_required_prompt_contribution(&work_loop_decision)
        {
            external_contributions.push(contribution);
        }
        if let Some(contribution) =
            work_loop::single_shell_command_prompt_contribution(&work_loop_decision)
        {
            external_contributions.push(contribution);
        }
        if let Some(contribution) = work_loop::continuation_context_prompt_contribution(
            &work_loop_decision,
            &work_loop_route_context,
        ) {
            external_contributions.push(contribution);
        }
        if let Some(contribution) =
            todo_ledger::prompt_contribution(&work_loop_decision, request.session_id.as_deref())
        {
            external_contributions.push(contribution);
        }
        if let Some(contribution) = skill_prompt_contribution {
            external_contributions.push(contribution);
        }

        let planner_request = build_prompt_plan_request_from_coordinator(
            request
                .session_id
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            request.user_message.clone(),
            request.workdir,
            request.current_date,
            request.os_name,
            request.os_family,
            registered_tool_names,
            memory_injection.clone(),
            active_strategy_overlay,
            request.caller,
            coordinated_prompt.decision.clone(),
            coordinated_prompt.coordinated_inputs.clone(),
            learned_traits,
        );
        let mut prompt = build_prompt_plan(planner_request, external_contributions).await?;

        // Inject a tiny "runtime model" hint so the LLM answers
        // "你是什么模型 / what model are you" with the actual provider +
        // model identifier instead of a generic boilerplate. Both the
        // structured block list (consumed by run.rs) and the flattened
        // text (consumed by stream.rs) get the same line — we
        // re-render the text from blocks to keep them in lockstep.
        {
            use super::prompt_planner::{PromptBlock, PromptBlockKind, PromptBlockSource};
            let provider_id = provider.provider_id.clone();
            let model_id = provider.model.clone();
            let provider_display = crate::modules::provider::known_providers::find(&provider_id)
                .map(|p| p.display_name.to_string())
                .unwrap_or_else(|| provider_id.clone());
            let qualified = format!("{provider_display} / {model_id}");
            let line = format!(
                "[runtime_model] You are currently running on `{qualified}` \
                 (provider_id=`{provider_id}`, model_id=`{model_id}`). \
                 If the user asks which model you are (e.g. \"你是什么模型\", \
                 \"what model are you\"), answer truthfully with this exact \
                 provider + model name."
            );
            prompt.plan.blocks.push(PromptBlock {
                id: "runtime_model".into(),
                kind: PromptBlockKind::System,
                title: "Runtime Model".into(),
                content: line,
                source: PromptBlockSource {
                    subsystem: "runtime".into(),
                    reference: Some(qualified),
                },
                priority: 90,
                is_sensitive: false,
            });
            prompt.text = prompt.plan.join_into_text();
        }

        if !skill_resolution_plan.candidates.is_empty()
            || !skill_resolution_plan.active_skill_ids.is_empty()
            || skill_resolution_plan.should_load_find_skills
        {
            use super::prompt_planner::{PromptBlock, PromptBlockKind, PromptBlockSource};
            let candidates = skill_resolution_plan
                .candidates
                .iter()
                .map(|candidate| {
                    let blocked = candidate
                        .blocked_reason
                        .as_deref()
                        .map(|reason| format!(" blocked_reason={reason}"))
                        .unwrap_or_default();
                    let warning = candidate
                        .load_warning
                        .as_deref()
                        .map(|warning| format!(" load_warning={warning}"))
                        .unwrap_or_default();
                    format!(
                        "- {} [{}]: {} loaded={} auto_load_allowed={}{}{}",
                        candidate.name,
                        candidate.source,
                        candidate.reason,
                        candidate.loaded,
                        candidate.auto_load_allowed,
                        blocked,
                        warning
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            let active = if skill_resolution_plan.active_skill_ids.is_empty() {
                "none".to_string()
            } else {
                skill_resolution_plan.active_skill_ids.join(", ")
            };
            let body = format!(
                "[skill_resolution]\nactive_skill_ids: {active}\nremote_install_policy: {}\nauto_discovery_tools: {}\nloaded_skills: {}\nblocked_skills: {}\nload_warnings: {}\n{}\nUse skill_view only for approved local/builtin/reviewed skills. Do not install remote skills without user approval.",
                skill_resolution_plan.remote_install_policy,
                skill_resolution_plan.auto_discovery_tools.join(", "),
                skill_resolution_plan.loaded_skill_names.join(", "),
                skill_resolution_plan.blocked_skill_names.join(", "),
                skill_resolution_plan.load_warnings.join(" | "),
                candidates
            );
            prompt.plan.blocks.push(PromptBlock {
                id: "skill_resolution_plan".into(),
                kind: PromptBlockKind::Skill,
                title: "Skill Resolution Plan".into(),
                content: body,
                source: PromptBlockSource {
                    subsystem: "skills".into(),
                    reference: Some("turn_service.work_loop".into()),
                },
                priority: 88,
                is_sensitive: false,
            });
            prompt.text = prompt.plan.join_into_text();
        }

        Ok(PreparedChatInputs {
            provider,
            prompt,
            memory_items,
            memory_injection,
            execution_mode_decision: intelligence.decision,
            work_loop_decision,
            skill_resolution_plan,
            prompt_assembly_decision: coordinated_prompt.decision,
            prompt_diagnostics_enabled: runtime_config.control_plane().prompt_diagnostics_enabled(),
            active_skill_ids: coordinated_prompt
                .coordinated_inputs
                .active_skill_ids
                .clone(),
        })
    }
}

#[cfg(test)]
mod mod_tests {
    use super::*;

    /// MIG-001-a smoke test: verify the expanded `TurnServiceDeps`
    /// surface holds all eleven fields with the trait bounds the
    /// later sub-packs will rely on.
    ///
    /// MIG-001-c/d will spawn the canonical turn into a
    /// `tokio::spawn` task closure, so every dependency reachable
    /// from the moved closure must be `Send + Sync`. This compile-
    /// time assertion guards that invariant before runtime
    /// migration begins, so any future field added to the struct
    /// that breaks `Send + Sync` is rejected at this seam rather
    /// than deep inside the streaming path.
    #[test]
    fn turn_service_deps_construction_smoke() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TurnServiceDeps>();
        assert_send_sync::<TurnService>();
        assert_send_sync::<Arc<TurnService>>();
    }
}
