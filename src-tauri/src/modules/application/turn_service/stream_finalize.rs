//! Post-loop finalize block extracted from
//! [`crate::modules::application::turn_service::stream_task::run_stream_task`].
//!
//! Owns: guardrail rewrite of unverified completion claims,
//! TaskOutcomeResolver projection, persisted-turn-outcome
//! synthesis, timeline-session-message append, app-session save,
//! trajectory recording, after-turn memory candidate dispatch,
//! learning-module turn outcome + reflection, stream_complete
//! emission with context-budget-usage payload, harness
//! TurnFinished emission, MemoryTicker.on_turn_complete fire,
//! resume-outcome log, and the stream_diag_summary tail log.
//!
//! Lifted out of `stream_task.rs` so the orchestrator file is
//! easier to navigate; behaviour is bit-for-bit preserved. The
//! 40-field [`FinalizeStreamInputs`] bundle mirrors what the
//! original closure had on its locals at the point the outer
//! `loop {}` exited.

use std::sync::Arc;

use tauri::{AppHandle, Manager};

use crate::modules::api::InputMessage;
use crate::modules::application::memory_candidate_extractor::{
    extract_memory_store_tool_candidates, lookup_existing_records_for_candidates,
};
use crate::modules::application::memory_injection_service::MemoryInjectionDeps;
use crate::modules::application::stream_emitter_service::dispatch_after_turn;
use crate::modules::application::tool_heuristics::contains_unverified_file_claim;
use crate::modules::application::trajectory_service::record_trajectory_if_possible;
use crate::modules::application::MemoryItemProjection;
use crate::modules::harness::EventBus;
use crate::modules::learning::reflection::ReflectionEngine;
use crate::modules::learning::trajectory::TrajectoryManager;
use crate::modules::learning::LearningModule;
use crate::modules::memory::retrieval::ActiveRetrievalManager;
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::{MemoryTicker, PinnedStore, SharedMemoryProvider};
use crate::modules::runtime::budget::MAX_REQUEST_TOKEN_BUDGET_ESTIMATE;
use crate::modules::runtime::compact::{compact_session, should_compact, CompactionConfig};
use crate::modules::runtime::contracts::agent_loop::{SkillResolutionPlan, WorkLoopDecision};
use crate::modules::runtime::contracts::prompt::PromptDiagnosticsSummary;
use crate::modules::runtime::event_log::RunEventLogger;
use crate::modules::runtime::projection::{self, ProjectionCheckpoint};
use crate::modules::runtime::resume_cursor::build_resume_cursor;
use crate::modules::runtime::session::{
    ContentBlock, ConversationMessage, Session as RuntimeSession,
};
use crate::modules::runtime::stream_emitter::{
    AgentStreamEmitter, ContextBudgetUsagePayload, StreamTokenPayload,
};
use crate::modules::runtime::stream_outcome::{
    ConversationTruth, ExecutionTruth, TaskOutcomeResolver,
};
use crate::modules::runtime::supervisor::SupervisorOps;
use crate::modules::runtime::timeline_flush::{
    flush_assistant_timeline_segment, PersistedTurnOutcome,
};
use crate::modules::session::{Session as AppSession, SessionManager};

/// All state the post-loop finalize block needs after the
/// streaming outer loop exits. Bundled into a struct so the
/// call site at the bottom of `run_stream_task` is one
/// `FinalizeStreamInputs { ... }` literal followed by an `.await`
/// rather than 40 inline arguments.
pub(super) struct FinalizeStreamInputs {
    // ── per-task identity ──
    pub session_id: String,
    pub stream_id_for_task: String,
    pub provider_request_id: String,
    pub turn_number_for_stream: u64,
    pub stream_turn_started_at: std::time::Instant,
    pub user_message_clone: String,
    pub inbound_resume_cursor: Option<String>,
    pub is_resume_turn: bool,

    // ── outer-loop accumulator state ──
    pub accumulated_text: String,
    pub accumulated_thinking: String,
    pub has_successful_tool: bool,
    pub has_successful_mutating_tool: bool,
    pub stream_failed: bool,
    pub completion_already_emitted: bool,
    pub terminal_status: Option<&'static str>,
    pub last_stream_error_reason: Option<String>,
    pub tool_loop_iter: usize,
    pub token_count: u32,
    pub timeline_session_messages: Vec<ConversationMessage>,
    pub session_messages: Vec<InputMessage>,
    pub system_prompt_for_stream: String,

    // ── diag counters (read-only at finalize) ──
    pub preflight_trim_rounds: usize,
    pub preflight_dropped_messages_total: usize,
    pub preflight_trimmed_chars_total: usize,
    pub sanitize_rounds: usize,
    pub sanitized_dropped_empty_messages: usize,
    pub sanitized_dropped_orphan_tool_results: usize,
    pub sanitized_dropped_unmatched_tool_uses: usize,
    pub sanitized_dropped_invalid_tool_use_inputs: usize,
    pub stream_start_retry_count: usize,
    pub stream_event_retry_count: usize,
    pub sanitize_orphan_samples: Vec<String>,
    pub sanitize_unmatched_samples: Vec<String>,
    pub sanitize_invalid_tool_use_samples: Vec<String>,
    /// P1-7 / P2-11: provider-billable token usage summed across this
    /// turn's LLM calls. `default()` (all zeros) when the provider stream
    /// did not emit `message_delta` events (frontend then falls back to
    /// the budget estimate).
    pub accumulated_usage: crate::modules::runtime::usage::TokenUsage,
    /// Effective model id used for this turn (post smart-routing). Drives
    /// pricing lookup + RoutingInfoPayload `effective_model`.
    pub effective_model: String,
    /// Provider id paired with [`effective_model`].  Forwarded to the
    /// usage store so the per-role aggregations carry vendor breakdowns.
    pub effective_provider_id: String,
    /// Provider-advertised context window (in tokens) for the effective
    /// model. Drives ContextBar `total_budget` so the bar reflects the
    /// real model limit instead of a fixed 30k baseline.
    pub effective_context_window: u64,
    /// P1-8: routing decision summary already attached to TurnContext.
    pub routing_info: Option<crate::modules::runtime::stream_emitter::RoutingInfoPayload>,
    /// Internal work-loop decision for the final report.
    pub work_loop_decision: WorkLoopDecision,
    /// Skill-resolution snapshot for the final report/event log.
    pub skill_resolution_plan: SkillResolutionPlan,
    /// Provider/tool compatibility diagnostics collected during the loop.
    pub diagnostic_warnings: Vec<String>,

