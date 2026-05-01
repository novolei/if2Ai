//! MIG-001-b — non-streaming chat turn lifecycle owned by
//! [`crate::modules::application::turn_service::TurnService`].
//!
//! This module hosts the canonical implementation of
//! [`TurnService::run_turn`], which used to live as the body of the
//! `commands::agent::run_agent_turn` IPC command. The IPC command
//! is now a thin adapter that parses the Tauri arguments and
//! delegates to this method.
//!
//! The method owns:
//!
//! 1. Session restoration via the injected `session_manager`.
//! 2. Per-turn execution-context resolution via
//!    `control_plane::SessionContextResolver`.
//! 3. Provider + memory + prompt assembly via
//!    [`TurnService::prepare_chat_inputs`].
//! 4. Runtime construction (`ConversationRuntime`) wired with the
//!    injected `context_budget`, `memory_ticker`, and the resolved
//!    permission policy / tool executor.
//! 5. Synchronous `runtime.run_turn(...)` execution.
//! 6. Post-turn finalize: skill-proposal handling, compaction,
//!    session save, trajectory recording, after-turn memory
//!    dispatch, learning-module update, frozen-snapshot
//!    verification, working-memory budget audit, Weibull decay,
//!    promotion engine scan, and harness `TurnFinished` emission.
//!
//! All cross-module imports go through `crate::modules::*` per
//! CHARTER §3.1; nothing in this module reaches into
//! `crate::commands::*`.

use serde::Serialize;

use super::TurnService;
use crate::modules::application::memory_candidate_extractor::{
    extract_memory_store_tool_candidates, lookup_existing_records_for_candidates,
};
use crate::modules::application::memory_injection_service::MemoryInjectionDeps;
use crate::modules::application::permission_service::{
    build_permission_policy, parse_permission_mode,
};
use crate::modules::application::real_api_client::RealApiClient;
use crate::modules::application::stream_emitter_service::dispatch_after_turn;
use crate::modules::application::tool_executor::ToolRegistryExecutor;
use crate::modules::application::tool_heuristics::extract_skill_proposal_name;
use crate::modules::application::trajectory_service::record_trajectory_if_possible;
use crate::modules::application::turn_service::{
    PrepareChatInputsRequest, RuntimeProviderResolution, TurnServiceError,
};
use crate::modules::control_plane::session_bridge::{
    app_session_to_runtime, log_context_fingerprint,
};
use crate::modules::control_plane::SessionContextResolver;
use crate::modules::learning::reflection::ReflectionEngine;
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::working_memory::WorkingMemory;
use crate::modules::runtime::compact::{compact_session, should_compact, CompactionConfig};
use crate::modules::runtime::contracts::common::CorrelationIds;
use crate::modules::runtime::conversation::{ConversationRuntime, RuntimeError};
use crate::modules::runtime::episodic_compaction::WeibullDecay;
use crate::modules::runtime::event_log::RunEventLogger;
use crate::modules::runtime::session::{ContentBlock, ConversationMessage};
use crate::modules::runtime::snapshot::FrozenSnapshot;

/// Per-turn input bundle for [`TurnService::run_turn`].
///
/// Mirrors the original Tauri command parameters one-for-one so the
/// IPC adapter is a pure rename.
pub struct RunTurnRequest {
    /// Session id to restore + persist against.
    pub session_id: String,
    /// User message text driving this turn.
    pub user_message: String,
    /// Permission mode token (`"plan"` / `"acceptEdits"` /
    /// `"bypassPermissions"` / `"dangerFullAccess"` / `None` for
    /// the default).
    pub permission_mode: Option<String>,
}

/// Response surface returned to the IPC caller. Identical shape to
/// the legacy `commands::agent::RunAgentTurnResponse`; the IPC
/// command now re-exports this type via a `pub use`.
#[derive(Serialize)]
pub struct RunTurnResponse {
    /// The generated assistant message text.
    pub message: String,
    /// Echoed session id so the caller can correlate.
    pub session_id: String,
    /// Optional model thinking content joined with `"\n\n"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
}

