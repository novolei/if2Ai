//! Tool-batch execution loop extracted from
//! [`crate::modules::application::turn_service::stream_task::run_stream_task`].
//!
//! Owns: flush_assistant_timeline_segment, permission-prompter setup,
//! per-tool permission checking (session-override, policy, interactive prompt),
//! tool validation + execution, event emission (running/queued/completed/error),
//! session/timeline message appending, invalid-args-streak tracking, harness
//! ToolCalled/ToolResult/PrepareStepExecuted events, and self-repair recording.
//!
//! Lifted out of `stream_task.rs` per GAP-005. Behaviour is bit-for-bit
//! preserved.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use crate::modules::api::InputMessage;
use crate::modules::application::permission_service::TauriPermissionPrompter;
use crate::modules::application::tool_executor::ToolRegistryExecutor;
use crate::modules::application::tool_heuristics::is_mutating_tool_success;
use crate::modules::control_plane::AuditEmitter;
use crate::modules::harness::{agent_loop_integration, AgentEvent, EventBus};
use crate::modules::runtime::attempt_ledger::{self, ToolAttempt, ToolAttemptLedger};
use crate::modules::runtime::block_conversion::{
    parse_tool_input_json, summarize_tool_result_for_model,
};
use crate::modules::runtime::contracts::agent_loop::{PendingOperationMetadata, WorkLoopDecision};
use crate::modules::runtime::contracts::common::CorrelationIds;
use crate::modules::runtime::event_log::RunEventLogger;
use crate::modules::runtime::permissions::{
    PermissionMode, PermissionOutcome, PermissionPolicy, PermissionPromptDecision,
};
use crate::modules::runtime::session::ConversationMessage;
use crate::modules::runtime::stream_emitter::{AgentStreamEmitter, StreamTokenPayload};
use crate::modules::runtime::timeline_flush::flush_assistant_timeline_segment;
use crate::modules::session::SessionManager;
use crate::modules::tools::ToolRegistry;

use super::stream_task::append_remembered_permission_events;

const INVALID_TOOL_ARGS_LIMIT: usize = 2;

fn tool_schema_json(tool_registry: &ToolRegistry, tool_name: &str) -> String {
    tool_registry
        .get(tool_name)
        .and_then(|entry| serde_json::to_string_pretty(&entry.input_schema).ok())
        .unwrap_or_else(|| "{\"type\":\"object\"}".to_string())
}

