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
//! - MIG-001-a (this commit) — long-lived dependency surface
//!   expanded so the service can own runtime construction, the tool
//!   loop, stream emission, and finalize hooks. The deps struct now
//!   carries `session_manager`, `harness`, `learning_module`,
//!   `context_budget`, `memory_ticker`, `trajectory_manager`, and a
//!   Tauri `AppHandle` channel; the IPC adapter no longer needs to
//!   re-thread these through every call site.
//! - MIG-001-b/c/d — owns turn lifecycle via `run_turn` and
//!   `stream_turn`; the IPC layer collapses to a thin adapter that
//!   parses arguments and delegates.
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

use tauri::AppHandle;

use crate::modules::harness::HarnessState;
use crate::modules::learning::trajectory::TrajectoryManager;
use crate::modules::learning::LearningModule;
use crate::modules::memory::retrieval::ActiveRetrievalManager;
use crate::modules::memory::MemoryTicker;
use crate::modules::memory::{PinnedStore, SharedMemoryProvider};
use crate::modules::runtime::budget::ContextBudget;
use crate::modules::session::SessionManager;
use crate::modules::tools::ToolRegistry;

use crate::modules::runtime::contracts::execution_mode::ExecutionModeDecision;

use super::memory_coordinator::{MemoryCoordinator, PrepareContextInput};
use super::memory_injection_service::{
    MemoryInjectionArtifacts, MemoryInjectionDeps, MemoryItemProjection,
};
use super::prompt_planner::{
    build_prompt_plan, BuildPromptPlanRequest, PromptPlanResult, PromptPlannerError,
};
use super::provider_service::{resolve_chat_runtime_provider, RuntimeProviderResolution};
use super::request_intelligence_service::{classify, RequestIntelligenceInput};

/// Long-lived dependencies the service holds on construction.
///
/// The first four fields (`tool_registry`, `pinned_store`,
/// `memory_provider`, `active_retrieval_manager`) feed the M1.1–M1.6
/// `prepare_chat_inputs` seam (provider + prompt + memory).
///
/// MIG-001-a expanded this struct with seven new fields so the
/// service can own the full chat turn lifecycle in MIG-001-b/c/d
/// without the IPC adapter re-threading `AppState` handles into
/// every call:
///
/// - `session_manager` — restore + persist `AppSession`
/// - `harness` — emit `TurnStarted` / `TurnFinished` events
/// - `learning_module` — record per-turn outcomes + reflection
/// - `context_budget` — wire into `ConversationRuntime`
/// - `memory_ticker` — `TurnHook` for rolling summary + compile
/// - `trajectory_manager` — ShareGPT JSONL persistence after a turn
/// - `app_handle` — Tauri channel used by the streaming path to
///   construct the `AgentStreamEmitter`. `None` in unit tests; the
///   IPC adapter always passes `Some(handle)` in production.
pub struct TurnServiceDeps {
    pub tool_registry: Arc<ToolRegistry>,
    pub pinned_store: Arc<dyn PinnedStore>,
    pub memory_provider: SharedMemoryProvider,
    pub active_retrieval_manager: Option<Arc<ActiveRetrievalManager>>,
    pub session_manager: Arc<SessionManager>,
    pub harness: Option<Arc<HarnessState>>,
    pub learning_module: Option<Arc<tokio::sync::Mutex<LearningModule>>>,
    pub context_budget: ContextBudget,
    pub memory_ticker: Arc<MemoryTicker>,
    pub trajectory_manager: Option<Arc<TrajectoryManager>>,
    pub app_handle: Option<AppHandle>,
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
    deps: TurnServiceDeps,
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
}

/// Errors surfaced by [`TurnService`].
#[derive(Debug, thiserror::Error)]
pub enum TurnServiceError {
    #[error("{0}")]
    Provider(String),
    #[error(transparent)]
    Prompt(#[from] PromptPlannerError),
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
        let provider = resolve_chat_runtime_provider(&request.workdir)
            .await
            .map_err(TurnServiceError::Provider)?;

        // M1.6 — request intelligence runs first so future slices
        // can short-circuit memory + prompt work for
        // `specialized_surface` / denied modes. Today the decision
        // is advisory only.
        let intelligence = classify(RequestIntelligenceInput {
            user_message: request.user_message.clone(),
            session_id: request.session_id.clone(),
            project_id: request.project_id.clone(),
            workdir: Some(request.workdir.clone()),
        });

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

        let prompt = build_prompt_plan(BuildPromptPlanRequest {
            workdir: request.workdir,
            current_date: request.current_date,
            os_name: request.os_name,
            os_family: request.os_family,
            registered_tool_names,
            memory_injection: Some(memory_injection.clone()),
            active_strategy_overlay,
            caller: request.caller,
        })
        .await?;

        Ok(PreparedChatInputs {
            provider,
            prompt,
            memory_items,
            memory_injection,
            execution_mode_decision: intelligence.decision,
        })
    }
}

#[cfg(test)]
mod tests {
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