    // ── service handles ──
    pub stream_emitter: AgentStreamEmitter,
    pub run_event_logger: RunEventLogger,
    pub session_manager: Arc<SessionManager>,
    pub rolling_summarizer_for_finalize: Arc<crate::modules::memory::summary::RollingSummarizer>,
    pub app_session_clone: AppSession,
    pub trajectory_manager_for_stream: Option<Arc<TrajectoryManager>>,
    pub memory_provider_for_stream: SharedMemoryProvider,
    pub memory_ticker_for_stream: Arc<MemoryTicker>,
    pub harness_event_bus_for_stream: Option<EventBus>,
    pub learning_module_for_stream: Option<Arc<tokio::sync::Mutex<LearningModule>>>,
    pub memory_context_items_for_task: Vec<MemoryItemProjection>,
    pub prompt_diagnostics_for_task: PromptDiagnosticsSummary,
    pub prompt_diagnostics_enabled_for_task: bool,
    pub baseline_message_count_stream: usize,

    // ── after-turn dispatch ──
    pub app_handle_for_after_turn: AppHandle,
    pub harness_bus_for_after_turn: Option<EventBus>,
    pub pinned_store_for_after_turn: Arc<dyn PinnedStore>,
    pub memory_provider_for_after_turn: SharedMemoryProvider,
    pub active_retrieval_manager_for_after_turn: Option<Arc<ActiveRetrievalManager>>,
    pub stream_session_id_for_after_turn: String,
    pub stream_project_id_for_after_turn: Option<String>,
    /// A.3.1 — trajectory collector forwarded from `StreamTaskInputs`.
    pub trajectory_collector:
        Arc<crate::modules::memory::evolution::trajectory::TrajectoryCollector>,
    /// A.3.1 — task description (user_message[..200]) captured at run entry,
    /// reused here when synthesising the final TurnRecord.
    pub trajectory_task_description: String,
}