fn tool_argument_example(tool_name: &str) -> Option<&'static str> {
    match tool_name {
        "REPL" => Some(r#"{"language":"shell","code":"printf 'hello\n'"}"#),
        "bash" => Some(r#"{"command":"pwd"}"#),
        "file_write" | "write_file" => {
            Some(r#"{"path":"index.html","content":"<!doctype html>\n<html></html>"}"#)
        }
        "file_read" | "read_file" => Some(r#"{"path":"index.html"}"#),
        _ => None,
    }
}

fn invalid_tool_args_repair_instruction(
    tool_registry: &ToolRegistry,
    tool_name: &str,
    validation_error: &str,
) -> String {
    let schema = tool_schema_json(tool_registry, tool_name);
    let example = tool_argument_example(tool_name)
        .map(|value| format!("\nMinimal valid example for `{tool_name}`:\n```json\n{value}\n```"))
        .unwrap_or_default();
    format!(
        "[tool_argument_repair]\nThe previous `{tool_name}` tool call was rejected before execution: {validation_error}.\nCall `{tool_name}` again only with a complete JSON object matching this exact schema:\n```json\n{schema}\n```{example}\nDo not call the tool with `{{}}` or omit required fields. If this tool is not appropriate, choose another available tool with complete arguments."
    )
}

fn repair_tool_input_before_validation(
    tool_name: &str,
    tool_input: &serde_json::Value,
) -> Option<serde_json::Value> {
    if !matches!(tool_name, "file_write" | "write_file") {
        return None;
    }
    let object = tool_input.as_object()?;
    let has_path = object
        .get("path")
        .and_then(|value| value.as_str())
        .is_some_and(|value| !value.trim().is_empty());
    if has_path {
        return None;
    }
    let content = object.get("content").and_then(|value| value.as_str())?;
    let inferred_path = infer_file_write_path_from_content(content)?;
    let mut repaired = object.clone();
    repaired.insert(
        "path".to_string(),
        serde_json::Value::String(inferred_path.to_string()),
    );
    Some(serde_json::Value::Object(repaired))
}

fn infer_file_write_path_from_content(content: &str) -> Option<&'static str> {
    let trimmed = content.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    let lower_prefix: String = trimmed.chars().take(256).collect::<String>().to_lowercase();
    if lower_prefix.starts_with("<!doctype html")
        || lower_prefix.starts_with("<html")
        || (lower_prefix.contains("<html") && lower_prefix.contains("<body"))
    {
        return Some("index.html");
    }
    if lower_prefix.starts_with("{") || lower_prefix.starts_with("[") {
        return Some("output.json");
    }
    if lower_prefix.starts_with("# ") || lower_prefix.starts_with("---\n") {
        return Some("output.md");
    }
    Some("output.txt")
}

/// All context needed to execute one batch of pending tool calls.
pub(super) struct ToolExecutionContext {
    pub pending_tool_uses: Vec<(String, String, String)>,
    pub stream_id: String,
    pub session_id: String,
    pub provider_request_id: String,
    pub accumulated_text: String,
    pub accumulated_thinking: String,
    pub session_messages: Vec<InputMessage>,
    pub timeline_session_messages: Vec<ConversationMessage>,
    pub stream_emitter: AgentStreamEmitter,
    pub run_event_logger: RunEventLogger,
    pub tool_registry: Arc<ToolRegistry>,
    pub tool_executor: ToolRegistryExecutor,
    pub permission_senders:
        Arc<Mutex<HashMap<String, std::sync::mpsc::Sender<PermissionPromptDecision>>>>,
    pub permission_overrides:
        Arc<Mutex<HashMap<String, HashMap<String, PermissionPromptDecision>>>>,
    pub permission_policy: Arc<PermissionPolicy>,
    pub execution_context: crate::modules::control_plane::SessionExecutionContext,
    pub work_loop_decision: WorkLoopDecision,
    pub harness_bus: Option<EventBus>,
    pub mode: PermissionMode,
    pub invalid_tool_args_streak: usize,
    pub has_successful_tool: bool,
    pub has_successful_mutating_tool: bool,
    pub sanitized_dropped_invalid_tool_use_inputs: usize,
    pub sanitize_invalid_tool_use_samples: Vec<String>,
    pub run_id: String,
    pub app_data_dir: PathBuf,
    /// A.3.2 — trajectory collector for per-tool ToolCallRecord observation.
    /// Each completed tool execution calls
    /// [`crate::modules::memory::evolution::trajectory::TrajectoryCollector::observe_tool_call`]
    /// so the run finalize site can drain into `TurnRecord.tool_calls`.
    pub trajectory_collector:
        Arc<crate::modules::memory::evolution::trajectory::TrajectoryCollector>,
}

/// Results produced by executing one batch of tool calls.
pub(super) struct ToolExecutionResult {
    pub accumulated_text: String,
    pub accumulated_thinking: String,
    pub session_messages: Vec<InputMessage>,
    pub timeline_session_messages: Vec<ConversationMessage>,
    pub has_successful_tool: bool,
    pub has_successful_mutating_tool: bool,
    pub force_final_response_next: bool,
    pub finalization_reason: Option<String>,
    pub invalid_tool_args_streak: usize,
    pub sanitized_dropped_invalid_tool_use_inputs: usize,
    pub sanitize_invalid_tool_use_samples: Vec<String>,
    /// Returned to the orchestrator for reuse in subsequent outer-loop iterations.
    pub tool_executor: ToolRegistryExecutor,
    /// Pending operation preserved when the loop blocks on approval.
    pub pending_operation: Option<PendingOperationMetadata>,
}

/// Execute one batch of pending tool calls within a streaming chat turn.
///
/// Persists the accumulated text/thinking segment before tool execution,
/// sets up the interactive permission prompter, then processes each tool:
/// validation, permission check (session override → policy → prompt),
/// execution, event emission, and session/timeline message appending.
///
/// Returns all mutated state via [`ToolExecutionResult`]; the orchestrator
/// destructures and continues the outer tool loop.
pub(super) async fn execute_tool_batch(ctx: ToolExecutionContext) -> ToolExecutionResult {
    let ToolExecutionContext {
        pending_tool_uses,
        stream_id,
        session_id,
        provider_request_id,
        accumulated_text,
        accumulated_thinking,
        mut session_messages,
        mut timeline_session_messages,
        stream_emitter,
        run_event_logger,
        tool_registry,
        mut tool_executor,
        permission_senders,
        permission_overrides,
        permission_policy,
        execution_context,
        work_loop_decision,
        harness_bus,
        mode,
        mut invalid_tool_args_streak,
        mut has_successful_tool,
        mut has_successful_mutating_tool,
        mut sanitized_dropped_invalid_tool_use_inputs,
        mut sanitize_invalid_tool_use_samples,
        run_id,
        app_data_dir,
        trajectory_collector,
    } = ctx;

    let mut force_final_response_next = false;
    let mut finalization_reason: Option<String> = None;
    let mut pending_operation: Option<PendingOperationMetadata> = None;
    let mut accumulated_text = accumulated_text;
    let mut accumulated_thinking = accumulated_thinking;

    // Persist the assistant segment before tool execution.
    flush_assistant_timeline_segment(
        &mut timeline_session_messages,
        &mut accumulated_text,
        &mut accumulated_thinking,
        None,
    );

    let execution_context_for_policy = execution_context.clone();

    // Set up TauriPermissionPrompter for interactive permission requests
    let (perm_tx, perm_rx): (
        std::sync::mpsc::Sender<PermissionPromptDecision>,
        std::sync::mpsc::Receiver<PermissionPromptDecision>,
    ) = std::sync::mpsc::channel();
    match permission_senders.lock() {
        Ok(mut senders) => {
            senders.insert(session_id.clone(), perm_tx);
        }
        Err(e) => {
            tracing::error!(
                "[start_agent_stream] failed to lock permission_senders: {}",
                e
            );
        }
    }
    let mut prompter = TauriPermissionPrompter::new(
        stream_emitter.window().clone(),
        session_id.clone(),
        perm_rx,
        Some(run_event_logger.clone()),
    );

    for (tool_id, tool_name, input_json) in pending_tool_uses.into_iter() {
        let mut input_json = input_json;
        let policy_trace_id = AuditEmitter::new_trace_id();
        let attempt_id = Uuid::new_v4().to_string();

        // ── Attempt Ledger: determine attempt_no and record entry (MIG-022 / T-013) ──
        let attempt_no = ToolAttemptLedger::next_attempt_no(&app_data_dir, &session_id, &tool_id)
            .unwrap_or_else(|e| {
                tracing::warn!(
                    "[attempt_ledger] failed to compute next attempt_no for {}: {}",
                    tool_id,
                    e
                );
                1
            });
        let mut attempt_entry = ToolAttempt::new(
            session_id.clone(),
            run_id.clone(),
            tool_id.clone(),
            attempt_id.clone(),
            attempt_no,
            tool_name.clone(),
        );
        if let Err(e) = attempt_ledger::record_attempt(&app_data_dir, &attempt_entry) {
            tracing::warn!("[attempt_ledger] failed to persist queued entry: {}", e);
        }

        let correlation_ids = CorrelationIds {
            attempt_id: Some(attempt_id.clone()),
            ..Default::default()
        };
        let diag_key = format!(
            "stream_id={};trace_id={};request_id={}",
            stream_id, policy_trace_id, provider_request_id
        );
        tracing::info!(
            "[stream_audit_link] diag_key={}, stream_id={}, session_id={}, tool_call_id={}, trace_id={}, request_id={}, tool_name={}",
            diag_key,
            stream_id,
            session_id,
            tool_id,
            policy_trace_id,
            provider_request_id.as_str(),
            tool_name
        );

        // Check session-scoped remember decisions first.
        let remembered_decision = permission_overrides
            .lock()
            .ok()
            .and_then(|all| all.get(&session_id).cloned())
            .and_then(|tool_map| {
                tool_map
                    .get(&tool_name)
                    .cloned()
                    .or_else(|| tool_map.get("*").cloned())
            });
        if let Some(decision) = remembered_decision.as_ref() {
            append_remembered_permission_events(&run_event_logger, &tool_name, decision).await;
        }
        let running_policy_decision = match remembered_decision.as_ref() {
            Some(PermissionPromptDecision::Allow) => "session_allow",
            Some(PermissionPromptDecision::Deny { .. }) => "session_deny",
            None => "prompt",
        };

        let mut tool_input = parse_tool_input_json(&input_json);
        if let Some(repaired_input) = repair_tool_input_before_validation(&tool_name, &tool_input) {
            tracing::warn!(
                "[start_agent_stream] repaired incomplete tool args before validation: stream_id={}, session_id={}, tool_call_id={}, tool_name={}, repaired_fields=path",
                stream_id,
                session_id,
                tool_id,
                tool_name
            );
            input_json = repaired_input.to_string();
            tool_input = repaired_input;
        }

        // Emit running event once the permission source is known.
        let running_payload = StreamTokenPayload {
            stream_id: stream_id.clone(),
            correlation: Some(correlation_ids.clone()),
            text: None,
            thinking: None,
            event_type: "tool_call_update".to_string(),
            tool_call_id: Some(tool_id.clone()),
            tool_name: Some(tool_name.clone()),
            tool_status: Some("running".to_string()),
            tool_args: Some(tool_input.clone()),
            tool_result: None,
            tool_duration_ms: None,
            effective_workdir: Some(execution_context_for_policy.workdir.display().to_string()),
            policy_decision: Some(running_policy_decision.to_string()),
            evidence_id: Some(policy_trace_id.clone()),
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
        stream_emitter.emit_payload(running_payload.clone());
        super::stream_task_run_log::append_stream_event(&run_event_logger, &running_payload).await;

        // ── Attempt Ledger: transition to running ──
        attempt_entry.transition_running();
        if let Err(e) = attempt_ledger::record_attempt(&app_data_dir, &attempt_entry) {
            tracing::warn!("[attempt_ledger] failed to persist running entry: {}", e);
        }

        if let Some(block_reason) =
            super::work_loop::mutation_block_reason(&work_loop_decision, &tool_name, &input_json)
        {
            tracing::warn!(
                "[start_agent_stream] Work-loop policy blocked tool before execution: stream_id={}, session_id={}, tool_call_id={}, tool_name={}, reason={}",
                stream_id,
                session_id,
                tool_id,
                tool_name,
                block_reason
            );
            let terminal_tool_payload = StreamTokenPayload {
                stream_id: stream_id.clone(),
                correlation: Some(correlation_ids.clone()),
                text: None,
                thinking: None,
                event_type: "tool_call_update".to_string(),
                tool_call_id: Some(tool_id.clone()),
                tool_name: Some(tool_name.clone()),
                tool_status: Some("error".to_string()),
                tool_args: Some(tool_input.clone()),
                tool_result: Some(block_reason.clone()),
                tool_duration_ms: Some(0),
                effective_workdir: Some(execution_context_for_policy.workdir.display().to_string()),
                policy_decision: Some("needs_approval".to_string()),
                evidence_id: Some(policy_trace_id.clone()),
                request_id: Some(provider_request_id.clone()),
                task_outcome: None,
                degraded_reason: Some("approval_required_for_mutation".to_string()),
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
            stream_emitter.emit_payload(terminal_tool_payload.clone());
            super::stream_task_run_log::append_stream_event(
                &run_event_logger,
                &terminal_tool_payload,
            )
            .await;

            attempt_entry.transition_blocked();
            let _ = attempt_ledger::record_attempt(&app_data_dir, &attempt_entry);

            session_messages.push(InputMessage {
                role: "assistant".to_string(),
                content: vec![crate::modules::api::InputContentBlock::ToolUse {
                    id: tool_id.clone(),
                    name: tool_name.clone(),
                    input: tool_input.clone(),
                }],
                thinking: None,
            });
            session_messages.push(InputMessage {
                role: "user".to_string(),
                content: vec![crate::modules::api::InputContentBlock::ToolResult {
                    tool_use_id: tool_id.clone(),
                    content: vec![crate::modules::api::ToolResultContentBlock::Text {
                        text: summarize_tool_result_for_model(
                            &tool_name,
                            &tool_id,
                            &block_reason,
                            true,
                        ),
                    }],
                    is_error: true,
                }],
                thinking: None,
            });
            timeline_session_messages.push(ConversationMessage::tool_use(
                tool_id.clone(),
                tool_name.clone(),
                input_json.clone(),
            ));
            timeline_session_messages.push(ConversationMessage::tool_result(
                tool_id.clone(),
                tool_name.clone(),
                block_reason.clone(),
                true,
            ));
            if let Some(last_message) = timeline_session_messages.last_mut() {
                last_message.request_id = Some(provider_request_id.clone());
            }

            force_final_response_next = true;
            finalization_reason = Some("approval_required_for_mutation".to_string());
            pending_operation = Some(PendingOperationMetadata {
                tool_call_id: tool_id,
                tool_name,
                reason: block_reason,
            });
            continue;
        }

        // Validate tool args before permission/execution.
        if let Some(validation_error) = tool_registry.validate(&tool_name, &tool_input) {
            invalid_tool_args_streak += 1;
            sanitized_dropped_invalid_tool_use_inputs += 1;
            if sanitize_invalid_tool_use_samples.len() < 12 {
                sanitize_invalid_tool_use_samples
                    .push(format!("{}:{}:{}", tool_id, tool_name, validation_error));
            }
            tracing::warn!(
                "[start_agent_stream] invalid tool args blocked before execution: stream_id={}, session_id={}, tool_call_id={}, tool_name={}, error={}, streak={}",
                stream_id,
                session_id,
                tool_id,
                tool_name,
                validation_error,
                invalid_tool_args_streak
            );

            crate::modules::runtime::self_repair::record_tool_outcome(&tool_name, false);
            let schema = tool_schema_json(&tool_registry, &tool_name);
            let example = tool_argument_example(&tool_name)
                .map(|value| format!(" Example: {value}."))
                .unwrap_or_default();
            let invalid_result = format!(
                "invalid tool arguments: {validation_error}. The tool `{tool_name}` was not executed. Re-issue the tool call with a complete JSON object matching this schema: {schema}.{example}"
            );
            let repair_instruction =
                invalid_tool_args_repair_instruction(&tool_registry, &tool_name, &validation_error);
            let terminal_tool_payload = StreamTokenPayload {
                stream_id: stream_id.clone(),
                correlation: Some(correlation_ids.clone()),
                text: None,
                thinking: None,
                event_type: "tool_call_update".to_string(),
                tool_call_id: Some(tool_id.clone()),
                tool_name: Some(tool_name.clone()),
                tool_status: Some("error".to_string()),
                tool_args: Some(tool_input.clone()),
                tool_result: Some(invalid_result.clone()),
                tool_duration_ms: Some(0),
                effective_workdir: Some(execution_context_for_policy.workdir.display().to_string()),
                policy_decision: Some("blocked_invalid_args".to_string()),
                evidence_id: Some(policy_trace_id.clone()),
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
            stream_emitter.emit_payload(terminal_tool_payload.clone());
            super::stream_task_run_log::append_stream_event(
                &run_event_logger,
                &terminal_tool_payload,
            )
            .await;

            // ── Attempt Ledger: transition to failed (invalid args) ──
            attempt_entry.transition_failed("invalid_args".to_string(), 0);
            let _ = attempt_ledger::record_attempt(&app_data_dir, &attempt_entry);

            session_messages.push(InputMessage {
                role: "assistant".to_string(),
                content: vec![crate::modules::api::InputContentBlock::ToolUse {
                    id: tool_id.clone(),
                    name: tool_name.clone(),
                    input: tool_input.clone(),
                }],
                thinking: None,
            });
            session_messages.push(InputMessage {
                role: "user".to_string(),
                content: vec![crate::modules::api::InputContentBlock::ToolResult {
                    tool_use_id: tool_id.clone(),
                    content: vec![crate::modules::api::ToolResultContentBlock::Text {
                        text: summarize_tool_result_for_model(
                            &tool_name,
                            &tool_id,
                            &invalid_result,
                            true,
                        ),
                    }],
                    is_error: true,
                }],
                thinking: None,
            });
            session_messages.push(InputMessage::user_text(repair_instruction));
            timeline_session_messages.push(ConversationMessage::tool_use(
                tool_id.clone(),
                tool_name.clone(),
                input_json.clone(),
            ));
            timeline_session_messages.push(ConversationMessage::tool_result(
                tool_id.clone(),
                tool_name.clone(),
                invalid_result,
                true,
            ));
            if let Some(last_message) = timeline_session_messages.last_mut() {
                last_message.request_id = Some(provider_request_id.clone());
            }

            if invalid_tool_args_streak >= INVALID_TOOL_ARGS_LIMIT {
                tracing::warn!(
                    "[start_agent_stream] repeated invalid tool args detected; next iteration will force final summary. stream_id={}, session_id={}, streak={}",
                    stream_id,
                    session_id,
                    invalid_tool_args_streak
                );
                force_final_response_next = true;
                finalization_reason = Some("invalid_tool_args_repeated".to_string());
            }
            continue;
        }
        invalid_tool_args_streak = 0;

        // Permission check.
        let permission_outcome = match remembered_decision {
            Some(PermissionPromptDecision::Allow) => PermissionOutcome::Allow,
            Some(PermissionPromptDecision::Deny { reason }) => PermissionOutcome::Deny { reason },
            None => permission_policy.authorize(&tool_name, &input_json, Some(&mut prompter)),
        };
        match &permission_outcome {
            PermissionOutcome::Allow => {
                AuditEmitter::policy_decision_made(
                    &policy_trace_id,
                    &execution_context_for_policy.session_id,
                    &tool_name,
                    &execution_context_for_policy.workdir,
                    mode,
                    "allow",
                    Some(provider_request_id.as_str()),
                );
            }
            PermissionOutcome::Deny { reason } => {
                AuditEmitter::policy_decision_made(
                    &policy_trace_id,
                    &execution_context_for_policy.session_id,
                    &tool_name,
                    &execution_context_for_policy.workdir,
                    mode,
                    &format!("deny:{reason}"),
                    Some(provider_request_id.as_str()),
                );
            }
        }
        if let PermissionOutcome::Deny { reason } = &permission_outcome {
            tracing::warn!(
                "[start_agent_stream] Permission denied for tool '{}' in mode {}: {}",
                tool_name,
                mode.as_str(),
                reason
            );
        }

        let timeline_tool_use =
            ConversationMessage::tool_use(tool_id.clone(), tool_name.clone(), input_json.clone());
        timeline_session_messages.push(timeline_tool_use);

        let start_time = std::time::Instant::now();
        let denied_by_policy = matches!(permission_outcome, PermissionOutcome::Deny { .. });

        // Harness: emit ToolCalled before invocation.
        agent_loop_integration::emit_tool_called(
            harness_bus.as_ref(),
            &session_id,
            &tool_name,
            &input_json.to_string(),
        );

        // Harness: PrepareStepExecuted shadow trace.
        //
        // DT-01 S1.3 — mirror the bus emit onto the canonical
        // run-log via `system:prepare_step` so the runlog-fold path
        // (`fold_run_log_to_report`) can populate
        // `aggregate.prepare_step_*` and `evidence.prepare_step`
        // without reaching into the harness EventBus.  Audit §10.
        // The compute used to be gated on `harness_bus.is_some()`;
        // now the prepare-step decision is always computed because
        // the run-log is the canonical truth (the bus emit remains
        // gated as before).
        {
            let parsed_args = parse_tool_input_json(&input_json);
            let prep_out =
                crate::modules::control_plane::prepare_step_execution::prepare_step_execution(
                    crate::modules::control_plane::prepare_step_execution::PrepareStepExecutionInput {
                        tool_name: &tool_name,
                        session_context: &execution_context_for_policy,
                        args: &parsed_args,
                        permission_policy: permission_policy.clone(),
                    },
                );
            // (1) Run-log mirror — canonical truth for fold path.
            let prepare_payload = serde_json::json!({
                "toolName": tool_name,
                "outcome": prep_out.outcome,
                "boundary": prep_out.boundary_decision,
                "permission": prep_out.permission_decision,
                "sandbox": prep_out.sandbox_policy,
                "policyVersion": prep_out.policy_version,
            });
            let prepare_correlation = CorrelationIds {
                session_id: Some(session_id.clone()),
                run_id: Some(run_id.clone()),
                ..Default::default()
            };
            if let Err(err) = crate::modules::runtime::runtime_event::dispatch(
                None,
                crate::modules::runtime::contracts::common::RuntimeEventType::System,
                "prepare_step",
                prepare_correlation,
                &prepare_payload,
                Some(&run_event_logger),
            ) {
                tracing::trace!(
                    error = ?err,
                    "[harness] prepare_step envelope dispatch failed (non-fatal)"
                );
            }
            // (2) Bus emit — preserved for telemetry / aggregator
            // until DT-01 Stage 2 retires the EventBus.
            if let Some(bus) = harness_bus.as_ref() {
                let _ = bus.emit(AgentEvent::PrepareStepExecuted {
                    session_id: session_id.clone(),
                    tool_name: tool_name.clone(),
                    outcome: prep_out.outcome,
                    boundary: prep_out.boundary_decision,
                    permission: prep_out.permission_decision,
                    sandbox: prep_out.sandbox_policy,
                    policy_version: prep_out.policy_version,
                    at: chrono::Utc::now(),
                });
            }
        }

        let (result_text, is_error) = match permission_outcome {
            PermissionOutcome::Allow => {
                match tool_executor.execute_with_trace(
                    &tool_name,
                    &input_json,
                    &policy_trace_id,
                    Some(provider_request_id.as_str()),
                    Some(attempt_id.as_str()),
                ) {
                    Ok(output) => (output, false),
                    Err(e) => (e.to_string(), true),
                }
            }
            PermissionOutcome::Deny { reason } => (reason, true),
        };

        let safety = crate::modules::security::safety::shared_safety_layer();
        let sanitized_tool = safety.sanitize_tool_output(&tool_name, &result_text);
        let wrapped_for_llm = safety.wrap_for_llm(&tool_name, &sanitized_tool.content);
        let policy_decision = if denied_by_policy { "deny" } else { "allow" };
        let duration_ms = start_time.elapsed().as_millis() as u64;
        if !is_error {
            has_successful_tool = true;
        }

        // Harness: emit ToolResult.
        agent_loop_integration::emit_tool_result(
            harness_bus.as_ref(),
            &session_id,
            &tool_name,
            !is_error,
            duration_ms,
        );

        // A.3.2 — observe this tool's outcome on the active trajectory so
        // the run finalize site (stream_finalize.rs) can drain the bucket
        // into `TurnRecord.tool_calls` for SelfReflector.
        trajectory_collector
            .observe_tool_call(
                &session_id,
                crate::modules::memory::evolution::trajectory::ToolCallRecord {
                    tool_name: tool_name.clone(),
                    args_summary: String::new(),
                    success: !is_error,
                    duration_ms,
                    error_message: if is_error {
                        Some(result_text.clone())
                    } else {
                        None
                    },
                },
            )
            .await;

        if is_mutating_tool_success(&tool_name, &input_json, is_error) {
            has_successful_mutating_tool = true;
        }
        crate::modules::runtime::self_repair::record_tool_outcome(&tool_name, !is_error);

        // Emit completed/error event.
        let terminal_tool_payload = StreamTokenPayload {
            stream_id: stream_id.clone(),
            correlation: Some(correlation_ids.clone()),
            text: None,
            thinking: None,
            event_type: "tool_call_update".to_string(),
            tool_call_id: Some(tool_id.clone()),
            tool_name: Some(tool_name.clone()),
            tool_status: Some(if is_error { "error" } else { "completed" }.to_string()),
            tool_args: Some(tool_input.clone()),
            tool_result: Some(sanitized_tool.content.clone()),
            tool_duration_ms: Some(duration_ms),
            effective_workdir: Some(execution_context_for_policy.workdir.display().to_string()),
            policy_decision: Some(policy_decision.to_string()),
            evidence_id: Some(policy_trace_id.clone()),
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
        stream_emitter.emit_payload(terminal_tool_payload.clone());
        super::stream_task_run_log::append_stream_event(&run_event_logger, &terminal_tool_payload)
            .await;

        // ── Attempt Ledger: transition to terminal status ──
        if denied_by_policy {
            attempt_entry.transition_blocked();
        } else if is_error {
            attempt_entry.transition_failed("execution_error".to_string(), duration_ms);
        } else {
            attempt_entry.transition_completed(duration_ms);
        }
        let _ = attempt_ledger::record_attempt(&app_data_dir, &attempt_entry);

        // Append tool_use + tool_result to session_messages.
        session_messages.push(InputMessage {
            role: "assistant".to_string(),
            content: vec![crate::modules::api::InputContentBlock::ToolUse {
                id: tool_id.clone(),
                name: tool_name.clone(),
                input: tool_input,
            }],
            thinking: None,
        });
        session_messages.push(InputMessage {
            role: "user".to_string(),
            content: vec![crate::modules::api::InputContentBlock::ToolResult {
                tool_use_id: tool_id.clone(),
                content: vec![crate::modules::api::ToolResultContentBlock::Text {
                    text: summarize_tool_result_for_model(
                        &tool_name,
                        &tool_id,
                        &wrapped_for_llm,
                        is_error,
                    ),
                }],
                is_error,
            }],
            thinking: None,
        });

        // Persist tool result in timeline order.
        timeline_session_messages.push(ConversationMessage::tool_result(
            tool_id,
            tool_name,
            sanitized_tool.content,
            is_error,
        ));
        if let Some(last_message) = timeline_session_messages.last_mut() {
            last_message.request_id = Some(provider_request_id.clone());
        }
    }

    ToolExecutionResult {
        accumulated_text,
        accumulated_thinking,
        session_messages,
        timeline_session_messages,
        has_successful_tool,
        has_successful_mutating_tool,
        force_final_response_next,
        finalization_reason,
        invalid_tool_args_streak,
        sanitized_dropped_invalid_tool_use_inputs,
        sanitize_invalid_tool_use_samples,
        tool_executor,
        pending_operation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_tool_args_repair_instruction_includes_repl_schema_and_example() {
        let registry = ToolRegistry::default();
        registry
            .register(crate::modules::tools::builtin::repl::repl_tool_entry())
            .expect("register REPL");

        let instruction = invalid_tool_args_repair_instruction(
            &registry,
            "REPL",
            "missing required parameter: code",
        );

        assert!(instruction.contains("[tool_argument_repair]"));
        assert!(instruction.contains("\"required\": ["));
        assert!(instruction.contains("\"code\""));
        assert!(instruction.contains("\"language\""));
        assert!(instruction.contains(r#""language":"shell""#));
        assert!(instruction.contains("Do not call the tool with `{}`"));
    }

    #[test]
    fn invalid_tool_args_repair_instruction_includes_file_write_example() {
        let registry = ToolRegistry::default();
        registry
            .register(crate::modules::tools::builtin::file_write::entry())
            .expect("register file_write");

        let instruction = invalid_tool_args_repair_instruction(
            &registry,
            "file_write",
            "missing required parameter: content",
        );

        assert!(instruction.contains("\"path\""));
        assert!(instruction.contains("\"content\""));
        assert!(instruction.contains("index.html"));
    }

    #[test]
    fn repair_tool_input_infers_html_file_write_path() {
        let repaired = repair_tool_input_before_validation(
            "file_write",
            &serde_json::json!({
                "content": "<!DOCTYPE html>\n<html><body>Hello</body></html>"
            }),
        )
        .expect("repairs missing html path");

        assert_eq!(repaired["path"], "index.html");
        assert!(repaired["content"]
            .as_str()
            .is_some_and(|value| value.contains("Hello")));
    }

    #[test]
    fn repair_tool_input_preserves_existing_file_write_path() {
        let repaired = repair_tool_input_before_validation(
            "file_write",
            &serde_json::json!({
                "path": "game/index.html",
                "content": "<html></html>"
            }),
        );

        assert!(repaired.is_none());
    }
}