impl TurnService {
    /// Drive one non-streaming chat turn end to end.
    ///
    /// MIG-001-b — owns the full `prepare -> execute -> finalize`
    /// lifecycle that used to live inside
    /// `commands::agent::run_agent_turn`. The IPC command is now a
    /// thin adapter that constructs a [`TurnService`] from
    /// `AppState` and calls this method.
    ///
    /// Errors are returned as user-facing strings matching the
    /// legacy IPC contract (network / permission / timeout
    /// classification preserved verbatim).
    pub async fn run_turn(&self, request: RunTurnRequest) -> Result<RunTurnResponse, String> {
        let RunTurnRequest {
            session_id,
            user_message,
            permission_mode,
        } = request;
        let run_id = uuid::Uuid::new_v4().to_string();
        let run_event_logger = self
            .deps
            .app_handle
            .as_ref()
            .map(|handle| {
                RunEventLogger::for_app_handle(handle, session_id.clone(), run_id.clone())
            })
            .unwrap_or_else(|| RunEventLogger::disabled(session_id.clone(), run_id.clone()));

        let log_correlation = CorrelationIds {
            session_id: Some(session_id.clone()),
            run_id: Some(run_id.clone()),
            ..Default::default()
        };

        tracing::info!(
            "[run_agent_turn] Starting - session_id: {}, run_id: {}, message: {}",
            session_id,
            run_id,
            user_message
        );
        let _ = run_event_logger
            .append_with_correlation(
                "run_started",
                serde_json::json!({
                    "caller": "run_agent_turn",
                    "permission_mode": permission_mode.clone(),
                    "message_preview": user_message.chars().take(160).collect::<String>(),
                }),
                Some(&log_correlation),
            )
            .await;

        if let Some(warn) = crate::modules::security::safety::shared_safety_layer()
            .scan_inbound_for_secrets(&user_message)
        {
            let _ = run_event_logger
                .append_with_correlation(
                    "run_error",
                    serde_json::json!({
                        "stage": "inbound_secret_scan",
                        "error": warn,
                    }),
                    Some(&log_correlation),
                )
                .await;
            return Err(warn);
        }

        // Restore the session
        let app_session = match self.deps.session_manager.restore_session(&session_id).await {
            Ok(session) => session,
            Err(error) => {
                let message = error.to_string();
                let _ = run_event_logger
                    .append_with_correlation(
                        "run_error",
                        serde_json::json!({
                            "stage": "restore_session",
                            "error": message,
                        }),
                        Some(&log_correlation),
                    )
                    .await;
                return Err(message);
            }
        };

        tracing::info!(
            "[run_agent_turn] Session restored, {} messages",
            app_session.messages.len()
        );

        // Phase 6E harness: emit TurnStarted onto the EventBus.
        let harness_event_bus_run = self.deps.harness.as_ref().map(|h| h.event_bus.clone());
        let turn_number_run = (app_session.messages.len() as u64) + 1;
        // Phase M4.1 baseline message-vector slice.
        let baseline_message_count_run = app_session.messages.len();
        crate::modules::harness::agent_loop_integration::emit_turn_started(
            harness_event_bus_run.as_ref(),
            &session_id,
            turn_number_run,
        );
        let turn_started_at_run = std::time::Instant::now();

        let mode = parse_permission_mode(permission_mode.as_deref());

        // Per-turn execution context (replaces
        // commands::agent::resolve_session_execution_context which
        // closed over `&AppState`).
        let resolver = SessionContextResolver::new(
            self.deps.session_manager.clone(),
            self.deps.project_manager.clone(),
        );
        let execution_context = resolver
            .resolve_from_session(&app_session, mode, "run_agent_turn")
            .await;
        let proposal_workdir = execution_context.workdir.clone();
        log_context_fingerprint("run_agent_turn", &execution_context);

        let runtime_session = app_session_to_runtime(&app_session);

        // Phase M1.4 — provider + memory + prompt in one await.
        let project_id_opt: Option<String> = if execution_context.project_id.is_empty() {
            None
        } else {
            Some(execution_context.project_id.clone())
        };
        let prepared = match self
            .prepare_chat_inputs(PrepareChatInputsRequest {
                workdir: execution_context.workdir.clone(),
                current_date: crate::modules::runtime::logical_day::get_today().display,
                os_name: std::env::consts::OS.to_string(),
                os_family: std::env::consts::FAMILY.to_string(),
                session_id: Some(execution_context.session_id.clone()),
                project_id: project_id_opt.clone(),
                workdir_str: execution_context.workdir.to_str().map(str::to_string),
                user_message: user_message.clone(),
                caller: "run_agent_turn",
                active_skill_ids: app_session.active_skill_ids.clone(),
            })
            .await
        {
            Ok(prepared) => prepared,
            Err(err) => {
                let message = match err {
                    TurnServiceError::Provider(msg) => {
                        tracing::error!("[run_agent_turn] Failed to create API client: {}", msg);
                        format!("Failed to connect to AI service: {msg}")
                    }
                    TurnServiceError::Prompt(p) => {
                        tracing::error!("[run_agent_turn] Prompt planning failed: {}", p);
                        p.to_string()
                    }
                };
                let _ = run_event_logger
                    .append_with_correlation(
                        "run_error",
                        serde_json::json!({
                            "stage": "prepare_chat_inputs",
                            "error": message,
                        }),
                        Some(&log_correlation),
                    )
                    .await;
                return Err(message);
            }
        };
        // MIG-002-a — request intelligence now acts as a real route gate.
        // Short-circuit for SpecializedSurface mode.
        use crate::modules::runtime::contracts::execution_mode::ExecutionMode;
        match prepared.execution_mode_decision.execution_mode {
            ExecutionMode::SpecializedSurface => {
                tracing::info!(
                    execution_mode = ?prepared.execution_mode_decision.execution_mode,
                    route_hint = ?prepared.execution_mode_decision.route_hint,
                    "[run_agent_turn] Short-circuiting: SpecializedSurface mode"
                );
                let routed_message = format!(
                    "Request routed to specialized surface: {:?}",
                    prepared.execution_mode_decision.route_hint
                );
                let _ = run_event_logger
                    .append_with_correlation(
                        "run_error",
                        serde_json::json!({
                            "stage": "execution_mode_gate",
                            "error": routed_message.clone(),
                        }),
                        Some(&log_correlation),
                    )
                    .await;
                crate::modules::harness::agent_loop_integration::emit_turn_finished(
                    harness_event_bus_run.as_ref(),
                    &session_id,
                    turn_number_run,
                    false,
                    0,
                    turn_started_at_run.elapsed().as_millis() as u64,
                );
                return Err(routed_message);
            }
            ExecutionMode::DirectExecute
            | ExecutionMode::AutoPlanExecute
            | ExecutionMode::PlanThenConfirm => {
                // Continue with normal execution path
                tracing::info!(
                    execution_mode = ?prepared.execution_mode_decision.execution_mode,
                    risk_level = ?prepared.execution_mode_decision.risk_level,
                    complexity_level = ?prepared.execution_mode_decision.complexity_level,
                    policy_version = %prepared.execution_mode_decision.classifier_policy_version,
                    rules = ?prepared.execution_mode_decision.classifier_matched_rule_ids,
                    "[run_agent_turn] request_intelligence decision (enforced route gate)"
                );
            }
        }
        let RuntimeProviderResolution {
            provider_client,
            provider_id: _provider_id,
            model,
            context_window: _context_window,
            request_timeout,
        } = prepared.provider;
        tracing::info!("[run_agent_turn] API client created, model: {}", model);
        let api_client = RealApiClient::new(
            provider_client,
            model,
            request_timeout,
            self.deps.tool_registry.clone(),
        );

        let permission_policy = build_permission_policy(mode);
        tracing::info!(
            "[run_agent_turn] effective permission mode: {}",
            mode.as_str()
        );

        let tool_allowlist = crate::modules::skills::attenuation::tool_allowlist_for_active_skills(
            &prepared.active_skill_ids,
        );
        let tool_executor = ToolRegistryExecutor::new_with_context(
            self.deps.tool_registry.clone(),
            execution_context,
        )
        .with_definition_allowlist(tool_allowlist);

        // Phase M1.3 — prompt vec + joined text from the structured plan.
        let system_prompt: Vec<String> = prepared
            .prompt
            .plan
            .blocks
            .iter()
            .map(|b| b.content.clone())
            .collect();
        let system_prompt_text = prepared.prompt.text;
        let frozen_snapshot = FrozenSnapshot::capture(&system_prompt_text);
        tracing::info!(
            "[run_agent_turn] Frozen snapshot captured, prompt hash={}, estimate={} tokens",
            frozen_snapshot.prompt_hash,
            frozen_snapshot.token_estimate()
        );

        // Phase 8B.11 fix — wire MemoryTicker as TurnHook + supply
        // session/project context so RollingSummarizer + compile_today
        // actually fire on each turn.
        let session_ctx_id = tool_executor.execution_context.session_id.clone();
        let session_ctx_project = if tool_executor.execution_context.project_id.is_empty() {
            None
        } else {
            Some(tool_executor.execution_context.project_id.clone())
        };

        self.deps
            .session_manager
            .push_conversation_undo_checkpoint(&session_id, &app_session);

        let mut runtime = ConversationRuntime::new(
            runtime_session,
            api_client,
            tool_executor,
            permission_policy,
            system_prompt,
        )
        .with_context_budget(self.deps.context_budget.clone())
        // MEM-MOD-P2 — wire WorkingMemory's max_tokens to the user-tunable
        // ContextBudget instead of the hard-coded 1600 default.  `max_turns`
        // stays at the historical 8 because turn-count eviction has not had
        // a budget knob yet (a future Pack can add one).
        .with_working_memory(WorkingMemory::new(
            8,
            self.deps.context_budget.working_tokens(),
        ))
        .with_turn_hook(self.deps.memory_ticker.clone())
        .with_session_context(session_ctx_id.clone(), session_ctx_project.clone());

        tracing::info!(
            "[run_agent_turn] Runtime created, calling run_turn with message: {}",
            user_message
        );

        let cost_cfg_run = crate::modules::runtime::cost_guard::CostGuardConfig::from_env();
        if let Err(ce) = crate::modules::runtime::cost_guard::CostGuard::check_before_llm_call(
            &cost_cfg_run,
            &session_id,
        ) {
            return Err(ce.to_string());
        }

        let result = runtime.run_turn(user_message.clone(), None).await;

        tracing::info!(
            "[run_agent_turn] run_turn completed, result: {:?}",
            result.is_ok()
        );

        match result {
            Ok(summary) => {
                let sync_turn_ms = turn_started_at_run.elapsed().as_millis() as u64;
                crate::modules::learning::estimation::record_turn_duration_ms(sync_turn_ms);
                crate::modules::observability::emit(
                    "run_turn_finished",
                    &format!(
                        "session_id={} turn={} ms={} iterations={}",
                        session_id, turn_number_run, sync_turn_ms, summary.iterations
                    ),
                );
                crate::modules::application::job_monitor::publish_line(
                    &session_id,
                    format!(
                        "run turn {} finished: {}ms iterations={}",
                        turn_number_run, sync_turn_ms, summary.iterations
                    ),
                );
                for _ in 0..summary.iterations.max(1) {
                    crate::modules::runtime::cost_guard::CostGuard::record_llm_call_charged(
                        &cost_cfg_run,
                        &session_id,
                    );
                }
                let response_text = summary
                    .assistant_messages
                    .iter()
                    .filter_map(|msg| {
                        msg.blocks.iter().find_map(|block| {
                            if let ContentBlock::Text { text } = block {
                                Some(text.clone())
                            } else {
                                None
                            }
                        })
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                let thinking_content: Option<String> = {
                    let collected: Vec<String> = summary
                        .assistant_messages
                        .iter()
                        .filter_map(|msg| msg.thinking.clone())
                        .collect();
                    let joined = collected.join("\n\n");
                    if joined.is_empty() {
                        None
                    } else {
                        Some(joined)
                    }
                };

                let final_text = if response_text.is_empty() {
                    "Agent completed the request.".to_string()
                } else {
                    response_text
                };
                let final_text = if let Some(proposal_name) =
                    extract_skill_proposal_name(&final_text)
                {
                    match crate::modules::tools::builtin::skill::create_agent_skill_proposal_draft(
                        &proposal_workdir,
                        &proposal_name,
                        &final_text,
                    ) {
                        Ok(path) => format!(
                            "{final_text}\n\n[skill_proposal] draft created at {} (requires approval)",
                            path.display()
                        ),
                        Err(err) => {
                            format!("{final_text}\n\n[skill_proposal] draft create failed: {err}")
                        }
                    }
                } else {
                    final_text
                };

                let updated_runtime_session = runtime.into_session();
                let next_message_count = app_session.logical_message_count()
                    + updated_runtime_session
                        .messages
                        .len()
                        .saturating_sub(app_session.messages.len());

                let trajectory_session = updated_runtime_session.clone();

                let compaction_config = CompactionConfig::default();
                let pre_compact_message_count = updated_runtime_session.messages.len();
                let final_runtime_session =
                    if should_compact(&updated_runtime_session, compaction_config) {
                        let compact_result =
                            compact_session(&updated_runtime_session, compaction_config);
                        compact_result.compacted_session
                    } else {
                        updated_runtime_session
                    };
                let post_compact_message_count = final_runtime_session.messages.len();

                let mut updated_app_session = app_session;
                updated_app_session.messages = final_runtime_session.messages;
                updated_app_session.message_count = next_message_count;

                self.deps
                    .session_manager
                    .save_session(&updated_app_session)
                    .await
                    .map_err(|e| e.to_string())?;

                // P1-7 — conversation recall index (SQLite FTS; best-effort).
                {
                    let turn_id = format!("{}:{}", session_id, next_message_count);
                    if let Err(e) = self
                        .deps
                        .memory_provider
                        .conversation_recall_ingest(
                            &session_id,
                            session_ctx_project.as_deref(),
                            &turn_id,
                            &user_message,
                            &final_text.chars().take(12_000).collect::<String>(),
                        )
                        .await
                    {
                        tracing::debug!(error = %e, "[conversation_recall] ingest skipped");
                    }
                }

                record_trajectory_if_possible(
                    &trajectory_session,
                    std::slice::from_ref(&system_prompt_text),
                    self.deps.trajectory_manager.as_ref(),
                )
                .await;

                // Phase M4.1 — extract real `MemoryWriteCandidate`s
                // from the assistant `memory_store` tool calls
                // produced during THIS turn.
                let new_messages_run: Vec<ConversationMessage> = updated_app_session
                    .messages
                    .iter()
                    .skip(baseline_message_count_run)
                    .cloned()
                    .collect();
                let after_turn_scope_run = MemoryExecutionScope {
                    session_id: Some(session_ctx_id.clone()),
                    project_id: session_ctx_project.clone(),
                    workdir: None,
                };
                let candidates_run =
                    extract_memory_store_tool_candidates(&new_messages_run, &after_turn_scope_run);
                let existing_run = lookup_existing_records_for_candidates(
                    &self.deps.memory_provider,
                    &after_turn_scope_run,
                    &candidates_run,
                )
                .await;
                if let Some(handle) = self.deps.app_handle.as_ref() {
                    dispatch_after_turn(
                        handle,
                        harness_event_bus_run.as_ref(),
                        MemoryInjectionDeps {
                            pinned_store: self.deps.pinned_store.clone(),
                            memory_provider: self.deps.memory_provider.clone(),
                            active_retrieval_manager: self.deps.active_retrieval_manager.clone(),
                        },
                        Some(session_ctx_id.clone()),
                        session_ctx_project.clone(),
                        candidates_run,
                        existing_run,
                        Vec::new(),
                        "run_agent_turn",
                    )
                    .await;
                } else {
                    tracing::debug!(
                        "[run_agent_turn] dispatch_after_turn skipped (no AppHandle in deps)"
                    );
                }

                // LearningModule: record turn outcome.
                if let Some(lm_arc) = &self.deps.learning_module {
                    let mut lm = lm_arc.lock().await;
                    lm.self_model_mut()
                        .record_turn(/* success= */ true, /* response_time_ms= */ 0.0);

                    let turn_count = lm.self_model().performance.total_turns;
                    tracing::info!(
                        "[run_agent_turn] LearningModule: turn {} recorded, {} patterns tracked",
                        turn_count,
                        lm.self_model().learned_patterns.len(),
                    );

                    const REFLECT_INTERVAL: u64 = 5;
                    if turn_count > 0 && turn_count % REFLECT_INTERVAL == 0 {
                        match lm
                            .reflection_engine
                            .analyze_session(&trajectory_session)
                            .await
                        {
                            Ok(reflections) => {
                                let count: usize = reflections.len();
                                lm.self_model.update_from_reflections(&reflections);
                                tracing::info!(
                                    "[run_agent_turn] Reflection: {} insights at turn {}",
                                    count,
                                    turn_count
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "[run_agent_turn] Reflection failed at turn {turn_count}: {e}"
                                );
                            }
                        }
                    }
                } else {
                    tracing::debug!(
                        "[run_agent_turn] LearningModule: not initialised, skipping self-model update"
                    );
                }

                // Verify system prompt integrity.
                let verify = frozen_snapshot.verify_detailed(&system_prompt_text);
                if !verify.valid {
                    tracing::warn!(
                        expected_hash = %verify.expected_hash,
                        actual_hash = %verify.actual_hash,
                        details = %verify.details.as_deref().unwrap_or("-"),
                        "[run_agent_turn] System prompt integrity check FAILED",
                    );
                } else {
                    tracing::info!(
                        "[run_agent_turn] System prompt integrity verified: snapshot hash={}",
                        verify.expected_hash
                    );
                }

                // WorkingMemory budget audit. MEM-MOD-P2 — the audit
                // limit now mirrors the ContextBudget setting so the
                // "exceeded" warning fires against the same number we
                // built the runtime with.
                let working_memory =
                    WorkingMemory::new(8, self.deps.context_budget.working_tokens());
                let working_tokens: usize = trajectory_session
                    .messages
                    .iter()
                    .map(crate::modules::memory::working_memory::message_token_count)
                    .sum();
                if working_tokens > working_memory.max_tokens {
                    tracing::warn!(
                        "[run_agent_turn] WorkingMemory budget exceeded: {} tokens > {} max ({} messages)",
                        working_tokens,
                        working_memory.max_tokens,
                        trajectory_session.messages.len()
                    );
                } else {
                    tracing::info!(
                        "[run_agent_turn] WorkingMemory within budget: {} tokens / {} max",
                        working_tokens,
                        working_memory.max_tokens
                    );
                }

                // WeibullDecay: importance decay (C5).
                let decay_default = WeibullDecay::default();
                match self
                    .deps
                    .memory_provider
                    .apply_importance_decay(decay_default.lambda, decay_default.k)
                    .await
                {
                    Ok(updated) if updated > 0 => {
                        tracing::info!(
                            "[run_agent_turn] WeibullDecay: applied to {updated} memory entries \
                             (lambda={:.0}h, k={:.2})",
                            decay_default.lambda,
                            decay_default.k
                        );
                    }
                    Ok(_) => {
                        tracing::debug!(
                            "[run_agent_turn] WeibullDecay: no entries updated (no-op or empty store)"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "[run_agent_turn] WeibullDecay: apply_importance_decay failed: {e}"
                        );
                    }
                }

                // Background memory promotion scan (throttled).
                {
                    use crate::modules::memory::promotion::{
                        MemoryPromotionEngine, PromotionThresholds,
                    };
                    let thresholds = PromotionThresholds::load_from_disk();
                    let engine = MemoryPromotionEngine::with_thresholds(
                        self.deps.memory_provider.as_ref(),
                        thresholds,
                    );
                    match engine.evaluate_and_audit().await {
                        Ok(Some(n)) if n > 0 => tracing::info!(
                            "[run_agent_turn] PromotionEngine: surfaced {n} candidate(s)"
                        ),
                        Ok(Some(_)) => tracing::debug!(
                            "[run_agent_turn] PromotionEngine: scan ran, no candidates"
                        ),
                        Ok(None) => tracing::debug!(
                            "[run_agent_turn] PromotionEngine: throttled, scan skipped"
                        ),
                        Err(e) => {
                            tracing::warn!("[run_agent_turn] PromotionEngine: scan failed: {e}")
                        }
                    }
                }

                let removed_count =
                    pre_compact_message_count.saturating_sub(post_compact_message_count);
                if removed_count > 0 {
                    let decay_factor = decay_default.decay_factor(24.0);
                    tracing::info!(
                        "[run_agent_turn] WeibullDecay: {removed_count} msgs compacted, 1-day factor={:.3}",
                        decay_factor
                    );
                }

                for assistant_message in &summary.assistant_messages {
                    if let Some(thinking) = assistant_message.thinking.as_ref() {
                        let _ = run_event_logger
                            .append_with_correlation(
                                "thinking_delta",
                                serde_json::json!({
                                    "thinking": thinking,
                                    "request_id": assistant_message.request_id,
                                }),
                                Some(&log_correlation),
                            )
                            .await;
                    }
                    for block in &assistant_message.blocks {
                        if let ContentBlock::Text { text } = block {
                            let _ = run_event_logger
                                .append_with_correlation(
                                    "text_delta",
                                    serde_json::json!({
                                        "text": text,
                                        "request_id": assistant_message.request_id,
                                    }),
                                    Some(&log_correlation),
                                )
                                .await;
                        }
                    }
                }
                for tool_result in &summary.tool_results {
                    for block in &tool_result.blocks {
                        if let ContentBlock::ToolResult {
                            tool_use_id,
                            tool_name,
                            output,
                            is_error,
                        } = block
                        {
                            let _ = run_event_logger
                                .append_with_correlation(
                                    if *is_error {
                                        "tool_call_failed"
                                    } else {
                                        "tool_call_completed"
                                    },
                                    serde_json::json!({
                                        "tool_call_id": tool_use_id,
                                        "tool_name": tool_name,
                                        "output": output,
                                        "is_error": is_error,
                                    }),
                                    Some(&log_correlation),
                                )
                                .await;
                        }
                    }
                }
                let _ = run_event_logger
                    .append_with_correlation(
                        "run_completed",
                        serde_json::json!({
                            "message": final_text.clone(),
                            "thinking": thinking_content.clone(),
                            "turn_iterations": summary.iterations,
                            "assistant_message_count": summary.assistant_messages.len(),
                            "tool_result_count": summary.tool_results.len(),
                        }),
                        Some(&log_correlation),
                    )
                    .await;

                tracing::info!(
                    "[run_agent_turn] Returning response with message length: {}, thinking length: {:?}, session_id: {}",
                    final_text.len(),
                    thinking_content.as_ref().map(|s| s.len()),
                    session_id
                );
                crate::modules::harness::agent_loop_integration::emit_turn_finished(
                    harness_event_bus_run.as_ref(),
                    &session_id,
                    turn_number_run,
                    true,
                    0,
                    turn_started_at_run.elapsed().as_millis() as u64,
                );
                Ok(RunTurnResponse {
                    message: final_text,
                    session_id,
                    thinking: thinking_content,
                })
            }
            Err(e) => {
                let error_message = friendly_runtime_error_message(&e);
                let _ = run_event_logger
                    .append_with_correlation(
                        "run_error",
                        serde_json::json!({
                            "error": error_message,
                            "runtime_error": e.to_string(),
                        }),
                        Some(&log_correlation),
                    )
                    .await;
                crate::modules::harness::agent_loop_integration::emit_turn_finished(
                    harness_event_bus_run.as_ref(),
                    &session_id,
                    turn_number_run,
                    false,
                    0,
                    turn_started_at_run.elapsed().as_millis() as u64,
                );
                Err(error_message)
            }
        }
    }
}

/// Map a [`RuntimeError`] into the user-facing string the IPC
/// adapter has historically returned. Behaviour preserved
/// verbatim from `commands::agent::run_agent_turn`.
fn friendly_runtime_error_message(e: &RuntimeError) -> String {
    match e {
        RuntimeError::MaxIterationsExceeded => {
            "Maximum conversation iterations reached. Please try simplifying your question."
                .to_string()
        }
        RuntimeError::ApiError(msg) => {
            if msg.contains("connection refused") {
                "Failed to connect to AI service server. Please check your network connection."
                    .to_string()
            } else if msg.contains("timeout") || msg.contains("timed out") {
                "AI service response timed out. Please try again later.".to_string()
            } else if msg.contains("dns") || msg.contains("Name or service not known") {
                "Failed to resolve AI service address. Please check network configuration."
                    .to_string()
            } else if msg.contains("401")
                || msg.contains("403")
                || msg.contains("invalid signature")
            {
                "AI service authentication failed. Please check API configuration.".to_string()
            } else if msg.contains("429") {
                "Too many AI service requests. Please try again later.".to_string()
            } else if msg.contains("500") || msg.contains("502") || msg.contains("503") {
                "AI service temporarily unavailable. Please try again later.".to_string()
            } else {
                format!("AI service call failed: {}. Please try again later.", msg)
            }
        }
        RuntimeError::ToolError(msg) => {
            format!("Tool execution failed: {}. Please try again later.", msg)
        }
        RuntimeError::PermissionDenied(msg) => {
            format!("Permission denied: {}. Please check your settings.", msg)
        }
        RuntimeError::SessionError(msg) => {
            format!(
                "Session error: {}. Please refresh the page and try again.",
                msg
            )
        }
        RuntimeError::ConfigError(msg) => {
            format!("Configuration error: {}. Please check your settings.", msg)
        }
    }
}