/// Run the post-loop finalize block for one streaming chat turn.
///
/// Called by [`super::stream_task::run_stream_task`] after the
/// outer streaming loop exits, regardless of whether the loop
/// terminated via natural completion, cancellation,
/// max-iteration breakout, or stream failure.
///
/// Behaviour preserved bit-for-bit from the original inline
/// post-loop block; the only structural change is the
/// destructure prologue that re-binds the bundled struct fields
/// to the same local variable names the original code used.
pub(super) async fn finalize_stream_task(inputs: FinalizeStreamInputs) {
    let FinalizeStreamInputs {
        session_id,
        stream_id_for_task,
        provider_request_id,
        turn_number_for_stream,
        stream_turn_started_at,
        user_message_clone,
        inbound_resume_cursor,
        is_resume_turn,
        mut accumulated_text,
        mut accumulated_thinking,
        has_successful_tool,
        has_successful_mutating_tool,
        stream_failed,
        completion_already_emitted,
        mut terminal_status,
        last_stream_error_reason,
        tool_loop_iter,
        token_count,
        mut timeline_session_messages,
        session_messages,
        system_prompt_for_stream,
        preflight_trim_rounds,
        preflight_dropped_messages_total,
        preflight_trimmed_chars_total,
        sanitize_rounds,
        sanitized_dropped_empty_messages,
        sanitized_dropped_orphan_tool_results,
        sanitized_dropped_unmatched_tool_uses,
        sanitized_dropped_invalid_tool_use_inputs,
        stream_start_retry_count,
        stream_event_retry_count,
        sanitize_orphan_samples,
        sanitize_unmatched_samples,
        sanitize_invalid_tool_use_samples,
        accumulated_usage,
        effective_model,
        effective_provider_id,
        effective_context_window,
        routing_info,
        work_loop_decision,
        skill_resolution_plan,
        mut diagnostic_warnings,
        stream_emitter,
        run_event_logger,
        session_manager,
        rolling_summarizer_for_finalize,
        app_session_clone,
        trajectory_manager_for_stream,
        memory_provider_for_stream,
        memory_ticker_for_stream,
        harness_event_bus_for_stream,
        learning_module_for_stream,
        memory_context_items_for_task,
        prompt_diagnostics_for_task,
        prompt_diagnostics_enabled_for_task,
        baseline_message_count_stream,
        app_handle_for_after_turn,
        harness_bus_for_after_turn,
        pinned_store_for_after_turn,
        memory_provider_for_after_turn,
        active_retrieval_manager_for_after_turn,
        stream_session_id_for_after_turn,
        stream_project_id_for_after_turn,
        trajectory_collector,
        trajectory_task_description,
    } = inputs;

    // Guardrail: do not allow "operation completed" claims without a successful
    // mutating tool evidence in this request.
    //
    // Resume turns intentionally skip this check: the previous turn already
    // ran the tool (its mutation evidence lives in earlier session messages,
    // not in `has_successful_mutating_tool` of this resume), and the
    // accumulated_text typically carries the assistant's prior "已写入 …"
    // wrap-up text that we want to preserve.
    if should_rewrite_unverified_completion(
        &work_loop_decision,
        is_resume_turn,
        &accumulated_text,
        has_successful_mutating_tool,
    ) {
        let guarded = "未执行工具，无法确认完成。".to_string();
        tracing::warn!(
            "[start_agent_stream] Rewriting unverified completion claim to guarded message"
        );
        accumulated_text = guarded.clone();
        let override_payload = StreamTokenPayload {
            stream_id: stream_id_for_task.clone(),
            correlation: None,
            text: Some(guarded),
            thinking: None,
            event_type: "final_text_override".to_string(),
            tool_call_id: None,
            tool_name: None,
            tool_status: None,
            tool_args: None,
            tool_result: None,
            tool_duration_ms: None,
            effective_workdir: None,
            policy_decision: None,
            evidence_id: None,
            request_id: Some(provider_request_id.clone()),
            task_outcome: None,
            degraded_reason: None,
            resume_available: None,
            resume_cursor: None,
            recoverability: None,
            context_budget_usage: None,
            memory_context: None,
            prompt_diagnostics: None,
            turn_cost: None,
            routing_info: None,
            session_totals: None,
        };
        stream_emitter.emit_payload(override_payload.clone());
        let _ = run_event_logger
            .append("final_text_override", override_payload)
            .await;
    }

    if terminal_status == Some("memory_recall_required_no_tool") {
        let guarded = "我没有完成记忆查询：本轮模型只说要查看记忆，但没有成功调用 memory_recall 或 memory_export。请重试这条消息；系统会把它作为需要记忆证据的请求处理。".to_string();
        tracing::warn!(
            "[start_agent_stream] Rewriting incomplete memory lookup filler to guarded message"
        );
        accumulated_text = guarded.clone();
        let override_payload = StreamTokenPayload {
            stream_id: stream_id_for_task.clone(),
            correlation: None,
            text: Some(guarded),
            thinking: None,
            event_type: "final_text_override".to_string(),
            tool_call_id: None,
            tool_name: None,
            tool_status: None,
            tool_args: None,
            tool_result: None,
            tool_duration_ms: None,
            effective_workdir: None,
            policy_decision: None,
            evidence_id: None,
            request_id: Some(provider_request_id.clone()),
            task_outcome: None,
            degraded_reason: Some("memory_recall_required_no_tool".to_string()),
            resume_available: None,
            resume_cursor: None,
            recoverability: None,
            context_budget_usage: None,
            memory_context: None,
            prompt_diagnostics: None,
            turn_cost: None,
            routing_info: None,
            session_totals: None,
        };
        stream_emitter.emit_payload(override_payload.clone());
        let _ = run_event_logger
            .append("final_text_override", override_payload)
            .await;
    }

    let todo_snapshot = super::todo_ledger::load_snapshot(&session_id);
    let (guarded_terminal_status, todo_warning) =
        super::todo_ledger::terminal_status_with_todo_ledger(
            &work_loop_decision,
            terminal_status,
            todo_snapshot.as_ref(),
            has_successful_mutating_tool,
        );
    if guarded_terminal_status != terminal_status {
        tracing::warn!(
            session_id,
            stream_id = stream_id_for_task,
            status = ?guarded_terminal_status,
            "[start_agent_stream] Todo ledger still has unfinished work at finalization"
        );
        terminal_status = guarded_terminal_status;
    }
    if let Some(warning) = todo_warning {
        diagnostic_warnings.push(warning);
    }

    let user_visible_truth = TaskOutcomeResolver::resolve(
        ExecutionTruth {
            has_successful_tool,
            has_successful_mutating_tool,
        },
        &ConversationTruth {
            stream_failed,
            terminal_status: terminal_status.unwrap_or("unknown"),
            last_stream_error_reason: last_stream_error_reason.clone(),
        },
    );
    let resume_cursor = user_visible_truth
        .resume_available
        .then(|| build_resume_cursor(&stream_id_for_task, tool_loop_iter, token_count));
    let degraded_reason = user_visible_truth.degraded_reason.clone();
    if user_visible_truth.task_outcome == "partial_success" && accumulated_text.trim().is_empty() {
        let fallback = match degraded_reason.as_deref() {
            Some("max_iterations_reached") => {
                "已执行部分工具调用，但达到工具迭代上限，未能生成最终总结。你可以点击继续或重新发送，让我基于已有结果继续。"
            }
            Some(_) => "已执行部分工具调用，但本轮在生成最终总结前降级结束。你可以点击继续或重新发送，让我基于已有结果继续。",
            None => "已执行部分工具调用，但本轮未生成最终总结。你可以点击继续或重新发送，让我基于已有结果继续。",
        }
        .to_string();
        tracing::warn!(
            "[start_agent_stream] Emitting partial-success fallback text: status={}, degraded={:?}",
            terminal_status.unwrap_or("unknown"),
            degraded_reason
        );
        accumulated_text = fallback.clone();
        let override_payload = StreamTokenPayload {
            stream_id: stream_id_for_task.clone(),
            correlation: None,
            text: Some(fallback),
            thinking: None,
            event_type: "final_text_override".to_string(),
            tool_call_id: None,
            tool_name: None,
            tool_status: None,
            tool_args: None,
            tool_result: None,
            tool_duration_ms: None,
            effective_workdir: None,
            policy_decision: None,
            evidence_id: None,
            request_id: Some(provider_request_id.clone()),
            task_outcome: Some(user_visible_truth.task_outcome.to_string()),
            degraded_reason: degraded_reason.clone(),
            resume_available: Some(user_visible_truth.resume_available),
            resume_cursor: resume_cursor.clone(),
            recoverability: None,
            context_budget_usage: None,
            memory_context: None,
            prompt_diagnostics: None,
            turn_cost: None,
            routing_info: None,
            session_totals: None,
        };
        stream_emitter.emit_payload(override_payload.clone());
        let _ = run_event_logger
            .append("final_text_override", override_payload)
            .await;
    }
    let persisted_turn_outcome = PersistedTurnOutcome {
        task_outcome: user_visible_truth.task_outcome.to_string(),
        degraded_reason: degraded_reason.clone(),
        resume_available: user_visible_truth.resume_available,
        resume_cursor: resume_cursor.clone(),
        request_id: provider_request_id.clone(),
    };

    let final_run_report = super::work_loop::build_final_run_report(
        &work_loop_decision,
        user_visible_truth.task_outcome.to_string(),
        terminal_status.unwrap_or("unknown").to_string(),
        provider_request_id.clone(),
        tool_loop_iter,
        has_successful_tool,
        has_successful_mutating_tool,
        user_visible_truth.resume_available,
        resume_cursor.clone(),
        Some(&skill_resolution_plan),
        diagnostic_warnings,
    );
    let final_report_value = serde_json::to_value(&final_run_report)
        .unwrap_or_else(|error| serde_json::json!({ "serializationError": error.to_string() }));
    let mut final_report_payload =
        StreamTokenPayload::skeleton(stream_id_for_task.clone(), "final_run_report");
    final_report_payload.request_id = Some(provider_request_id.clone());
    final_report_payload.task_outcome = Some(user_visible_truth.task_outcome.to_string());
    final_report_payload.degraded_reason = degraded_reason.clone();
    final_report_payload.resume_available = Some(user_visible_truth.resume_available);
    final_report_payload.resume_cursor = resume_cursor.clone();
    final_report_payload.tool_args = Some(final_report_value);
    stream_emitter.emit_payload(final_report_payload.clone());
    let _ = run_event_logger
        .append("final_run_report", final_report_payload)
        .await;

    let skill_resolution_value = serde_json::to_value(&skill_resolution_plan)
        .unwrap_or_else(|error| serde_json::json!({ "serializationError": error.to_string() }));
    let _ = run_event_logger
        .append(
            "skill_resolution_snapshot",
            serde_json::json!({
                "stream_id": stream_id_for_task.clone(),
                "request_id": provider_request_id.clone(),
                "skill_resolution": skill_resolution_value,
            }),
        )
        .await;

    session_manager.push_conversation_undo_checkpoint(&session_id, &app_session_clone);

    // Save session with all accumulated messages
    let mut updated_app_session = app_session_clone;
    let user_msg = crate::modules::runtime::session::ConversationMessage {
        role: crate::modules::runtime::session::MessageRole::User,
        blocks: vec![ContentBlock::Text {
            text: user_message_clone.clone(),
        }],
        usage: None,
        thinking: None,
        task_outcome: None,
        degraded_reason: None,
        resume_available: None,
        resume_cursor: None,
        request_id: Some(provider_request_id.clone()),
        finish_reason: None,
    };
    // P1-7 / P2-11 — capture the assistant output text length BEFORE
    // `flush_assistant_timeline_segment` `mem::take`s `accumulated_text`,
    // otherwise the tiktoken fallback below sees an empty string and the
    // chip / session totals show `0 输出` even when the LLM produced output.
    let assistant_output_for_estimate: String = accumulated_text.clone();

    flush_assistant_timeline_segment(
        &mut timeline_session_messages,
        &mut accumulated_text,
        &mut accumulated_thinking,
        if stream_failed || user_visible_truth.task_outcome == "partial_success" {
            Some(&persisted_turn_outcome)
        } else {
            None
        },
    );
    if (stream_failed || user_visible_truth.task_outcome == "partial_success")
        && timeline_session_messages
            .last()
            .is_none_or(|message| message.resume_cursor.as_deref() != resume_cursor.as_deref())
    {
        timeline_session_messages.push(crate::modules::runtime::session::ConversationMessage {
            role: crate::modules::runtime::session::MessageRole::Assistant,
            blocks: vec![ContentBlock::Text {
                text: String::new(),
            }],
            usage: None,
            thinking: None,
            task_outcome: Some(persisted_turn_outcome.task_outcome.clone()),
            degraded_reason: persisted_turn_outcome.degraded_reason.clone(),
            resume_available: Some(persisted_turn_outcome.resume_available),
            resume_cursor: persisted_turn_outcome.resume_cursor.clone(),
            request_id: Some(persisted_turn_outcome.request_id.clone()),
            finish_reason: None,
        });
    }
    // P1-7 / P2-11 — fall back to a tiktoken estimate when the provider
    // didn't ship usage (Ollama / many OpenAI-compat endpoints unless
    // `stream_options.include_usage: true` is set).  We tag the model
    // string with `~est` so the chip / dashboards can flag the value
    // as approximate while still giving the user *some* signal.
    let (final_usage, model_label) = if accumulated_usage.total_tokens() > 0 {
        (accumulated_usage, effective_model.clone())
    } else {
        let est_input: u32 =
            (crate::modules::runtime::budget::estimate_tokens(&system_prompt_for_stream)
                + session_messages
                    .iter()
                    .map(|m| {
                        m.content
                            .iter()
                            .map(|c| match c {
                                crate::modules::api::InputContentBlock::Text { text } => {
                                    crate::modules::runtime::budget::estimate_tokens(text)
                                }
                                crate::modules::api::InputContentBlock::ToolResult {
                                    content,
                                    ..
                                } => content
                                    .iter()
                                    .map(|b| match b {
                                        crate::modules::api::ToolResultContentBlock::Text {
                                            text,
                                        } => crate::modules::runtime::budget::estimate_tokens(text),
                                        crate::modules::api::ToolResultContentBlock::Json {
                                            value,
                                        } => crate::modules::runtime::budget::estimate_tokens(
                                            &value.to_string(),
                                        ),
                                        crate::modules::api::ToolResultContentBlock::Image {
                                            alt,
                                            ..
                                        } => alt.as_deref().map_or(
                                            0,
                                            crate::modules::runtime::budget::estimate_tokens,
                                        ),
                                    })
                                    .sum::<usize>(),
                                _ => 0,
                            })
                            .sum::<usize>()
                    })
                    .sum::<usize>())
            .min(u32::MAX as usize) as u32;
        let est_output: u32 =
            (crate::modules::runtime::budget::estimate_tokens(&assistant_output_for_estimate))
                .min(u32::MAX as usize) as u32;
        let est = crate::modules::runtime::usage::TokenUsage {
            input_tokens: est_input,
            output_tokens: est_output,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };
        let label = if effective_model.is_empty() {
            "~est".to_string()
        } else {
            format!("{} ~est", effective_model)
        };
        tracing::info!(
            "[start_agent_stream] usage fallback to tiktoken estimate: in={} out={} model={}",
            est_input,
            est_output,
            effective_model
        );
        (est, label)
    };

    // Stamp the final assistant message with the resolved usage.  Done
    // BEFORE save_session so reload picks up the chip.
    if final_usage.total_tokens() > 0 {
        if let Some(last_assistant) = timeline_session_messages
            .iter_mut()
            .rev()
            .find(|m| m.role == crate::modules::runtime::session::MessageRole::Assistant)
        {
            last_assistant.usage = Some(final_usage);
        }
    }
    let appended_message_count = 1 + timeline_session_messages.len();
    updated_app_session.messages.push(user_msg);
    updated_app_session
        .messages
        .extend(timeline_session_messages);
    updated_app_session.message_count =
        updated_app_session.logical_message_count() + appended_message_count;

    // P2-11 — bump per-session running totals before save_session so
    // reload reflects the latest billed cost. `cost_usd_for_turn` is
    // also forwarded into the stream_complete payload below.
    let cost_usd_for_turn = if final_usage.total_tokens() > 0 {
        let cost = crate::modules::runtime::usage::cost_for_usage(final_usage, &effective_model);
        let mut totals = updated_app_session
            .session_totals
            .clone()
            .unwrap_or_default();
        totals.record(final_usage, cost);
        updated_app_session.session_totals = Some(totals);

        // Per-role usage durable log (Settings ▸ 用量统计 page).  Best
        // effort — failures inside the store are logged but never bubble
        // back to the chat turn (observability, not control flow).
        crate::modules::usage::record_turn_usage(crate::modules::usage::TurnUsageRecord {
            caller: crate::modules::usage::CALLER_CHAT.to_string(),
            provider_id: effective_provider_id.clone(),
            model_id: effective_model.clone(),
            usage: final_usage,
            cost_usd: cost,
            session_id: Some(stream_session_id_for_after_turn.clone()),
        });

        Some(cost)
    } else {
        None
    };

    // Context compaction — compact if session exceeds token threshold
    let compaction_config = CompactionConfig::default();
    if should_compact(
        &RuntimeSession {
            version: 1,
            messages: updated_app_session.messages.clone(),
        },
        compaction_config,
    ) {
        let compact_result = compact_session(
            &RuntimeSession {
                version: 1,
                messages: updated_app_session.messages.clone(),
            },
            compaction_config,
        );
        updated_app_session.messages = compact_result.compacted_session.messages;
    }

    if let Err(e) = session_manager.save_session(&updated_app_session).await {
        tracing::error!("[start_agent_stream] Failed to save session: {}", e);
    }

    // P1-7 — conversation recall index (SQLite FTS; best-effort).
    {
        let turn_id = format!(
            "{}:{}",
            stream_session_id_for_after_turn, updated_app_session.message_count
        );
        if let Err(e) = memory_provider_for_stream
            .conversation_recall_ingest(
                &stream_session_id_for_after_turn,
                stream_project_id_for_after_turn.as_deref(),
                &turn_id,
                &user_message_clone,
                &accumulated_text.chars().take(12_000).collect::<String>(),
            )
            .await
        {
            tracing::debug!(error = %e, "[conversation_recall] ingest skipped");
        }
    }

    // ── Post-turn streaming parity (mirrors run_agent_turn) ──

    // Build a runtime session snapshot for trajectory recording.
    let trajectory_runtime_session = RuntimeSession {
        version: 1,
        messages: updated_app_session.messages.clone(),
    };

    // Trajectory recording.
    record_trajectory_if_possible(
        &trajectory_runtime_session,
        std::slice::from_ref(&system_prompt_for_stream),
        trajectory_manager_for_stream.as_ref(),
    )
    .await;

    // Phase M4.1 — extract real `MemoryWriteCandidate`s from
    // the assistant `memory_store` tool calls produced during
    // THIS streaming turn (slice from the captured baseline)
    // and look up existing records so the conflict resolver
    // renders real outcomes.  Mirrors the `run_agent_turn`
    // site exactly.
    let new_messages_stream: Vec<ConversationMessage> = updated_app_session
        .messages
        .iter()
        .skip(baseline_message_count_stream)
        .cloned()
        .collect();
    let after_turn_scope_stream = MemoryExecutionScope {
        session_id: Some(stream_session_id_for_after_turn.clone()),
        project_id: stream_project_id_for_after_turn.clone(),
        workdir: None,
    };
    let candidates_stream =
        extract_memory_store_tool_candidates(&new_messages_stream, &after_turn_scope_stream);
    let existing_stream = lookup_existing_records_for_candidates(
        &memory_provider_for_after_turn,
        &after_turn_scope_stream,
        &candidates_stream,
    )
    .await;
    dispatch_after_turn(
        &app_handle_for_after_turn,
        harness_bus_for_after_turn.as_ref(),
        MemoryInjectionDeps {
            pinned_store: pinned_store_for_after_turn.clone(),
            memory_provider: memory_provider_for_after_turn.clone(),
            active_retrieval_manager: active_retrieval_manager_for_after_turn.clone(),
        },
        Some(stream_session_id_for_after_turn.clone()),
        stream_project_id_for_after_turn.clone(),
        candidates_stream,
        existing_stream,
        Vec::new(),
        "start_agent_stream",
    )
    .await;

    // LearningModule: record turn + reflection trigger.
    if let Some(lm_arc) = &learning_module_for_stream {
        let mut lm = lm_arc.lock().await;
        lm.self_model_mut().record_turn(
            /* success= */ !stream_failed,
            /* response_time_ms= */ 0.0,
        );

        let turn_count = lm.self_model().performance.total_turns;
        tracing::info!(
            "[start_agent_stream] LearningModule: turn {} recorded",
            turn_count
        );

        const STREAM_REFLECT_INTERVAL: u64 = 5;
        if turn_count > 0 && turn_count % STREAM_REFLECT_INTERVAL == 0 {
            match lm
                .reflection_engine
                .analyze_session(&trajectory_runtime_session)
                .await
            {
                Ok(reflections) => {
                    let count: usize = reflections.len();
                    lm.self_model.update_from_reflections(&reflections);
                    tracing::info!(
                        "[start_agent_stream] Reflection: {} insights at turn {}",
                        count,
                        turn_count
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "[start_agent_stream] Reflection failed at turn {turn_count}: {e}"
                    );
                }
            }
        }
    }

    // WeibullDecay importance decay (non-blocking, warn-only on error).
    {
        use crate::modules::runtime::episodic_compaction::WeibullDecay;
        let decay_default = WeibullDecay::default();
        if let Err(e) = memory_provider_for_stream
            .apply_importance_decay(decay_default.lambda, decay_default.k)
            .await
        {
            tracing::warn!("[start_agent_stream] WeibullDecay: apply_importance_decay failed: {e}");
        }
    }

    // Background memory promotion scan — throttled to once per minute
    // (process-wide) so the cost is amortised across turns.  Surfaces
    // candidates as `memory_promotion_candidate` audit events; never
    // mutates the store on its own.
    {
        use crate::modules::memory::promotion::{MemoryPromotionEngine, PromotionThresholds};
        let thresholds = PromotionThresholds::load_from_disk();
        let engine =
            MemoryPromotionEngine::with_thresholds(memory_provider_for_stream.as_ref(), thresholds);
        match engine.evaluate_and_audit().await {
            Ok(Some(n)) if n > 0 => {
                tracing::info!("[start_agent_stream] PromotionEngine: surfaced {n} candidate(s)")
            }
            Ok(Some(_)) => {
                tracing::debug!("[start_agent_stream] PromotionEngine: scan ran, no candidates")
            }
            Ok(None) => {
                tracing::debug!("[start_agent_stream] PromotionEngine: throttled, scan skipped")
            }
            Err(e) => {
                tracing::warn!("[start_agent_stream] PromotionEngine: scan failed: {e}")
            }
        }
    }

    // Emit stream_complete exactly once, and only after the full
    // tool/LLM loop has finished for this request.
    if !stream_failed && !completion_already_emitted {
        // Compute live token-budget breakdown for the frontend ContextBar.
        // Counts are estimates derived from tiktoken cl100k_base; the
        // total budget mirrors the runtime ContextGovernor.
        let system_tokens =
            crate::modules::runtime::budget::estimate_tokens(&system_prompt_for_stream);
        let history_tokens: usize = session_messages
            .iter()
            .map(|m| {
                m.content
                    .iter()
                    .map(|c| match c {
                        crate::modules::api::InputContentBlock::Text { text } => {
                            crate::modules::runtime::budget::estimate_tokens(text)
                        }
                        crate::modules::api::InputContentBlock::ToolResult { content, .. } => {
                            content
                                .iter()
                                .map(|b| match b {
                                    crate::modules::api::ToolResultContentBlock::Text { text } => {
                                        crate::modules::runtime::budget::estimate_tokens(text)
                                    }
                                    // JSON tool results are estimated from their serialised
                                    // representation so structured outputs still count toward
                                    // the per-turn history budget surfaced in `ContextBar`.
                                    crate::modules::api::ToolResultContentBlock::Json { value } => {
                                        crate::modules::runtime::budget::estimate_tokens(
                                            &value.to_string(),
                                        )
                                    }
                                    // Image content blocks (Phase 7C, slice 7C.2):
                                    // base64 payload doesn't go through the text
                                    // tokeniser (vision providers count it on
                                    // their own); contribute the alt text only.
                                    crate::modules::api::ToolResultContentBlock::Image {
                                        alt,
                                        ..
                                    } => alt.as_deref().map_or(0, |a| {
                                        crate::modules::runtime::budget::estimate_tokens(a)
                                    }),
                                })
                                .sum::<usize>()
                        }
                        _ => 0,
                    })
                    .sum::<usize>()
            })
            .sum();
        let memory_tokens: usize = memory_context_items_for_task
            .iter()
            .map(|i| crate::modules::runtime::budget::estimate_tokens(&i.content))
            .sum();
        const OUTPUT_RESERVE: usize = 4_096;
        // Real model context window drives the bar; legacy 30k baseline
        // stays as a hard floor for unknown / undersized models so we
        // never display a tinier-than-30k cap.
        let context_floor =
            MAX_REQUEST_TOKEN_BUDGET_ESTIMATE.max(effective_context_window as usize);
        let total_budget = context_floor
            .max(system_tokens + history_tokens + memory_tokens + OUTPUT_RESERVE + 1024);
        let used = system_tokens + history_tokens + memory_tokens + OUTPUT_RESERVE;
        let remaining = total_budget.saturating_sub(used);
        let usage = ContextBudgetUsagePayload {
            total_budget,
            system_tokens,
            history_tokens,
            memory_tokens,
            output_reserve: OUTPUT_RESERVE,
            remaining,
        };
        let memory_payload = if memory_context_items_for_task.is_empty() {
            None
        } else {
            Some(memory_context_items_for_task.clone())
        };

        // Build the per-turn / per-session payloads from the values we
        // already wrote into `updated_app_session` above. `model_label`
        // carries the `~est` suffix when we fell back to tiktoken so
        // the chip tooltip shows it.
        let turn_cost_payload = cost_usd_for_turn.map(|cost| {
            crate::modules::runtime::stream_emitter::TurnCostPayload {
                input_tokens: final_usage.input_tokens,
                output_tokens: final_usage.output_tokens,
                cache_creation_input_tokens: final_usage.cache_creation_input_tokens,
                cache_read_input_tokens: final_usage.cache_read_input_tokens,
                cost_usd: cost,
                model: model_label.clone(),
            }
        });
        let session_totals_payload = updated_app_session.session_totals.as_ref().map(|t| {
            crate::modules::runtime::stream_emitter::SessionUsageTotalsPayload {
                input_tokens: t.input_tokens,
                output_tokens: t.output_tokens,
                cache_creation_input_tokens: t.cache_creation_input_tokens,
                cache_read_input_tokens: t.cache_read_input_tokens,
                cost_usd: t.cost_usd,
                turns: t.turns,
            }
        });

        let payload = StreamTokenPayload {
            stream_id: stream_id_for_task.clone(),
            correlation: None,
            text: None,
            thinking: None,
            event_type: "stream_complete".to_string(),
            tool_call_id: None,
            tool_name: None,
            tool_status: None,
            tool_args: None,
            tool_result: None,
            tool_duration_ms: None,
            effective_workdir: None,
            policy_decision: None,
            evidence_id: None,
            request_id: Some(provider_request_id.clone()),
            task_outcome: Some(user_visible_truth.task_outcome.to_string()),
            degraded_reason: degraded_reason.clone(),
            resume_available: Some(user_visible_truth.resume_available),
            resume_cursor: resume_cursor.clone(),
            recoverability: Some(user_visible_truth.recoverability.clone()),
            context_budget_usage: Some(usage),
            memory_context: memory_payload,
            prompt_diagnostics: prompt_diagnostics_enabled_for_task
                .then_some(prompt_diagnostics_for_task),
            turn_cost: turn_cost_payload,
            routing_info: routing_info.clone(),
            session_totals: session_totals_payload,
        };
        stream_emitter.emit_payload(payload);
        let _ = run_event_logger
            .append(
                "stream_complete",
                serde_json::json!({
                    "stream_id": stream_id_for_task.clone(),
                    "request_id": provider_request_id.clone(),
                    "task_outcome": user_visible_truth.task_outcome.to_string(),
                    "degraded_reason": degraded_reason.clone(),
                    "resume_available": user_visible_truth.resume_available,
                    "resume_cursor": resume_cursor.clone(),
                }),
            )
            .await;
        if terminal_status.is_none() {
            terminal_status = Some("completed");
        }

        // Auto-compact: if the budget bar just crossed the configured
        // threshold (default 85%), kick off RollingSummarizer in the
        // background so the *next* turn ships with a leaner context.
        // The actual compact runs in `chat_compact::spawn_auto_compact`
        // and emits `chat_compact_completed` on success so the UI can
        // toast.
        use crate::modules::application::compact_service as cs;
        if cs::auto_compact_enabled() && total_budget > 0 {
            let used_pct = used as f32 / total_budget as f32;
            let threshold = cs::auto_compact_threshold();
            if used_pct >= threshold {
                {
                    tracing::info!(
                        target: "if2ai::compact",
                        session_id = %stream_session_id_for_after_turn,
                        used_pct,
                        threshold,
                        "auto-compact: threshold crossed, spawning background fold"
                    );
                    cs::spawn_auto_compact(
                        app_handle_for_after_turn.clone(),
                        session_manager.clone(),
                        rolling_summarizer_for_finalize.clone(),
                        stream_session_id_for_after_turn.clone(),
                    );
                }
            }
        }
    }

    // Phase 6E harness: emit TurnFinished for the streaming path.
    // We treat any non-failed stream as success here; downstream consumers
    // can refine via `task_outcome` if needed.
    let turn_duration_ms = stream_turn_started_at.elapsed().as_millis() as u64;
    crate::modules::harness::agent_loop_integration::emit_turn_finished(
        harness_event_bus_for_stream.as_ref(),
        &session_id,
        turn_number_for_stream,
        !stream_failed,
        token_count,
        turn_duration_ms,
    );
    // DT-01 S1.3 — mirror the canonical `conversation:turn_finished`
    // envelope onto the run-log so the runlog-fold path
    // (`fold_run_log_to_report`) can derive turn-level aggregates
    // without reaching into the harness EventBus.  Audit §2.
    crate::modules::harness::agent_loop_integration::dispatch_turn_finished_envelope(
        Some(&app_handle_for_after_turn),
        Some(&run_event_logger),
        &session_id,
        Some(run_event_logger.run_id()),
        &crate::modules::harness::agent_loop_integration::TurnFinishedPayload {
            turn_number: turn_number_for_stream,
            succeeded: !stream_failed,
            duration_ms: turn_duration_ms,
            terminal_status: terminal_status.map(str::to_string),
            tokens_in: None,
            tokens_out: Some(u64::from(token_count)),
        },
    );
    crate::modules::learning::estimation::record_turn_duration_ms(turn_duration_ms);
    crate::modules::observability::emit(
        "stream_turn_finished",
        &format!(
            "session_id={} turn={} ms={} tokens={} failed={}",
            session_id, turn_number_for_stream, turn_duration_ms, token_count, stream_failed
        ),
    );
    crate::modules::application::job_monitor::publish_line(
        &session_id,
        format!(
            "stream turn {} finished: {}ms tokens={}",
            turn_number_for_stream, turn_duration_ms, token_count
        ),
    );

    // Phase 8B.11 fix — fire MemoryTicker.on_turn_complete for the
    // streaming path.  The non-streaming run_agent_turn path goes
    // through ConversationRuntime.with_turn_hook, but
    // start_agent_stream streams directly so the hook needs an
    // explicit invocation here.  Using the persisted message list
    // ensures RollingSummarizer's incremental slice (`message_count`)
    // matches what the user actually saw.
    if !stream_failed {
        use crate::modules::memory::scope::MemoryExecutionScope;
        use crate::modules::runtime::conversation::TurnHook;
        let messages_for_hook = updated_app_session.messages.clone();
        let scope_for_hook = MemoryExecutionScope {
            session_id: Some(session_id.clone()),
            project_id: if updated_app_session.project_id.is_empty() {
                None
            } else {
                Some(updated_app_session.project_id.clone())
            },
            workdir: None,
        };
        // Phase 8B.11 fix-debug — info log so it's visible in dev console.
        tracing::info!(
            session_id = %session_id,
            project_id = updated_app_session.project_id.as_str(),
            messages = messages_for_hook.len(),
            "[stream] firing memory_ticker.on_turn_complete"
        );
        memory_ticker_for_stream.on_turn_complete(&scope_for_hook, &session_id, &messages_for_hook);
    } else {
        tracing::info!(
            session_id = %session_id,
            "[stream] SKIP memory_ticker.on_turn_complete (stream_failed=true)"
        );
    }

    if is_resume_turn {
        tracing::info!(
            "[resume_outcome] session_id='{}', stream_id='{}', request_id='{}', inbound_resume_cursor='{}', task_outcome='{}', success={}",
            session_id,
            stream_id_for_task,
            provider_request_id.as_str(),
            inbound_resume_cursor.as_deref().unwrap_or("none"),
            user_visible_truth.task_outcome,
            user_visible_truth.task_outcome == "completed"
        );
    }

    tracing::info!(
        "[stream_diag_summary] stream_id='{}', session_id='{}', request_id='{}', status='{}', task_outcome='{}', degraded_reason='{}', resume_available={}, resume_cursor='{}', is_resume_turn={}, inbound_resume_cursor='{}', tool_loop_iter={}, token_count={}, stream_failed={}, completion_already_emitted={}, has_successful_tool={}, has_successful_mutating_tool={}, preflight_trim_rounds={}, preflight_dropped_messages_total={}, preflight_trimmed_chars_total={}, sanitize_rounds={}, dropped_empty_messages_total={}, dropped_orphan_tool_results_total={}, dropped_unmatched_tool_uses_total={}, dropped_invalid_tool_use_inputs_total={}, start_retry_count={}, event_retry_count={}, orphan_tool_result_samples={:?}, unmatched_tool_use_samples={:?}, invalid_tool_use_input_samples={:?}, last_stream_error={}",
        stream_id_for_task,
        session_id,
        provider_request_id.as_str(),
        terminal_status.unwrap_or("unknown"),
        user_visible_truth.task_outcome,
        degraded_reason.as_deref().unwrap_or("none"),
        user_visible_truth.resume_available,
        resume_cursor.as_deref().unwrap_or("none"),
        is_resume_turn,
        inbound_resume_cursor.as_deref().unwrap_or("none"),
        tool_loop_iter,
        token_count,
        stream_failed,
        completion_already_emitted,
        has_successful_tool,
        has_successful_mutating_tool,
        preflight_trim_rounds,
        preflight_dropped_messages_total,
        preflight_trimmed_chars_total,
        sanitize_rounds,
        sanitized_dropped_empty_messages,
        sanitized_dropped_orphan_tool_results,
        sanitized_dropped_unmatched_tool_uses,
        sanitized_dropped_invalid_tool_use_inputs,
        stream_start_retry_count,
        stream_event_retry_count,
        sanitize_orphan_samples,
        sanitize_unmatched_samples,
        sanitize_invalid_tool_use_samples,
        last_stream_error_reason.as_deref().unwrap_or("none"),
    );

    // MIG-020 (T-006) / DR-01: Record supervisor lifecycle event on run completion.
    // Best-effort: supervisor persistence failures must never fail the turn.
    if let Ok(app_data_dir) = app_handle_for_after_turn.path().app_data_dir() {
        let ops = SupervisorOps {
            app_data_dir: &app_data_dir,
            session_id: &session_id,
            app_handle: Some(&app_handle_for_after_turn),
            run_event_logger: Some(&run_event_logger),
        };
        if stream_failed {
            let reason = degraded_reason
                .clone()
                .unwrap_or_else(|| "stream_error".to_string());
            if user_visible_truth.resume_available {
                ops.run_failed_recoverable(&reason);
            } else {
                ops.run_failed_final(&reason);
            }
        } else {
            ops.run_completed();
        }

        // T-020: save projection checkpoint with latest seq.
        // Preserve existing snapshot if the frontend already saved one.
        {
            let last_seq = run_event_logger.current_seq();
            let existing = projection::load_checkpoint(&app_data_dir, &session_id).ok();
            let snapshot = existing
                .and_then(|r| {
                    if r.checkpoint_exists {
                        Some(r.snapshot)
                    } else {
                        None
                    }
                })
                .unwrap_or(serde_json::Value::Null);
            let cp = ProjectionCheckpoint::new(session_id.clone(), last_seq, snapshot);
            if let Err(e) = projection::save_checkpoint(&app_data_dir, &cp) {
                tracing::warn!(
                    session_id = %session_id,
                    error = %e,
                    "[projection] failed to save checkpoint after turn finalize"
                );
            }
        }
    }

    // DW-003 — fire-and-forget evolution finalize hooks. Three
    // independent pipelines (sedimentation / checkpoint extract /
    // domain-knowledge contributor). All failure-isolated; the
    // turn's main flow already returned by the time these spawn.
    spawn_evolution_finalize_hooks(EvolutionFinalizeArgs {
        session_id: session_id.clone(),
        history: session_messages.clone(),
        accumulated_text: accumulated_text.clone(),
        app_handle: app_handle_for_after_turn.clone(),
    });

    // A.3.1 — finalize trajectory based on terminal_status / stream_failed.
    // Mirrors run.rs::run_turn outcome classification (PR #4).
    let trajectory_outcome = if stream_failed || matches!(terminal_status, Some("cancelled")) {
        let category = if matches!(terminal_status, Some("cancelled")) {
            "cancelled".to_string()
        } else {
            last_stream_error_reason
                .as_deref()
                .map(classify_streaming_error)
                .unwrap_or_else(|| "unknown".to_string())
        };
        let root_cause = last_stream_error_reason
            .clone()
            .unwrap_or_else(|| terminal_status.unwrap_or("unknown").to_string());
        crate::modules::memory::evolution::trajectory::TaskOutcome::Failure {
            error_category: category,
            root_cause,
        }
    } else {
        crate::modules::memory::evolution::trajectory::TaskOutcome::Success { quality_score: 1.0 }
    };
    let trajectory_success = matches!(
        trajectory_outcome,
        crate::modules::memory::evolution::trajectory::TaskOutcome::Success { .. }
    );
    // A.3.2 — drain pending tool-call observations into TurnRecord.tool_calls
    // so SelfReflector's tool-pattern rules see real signal. agent_action is
    // ToolUse on a successful run that touched tools; Error on any failure;
    // Reply on a successful text-only turn.
    let trajectory_recorded_tool_calls = trajectory_collector
        .drain_pending_tool_calls(&session_id)
        .await;
    let trajectory_agent_action = if !trajectory_success {
        crate::modules::memory::evolution::trajectory::AgentAction::Error
    } else if trajectory_recorded_tool_calls.is_empty() {
        crate::modules::memory::evolution::trajectory::AgentAction::Reply
    } else {
        crate::modules::memory::evolution::trajectory::AgentAction::ToolUse
    };
    trajectory_collector
        .record_turn(
            &session_id,
            crate::modules::memory::evolution::trajectory::TurnRecord {
                turn_id: 0,
                timestamp: chrono::Utc::now(),
                user_input_summary: Some(trajectory_task_description),
                agent_action: trajectory_agent_action,
                tool_calls: trajectory_recorded_tool_calls,
                success: trajectory_success,
                self_assessment: None,
            },
        )
        .await;
    trajectory_collector
        .finish_trajectory(&session_id, trajectory_outcome)
        .await;
}

// ---------------------------------------------------------------------------
// DW-003 — Evolution finalize spawn helper
// ---------------------------------------------------------------------------

struct EvolutionFinalizeArgs {
    session_id: String,
    history: Vec<InputMessage>,
    accumulated_text: String,
    app_handle: AppHandle,
}

/// Spawn the three WU-003 finalize pipelines as fire-and-forget
/// tokio tasks. Each pipeline is independently failure-isolated:
/// any panic / error inside the spawned task only logs at warn —
/// it never bubbles back into the turn's main flow (which has
/// already returned by the time these run).
///
/// **Truth-loop iter-3 (2026-04-30)**: sedimentation + domain-knowledge
/// contributor now run against the real `ChatProviderUtilityLlm`
/// (same shim `bootstrap/memory.rs` binds for rolling summary /
/// reflection). Drafts and knowledge candidates are real LLM output;
/// the spawn pipeline + emit path remain failure-isolated so
/// provider downtime degrades gracefully to "no drafts this turn".
fn spawn_evolution_finalize_hooks(args: EvolutionFinalizeArgs) {
    use crate::modules::memory::ChatProviderUtilityLlm;
    use crate::modules::runtime::contracts::common::{CorrelationIds, RuntimeEventType};
    use crate::modules::runtime::evolution_emitter::emit_evolution_event;

    let EvolutionFinalizeArgs {
        session_id,
        history,
        accumulated_text,
        app_handle,
    } = args;

    // (2) Sync checkpoint extraction — always runs (no LLM, no I/O).
    let extraction = super::finalize_hooks::extract_turn_checkpoint(&accumulated_text);
    if extraction.key_info.is_some() || extraction.should_clear {
        let payload = serde_json::json!({
            "sessionId": session_id,
            "action": if extraction.should_clear { "cleared" } else { "extracted" },
            "keyInfoTokens": extraction
                .key_info
                .as_deref()
                .map(crate::modules::runtime::budget::estimate_tokens)
                .unwrap_or(0),
            "relatedSop": extraction.related_sop,
        });
        let _ = emit_evolution_event(
            Some(&app_handle),
            RuntimeEventType::CheckpointUpdated,
            if extraction.should_clear {
                "cleared"
            } else {
                "extracted"
            },
            CorrelationIds {
                session_id: Some(session_id.clone()),
                ..Default::default()
            },
            &payload,
            None,
        );
    }

    // (1) Sedimentation pipeline — real chat-provider LLM (iter-3).
    {
        let session_id = session_id.clone();
        let history = history.clone();
        let app_handle = app_handle.clone();
        tokio::spawn(async move {
            let llm: Arc<dyn crate::modules::memory::UtilityLlm> = Arc::new(
                ChatProviderUtilityLlm::new(crate::modules::config::store::if2ai_data_root()),
            );
            let drafts = super::finalize_hooks::run_sedimentation_pipeline(&history, llm).await;
            for draft in drafts {
                let payload = serde_json::json!({
                    "name": draft.name,
                    "description": draft.description,
                    "toolSequence": draft.tool_sequence,
                    "sourceTurns": draft.source_turns,
                });
                let _ = emit_evolution_event(
                    Some(&app_handle),
                    RuntimeEventType::SkillSedimented,
                    "draft",
                    CorrelationIds {
                        session_id: Some(session_id.clone()),
                        ..Default::default()
                    },
                    &payload,
                    None,
                );
            }
        });
    }

    // (3) Domain-knowledge contributor — real chat-provider LLM (iter-3).
    {
        let session_id = session_id.clone();
        let history = history.clone();
        let app_handle = app_handle.clone();
        tokio::spawn(async move {
            let llm: Arc<dyn crate::modules::memory::UtilityLlm> = Arc::new(
                ChatProviderUtilityLlm::new(crate::modules::config::store::if2ai_data_root()),
            );
            let candidates =
                super::finalize_hooks::run_domain_knowledge_contributor(&history, llm).await;
            let dk_store = crate::modules::skills::domain_knowledge::global_knowledge_store();
            for entry in candidates {
                // DW-004/WU-008: persist into the process-wide store so the DK
                // lookup hook in work_loop sees it on the very next turn.
                dk_store.upsert(entry.clone()).await;
                let payload = serde_json::json!({
                    "entryId": entry.id,
                    "kind": entry.kind.label(),
                    "source": "contribution",
                    "accessCount": entry.access_count,
                });
                let _ = emit_evolution_event(
                    Some(&app_handle),
                    RuntimeEventType::DomainKnowledge,
                    "contribution",
                    CorrelationIds {
                        session_id: Some(session_id.clone()),
                        ..Default::default()
                    },
                    &payload,
                    None,
                );
            }
        });
    }
}

fn should_rewrite_unverified_completion(
    work_loop_decision: &WorkLoopDecision,
    is_resume_turn: bool,
    accumulated_text: &str,
    has_successful_mutating_tool: bool,
) -> bool {
    !is_resume_turn
        && super::work_loop::requires_tool_execution_evidence(work_loop_decision)
        && contains_unverified_file_claim(accumulated_text)
        && !has_successful_mutating_tool
}

/// A.3.1 — coarse error categorization for streaming-path failures.
/// Mirrors `classify_trajectory_error` in `run.rs` but without
/// importing it (run.rs is a sibling, the helper is intentionally
/// duplicated to avoid pub-ing it for one reuse).
fn classify_streaming_error(msg: &str) -> String {
    let lower = msg.to_lowercase();
    if lower.contains("timeout") || lower.contains("timed out") {
        "timeout".into()
    } else if lower.contains("cancel") {
        "cancelled".into()
    } else if lower.contains("tool") {
        "tool_error".into()
    } else if lower.contains("provider") || lower.contains("api") {
        "provider_error".into()
    } else {
        "unknown".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::runtime::contracts::agent_loop::{WorkLoopDecision, WorkLoopKind};

    fn decision(reason_codes: Vec<&str>) -> WorkLoopDecision {
        WorkLoopDecision {
            loop_kind: WorkLoopKind::DirectExecute,
            reason_codes: reason_codes
                .into_iter()
                .map(std::string::ToString::to_string)
                .collect(),
            requires_confirmation: false,
            route_hint: None,
        }
    }

    #[test]
    fn read_only_shell_summary_does_not_trigger_mutation_guard() {
        let work_loop = decision(vec!["simple_shell_command_intent"]);

        assert!(!should_rewrite_unverified_completion(
            &work_loop,
            false,
            "README.md was successfully deleted earlier.",
            false,
        ));
    }

    #[test]
    fn tool_required_completion_claim_requires_mutating_evidence() {
        let work_loop = decision(vec!["tool_required_work_intent"]);

        assert!(should_rewrite_unverified_completion(
            &work_loop,
            false,
            "已创建 r.md。",
            false,
        ));
    }
}

