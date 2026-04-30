#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

use async_trait::async_trait;

use super::budget::ContextBudget;
use super::compact::{
    compact_session, estimate_session_tokens, CompactionConfig, CompactionResult,
};
use super::config::RuntimeFeatureConfig;
use super::hooks::{HookRunResult, HookRunner};
use super::permissions::{PermissionOutcome, PermissionPolicy, PermissionPrompter};
use super::session::{ContentBlock, ConversationMessage, Session};
use super::usage::{TokenUsage, UsageTracker};
use crate::modules::api::ToolDefinition;
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::working_memory::WorkingMemory;

/// Hook invoked by [`ConversationRuntime`] after each turn / on session end.
///
/// Phase 8A.7 / v2 §0.5 Δ-8.  Memory subsystems (rolling summarizer,
/// experience extractor, ...) implement this trait so the runtime can
/// fire background jobs without `commands/agent.rs` ever knowing the
/// memory layer exists.  Implementations MUST be cheap — both methods
/// are sync `fn` so [`ConversationRuntime`] stays `Send + Sync`; any
/// long work belongs in a `tokio::spawn` inside the impl.
///
/// `messages` is borrowed from the live [`Session`] and outlives only
/// the call: implementations MUST clone what they need before spawning
/// background work.
///
/// Note (vs the openhanako reference): openhanako passes its
/// in-house `ChatMessage` type; this crate hands over
/// [`ConversationMessage`] so the hook signature stays aligned with
/// the runtime's actual session model.
pub trait TurnHook: Send + Sync {
    /// Called after every successful agent turn (one full
    /// user → assistant → optional tool loop completion).
    fn on_turn_complete(
        &self,
        scope: &MemoryExecutionScope,
        session_id: &str,
        messages: &[ConversationMessage],
    );

    /// Called once when the session is being closed / archived.
    /// Default implementation is a no-op so most hooks only need to
    /// override [`Self::on_turn_complete`].
    fn on_session_end(
        &self,
        scope: &MemoryExecutionScope,
        session_id: &str,
        messages: &[ConversationMessage],
    ) {
        let _ = (scope, session_id, messages);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiRequest {
    pub system_prompt: Vec<String>,
    pub messages: Vec<ConversationMessage>,
    /// Tool definitions to pass to the LLM. If None, no tools are sent.
    pub tools: Option<Vec<crate::modules::api::ToolDefinition>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssistantEvent {
    TextDelta(String),
    ToolUse {
        id: String,
        name: String,
        input: String,
    },
    Thinking(String),
    Usage(TokenUsage),
    /// Provider-reported `stop_reason` / `finish_reason` for the assistant turn.
    /// Mirrors the streaming path's `MessageDelta::stop_reason` (T2). Emitting
    /// this is optional — providers that omit it leave the assembled
    /// `ConversationMessage::finish_reason` as `None`.
    FinishReason(String),
    MessageStop,
}

#[async_trait]
pub trait ApiClient: Send {
    async fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError>;
}

pub trait ToolExecutor {
    /// Executes a tool by name with the given input string.
    fn execute(&mut self, tool_name: &str, input: &str) -> Result<String, ToolError>;

    /// Returns the tool definitions available for this executor.
    /// Used to populate the `tools` field in API requests.
    fn get_definitions(&self) -> Vec<ToolDefinition>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolError {
    message: String,
}

impl ToolError {
    /// Construct a [`ToolError`] from any message convertible to `String`.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for ToolError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ToolError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    ApiError(String),
    ToolError(String),
    PermissionDenied(String),
    SessionError(String),
    ConfigError(String),
    MaxIterationsExceeded,
}

impl RuntimeError {
    /// Convenience constructor for an [`ApiError`](RuntimeError::ApiError) variant.
    #[must_use]
    pub fn api_error(message: impl Into<String>) -> Self {
        Self::ApiError(message.into())
    }

    /// Convenience constructor for a [`ToolError`](RuntimeError::ToolError) variant.
    #[must_use]
    pub fn tool_error(message: impl Into<String>) -> Self {
        Self::ToolError(message.into())
    }

    /// Convenience constructor for [`PermissionDenied`](RuntimeError::PermissionDenied).
    #[must_use]
    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self::PermissionDenied(message.into())
    }

    /// Convenience constructor for [`SessionError`](RuntimeError::SessionError).
    #[must_use]
    pub fn session_error(message: impl Into<String>) -> Self {
        Self::SessionError(message.into())
    }

    /// Convenience constructor for [`ConfigError`](RuntimeError::ConfigError).
    #[must_use]
    pub fn config_error(message: impl Into<String>) -> Self {
        Self::ConfigError(message.into())
    }
}

impl Display for RuntimeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApiError(msg) => write!(f, "API error: {msg}"),
            Self::ToolError(msg) => write!(f, "Tool error: {msg}"),
            Self::PermissionDenied(msg) => write!(f, "Permission denied: {msg}"),
            Self::SessionError(msg) => write!(f, "Session error: {msg}"),
            Self::ConfigError(msg) => write!(f, "Config error: {msg}"),
            Self::MaxIterationsExceeded => write!(f, "Max iterations exceeded"),
        }
    }
}

impl std::error::Error for RuntimeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnSummary {
    pub assistant_messages: Vec<ConversationMessage>,
    pub tool_results: Vec<ConversationMessage>,
    pub iterations: usize,
    pub usage: TokenUsage,
}

/// Mutable per-turn scratch state for [`ConversationRuntime::run_turn`].
/// Extracted from inline locals (Phase 2 T6) to make the function tractable
/// and to provide a clean handle for future `RunDelegate` adoption (T10).
/// **No behavior change vs the prior inline locals — this is a pure refactor.**
#[derive(Debug, Default)]
pub(super) struct RunLoopState {
    pub assistant_messages: Vec<ConversationMessage>,
    pub tool_results: Vec<ConversationMessage>,
    pub iterations: usize,
}

pub struct ConversationRuntime<C, T> {
    session: Session,
    api_client: C,
    tool_executor: T,
    permission_policy: PermissionPolicy,
    system_prompt: Vec<String>,
    max_iterations: usize,
    context_budget: Option<ContextBudget>,
    usage_tracker: UsageTracker,
    hook_runner: HookRunner,
    /// Working memory: when `Some`, only the sliding window of recent messages
    /// is sent to the LLM. Full history is preserved in `self.session.messages`.
    working_memory: Option<WorkingMemory>,
    /// Phase 8A.7 — optional [`TurnHook`] fired after each successful turn
    /// (and intended for `on_session_end` once the runtime gains an
    /// explicit shutdown path).  Wired by `AppState` to the Phase 8B
    /// `RollingSummarizer`; tests and harness fixtures leave it `None`.
    turn_hook: Option<Arc<dyn TurnHook>>,
    /// Phase 8B.11 fix — explicit session/project context for the
    /// `TurnHook::on_turn_complete` invocation.  Without this the hook
    /// fires with `session_id="-"` (and `MemoryExecutionScope::global()`)
    /// which the [`crate::modules::memory::ticker::MemoryTicker`]
    /// silently filters out, so RollingSummarizer never sees the turn.
    /// Set via [`Self::with_session_context`] from `commands/agent.rs`.
    session_id_for_hook: Option<String>,
    project_id_for_hook: Option<String>,
    /// P2-10 — when true, each [`Self::run_turn`] snapshots `session.messages`
    /// before appending the user message; see [`Self::undo_last_checkpoint`].
    undo_checkpoints_enabled: bool,
    undo_stack: Vec<Vec<ConversationMessage>>,
    redo_stack: Vec<Vec<ConversationMessage>>,
}

impl<C, T> ConversationRuntime<C, T>
where
    C: ApiClient,
    T: ToolExecutor,
{
    /// Construct a runtime with default feature config.  Equivalent to
    /// [`Self::new_with_features`] with [`RuntimeFeatureConfig::default()`].
    #[must_use]
    pub fn new(
        session: Session,
        api_client: C,
        tool_executor: T,
        permission_policy: PermissionPolicy,
        system_prompt: Vec<String>,
    ) -> Self {
        Self::new_with_features(
            session,
            api_client,
            tool_executor,
            permission_policy,
            system_prompt,
            RuntimeFeatureConfig::default(),
        )
    }

    /// Construct a runtime with an explicit feature-flag bundle (hooks /
    /// plugins / sandbox).  Used by tests and the harness shim that need
    /// to override the default feature config.
    #[must_use]
    pub fn new_with_features(
        session: Session,
        api_client: C,
        tool_executor: T,
        permission_policy: PermissionPolicy,
        system_prompt: Vec<String>,
        feature_config: RuntimeFeatureConfig,
    ) -> Self {
        let usage_tracker = UsageTracker::from_session(&session);
        Self {
            session,
            api_client,
            tool_executor,
            permission_policy,
            system_prompt,
            max_iterations: usize::MAX,
            context_budget: None,
            usage_tracker,
            hook_runner: HookRunner::from_feature_config(&feature_config),
            working_memory: None,
            turn_hook: None,
            session_id_for_hook: None,
            project_id_for_hook: None,
            undo_checkpoints_enabled: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Attach a [`TurnHook`] that fires after every successful turn.
    ///
    /// Phase 8A.7 / v2 §0.5 Δ-8 — the production wire-up lives in
    /// `AppState` and connects [`crate::modules::memory::summary::rolling::RollingSummarizer`].
    /// The hook is invoked synchronously at the end of `run_turn`; long
    /// work belongs in a `tokio::spawn` inside the impl.
    #[must_use]
    pub fn with_turn_hook(mut self, hook: Arc<dyn TurnHook>) -> Self {
        self.turn_hook = Some(hook);
        self
    }

    /// Phase 8B.11 fix — supply the real session_id + project_id so
    /// `TurnHook::on_turn_complete` can route through the
    /// [`crate::modules::memory::scope::MemoryExecutionScope`] resolver
    /// instead of the `"-"` / `global()` fallback.  Required when the
    /// hook is connected to `MemoryTicker`, optional otherwise.
    #[must_use]
    pub fn with_session_context(
        mut self,
        session_id: impl Into<String>,
        project_id: Option<String>,
    ) -> Self {
        self.session_id_for_hook = Some(session_id.into());
        self.project_id_for_hook = project_id;
        self
    }

    /// P2-10 — enable per-turn checkpoints of `session.messages` (before user text).
    #[must_use]
    pub fn with_undo_checkpoints(mut self, enabled: bool) -> Self {
        self.undo_checkpoints_enabled = enabled;
        self
    }

    fn push_undo_checkpoint(&mut self) {
        const MAX: usize = 32;
        self.undo_stack.push(self.session.messages.clone());
        while self.undo_stack.len() > MAX {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// Restore `session.messages` to the state before the last [`Self::run_turn`]
    /// that pushed a checkpoint. Returns `false` if undo is disabled or empty.
    pub fn undo_last_checkpoint(&mut self) -> bool {
        if !self.undo_checkpoints_enabled {
            return false;
        }
        let Some(prev) = self.undo_stack.pop() else {
            return false;
        };
        let cur = std::mem::replace(&mut self.session.messages, prev);
        self.redo_stack.push(cur);
        const MAX: usize = 32;
        while self.redo_stack.len() > MAX {
            self.redo_stack.remove(0);
        }
        true
    }

    /// Re-apply the last undone message list. Returns `false` if redo is empty.
    pub fn redo_last_checkpoint(&mut self) -> bool {
        if !self.undo_checkpoints_enabled {
            return false;
        }
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };
        let cur = std::mem::replace(&mut self.session.messages, next);
        self.undo_stack.push(cur);
        const MAX: usize = 32;
        while self.undo_stack.len() > MAX {
            self.undo_stack.remove(0);
        }
        true
    }

    /// Cap the number of agent loop iterations within a single turn.
    /// Default is `usize::MAX` (no cap).
    #[must_use]
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Set the context budget for token allocation.
    ///
    /// Default: 4000 tokens, split 10/20/30/40% across
    /// System/Episodic/Semantic/Working slots.
    #[must_use]
    pub fn with_context_budget(mut self, budget: ContextBudget) -> Self {
        self.context_budget = Some(budget);
        self
    }

    /// Enable working-memory sliding-window filtering.
    ///
    /// When set, each LLM request will contain only the messages retained by
    /// `wm` (evicted by turn count and token budget) rather than the full
    /// session history. Full history is always preserved in `self.session`.
    #[must_use]
    pub fn with_working_memory(mut self, wm: WorkingMemory) -> Self {
        self.working_memory = Some(wm);
        self
    }

    /// Deprecated: use [`Self::with_context_budget`] instead.
    #[deprecated(
        since = "0.1.0",
        note = "Use with_context_budget for per-slot budget tracking"
    )]
    /// Construct a default-allocation [`ContextBudget`] from a flat total
    /// token cap.  Retained only for legacy callers; new code should call
    /// [`Self::with_context_budget`] directly.
    #[must_use]
    pub fn with_max_token_budget(mut self, max_token_budget: usize) -> Self {
        // Create a simple budget with the given total and default percentages
        self.context_budget = Some(ContextBudget::with_total(max_token_budget));
        self
    }

    /// Runs a single conversation turn with the given user message.
    pub async fn run_conversation(
        &mut self,
        user_message: String,
    ) -> Result<TurnSummary, RuntimeError> {
        self.run_turn(user_message, None).await
    }

    /// Builds the system prompt string from the configured system prompt lines.
    fn build_system_prompt(&self) -> Result<String, RuntimeError> {
        if self.system_prompt.is_empty() {
            return Err(RuntimeError::ConfigError(
                "system prompt is empty".to_string(),
            ));
        }
        Ok(self.system_prompt.join("\n"))
    }

    /// Drive one full turn of the agent loop: append the user message,
    /// query the model, dispatch tool calls, and return a [`TurnSummary`].
    /// Honours the optional [`PermissionPrompter`] for per-tool approvals.
    pub async fn run_turn(
        &mut self,
        user_input: impl Into<String> + Send,
        mut prompter: Option<&mut (dyn PermissionPrompter + Send)>,
    ) -> Result<TurnSummary, RuntimeError> {
        let user_text = user_input.into();
        if let Some(warn) = crate::modules::security::safety::shared_safety_layer()
            .scan_inbound_for_secrets(&user_text)
        {
            return Err(RuntimeError::session_error(warn));
        }
        if self.undo_checkpoints_enabled {
            self.push_undo_checkpoint();
        }
        self.session
            .messages
            .push(ConversationMessage::user_text(user_text));

        let mut state = RunLoopState::default();

        loop {
            state.iterations += 1;
            if state.iterations > self.max_iterations {
                return Err(RuntimeError::MaxIterationsExceeded);
            }

            // ContextBudget check — validates total token usage against configured budget
            // System 10%, Episodic 20%, Semantic 30%, Working 40%
            if let Some(ref budget) = self.context_budget {
                let estimated_tokens = estimate_session_tokens(&self.session);
                if estimated_tokens > budget.total {
                    return Err(RuntimeError::SessionError(format!(
                        "context budget exceeded: estimated {estimated_tokens} tokens exceeds total budget of {} (system={}, episodic={}, semantic={}, working={})",
                        budget.total,
                        budget.system_tokens(),
                        budget.episodic_tokens(),
                        budget.semantic_tokens(),
                        budget.working_tokens(),
                    )));
                }
            }

            // Build the message list for this LLM request.
            // When a WorkingMemory is configured, populate it from the current
            // session so that only the most recent turns (within token budget)
            // are sent — full history stays in `self.session.messages`.
            let messages_for_request = if let Some(ref mut wm) = self.working_memory {
                wm.clear();
                wm.extend(self.session.messages.iter().cloned());
                wm.messages().to_vec()
            } else {
                self.session.messages.clone()
            };

            let request = ApiRequest {
                system_prompt: self.system_prompt.clone(),
                messages: messages_for_request,
                tools: Some(self.tool_executor.get_definitions()),
            };
            let events = self.api_client.stream(request).await?;
            let (assistant_message, usage) = build_assistant_message(events)?;
            if let Some(usage) = usage {
                self.usage_tracker.record(usage);
            }
            let pending_tool_uses = assistant_message
                .blocks
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::ToolUse { id, name, input } => {
                        Some((id.clone(), name.clone(), input.clone()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();

            self.session.messages.push(assistant_message.clone());
            state.assistant_messages.push(assistant_message);

            if pending_tool_uses.is_empty() {
                break;
            }

            for (tool_use_id, tool_name, input) in pending_tool_uses {
                let tool_name_for_metrics = tool_name.clone();
                let permission_outcome = if let Some(prompt) = prompter.as_mut() {
                    self.permission_policy
                        .authorize(&tool_name, &input, Some(*prompt))
                } else {
                    self.permission_policy.authorize(&tool_name, &input, None)
                };

                let result_message = match permission_outcome {
                    PermissionOutcome::Allow => {
                        let pre_hook_result = self.hook_runner.run_pre_tool_use(&tool_name, &input);
                        if pre_hook_result.is_denied() {
                            let deny_message = format!("PreToolUse hook denied tool `{tool_name}`");
                            ConversationMessage::tool_result(
                                tool_use_id,
                                tool_name,
                                format_hook_message(&pre_hook_result, &deny_message),
                                true,
                            )
                        } else {
                            let (mut output, mut is_error) =
                                match self.tool_executor.execute(&tool_name, &input) {
                                    Ok(output) => (output, false),
                                    Err(error) => (error.to_string(), true),
                                };
                            output = merge_hook_feedback(pre_hook_result.messages(), output, false);

                            let post_hook_result = self
                                .hook_runner
                                .run_post_tool_use(&tool_name, &input, &output, is_error);
                            if post_hook_result.is_denied() {
                                is_error = true;
                            }
                            output = merge_hook_feedback(
                                post_hook_result.messages(),
                                output,
                                post_hook_result.is_denied(),
                            );

                            let safety = crate::modules::security::safety::shared_safety_layer();
                            let sanitized = safety.sanitize_tool_output(&tool_name, &output);
                            let wrapped_for_llm =
                                safety.wrap_for_llm(&tool_name, &sanitized.content);

                            ConversationMessage::tool_result(
                                tool_use_id,
                                tool_name,
                                wrapped_for_llm,
                                is_error,
                            )
                        }
                    }
                    PermissionOutcome::Deny { reason } => {
                        ConversationMessage::tool_result(tool_use_id, tool_name, reason, true)
                    }
                };
                let ok = result_message
                    .blocks
                    .iter()
                    .find_map(|b| {
                        if let ContentBlock::ToolResult { is_error, .. } = b {
                            Some(!*is_error)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(false);
                crate::modules::runtime::self_repair::record_tool_outcome(
                    &tool_name_for_metrics,
                    ok,
                );
                self.session.messages.push(result_message.clone());
                state.tool_results.push(result_message);
            }
        }

        // Phase 8A.7 / v2 §0.5 Δ-8 — fire the turn-complete hook so
        // memory subsystems (RollingSummarizer, …) can spawn background
        // jobs without `commands/agent.rs` reaching into them directly.
        // TODO(8B): once `runtime::session::Session` carries an `id` and
        // `project_id`, replace the global / "-" fallback with the real
        // scope so audit + dual-write attribution lines up.
        if let Some(ref hook) = self.turn_hook {
            // Phase 8B.11 fix — prefer with_session_context() values; fall
            // back to "-" / global() when the runtime was built without
            // them (test paths + legacy callers that don't yet plumb
            // session metadata through).
            let session_id = self.session_id_for_hook.as_deref().unwrap_or("-");
            let scope = MemoryExecutionScope {
                session_id: self.session_id_for_hook.clone(),
                project_id: self.project_id_for_hook.clone(),
                workdir: None,
            };
            hook.on_turn_complete(&scope, session_id, &self.session.messages);
        }

        Ok(TurnSummary {
            assistant_messages: state.assistant_messages,
            tool_results: state.tool_results,
            iterations: state.iterations,
            usage: self.usage_tracker.cumulative_usage(),
        })
    }

    /// Run synchronous compaction over the inner session and return the
    /// trimmed result without mutating `self`.
    #[must_use]
    pub fn compact(&self, config: CompactionConfig) -> CompactionResult {
        compact_session(&self.session, config)
    }

    /// Coarse running estimate of total session token usage.
    #[must_use]
    pub fn estimated_tokens(&self) -> usize {
        estimate_session_tokens(&self.session)
    }

    /// Borrow the cumulative LLM usage counters tracked across turns.
    #[must_use]
    pub fn usage(&self) -> &UsageTracker {
        &self.usage_tracker
    }

    /// Borrow the inner session (immutable view of the conversation history).
    #[must_use]
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Consume the runtime and return ownership of the inner session.
    #[must_use]
    pub fn into_session(self) -> Session {
        self.session
    }
}

fn build_assistant_message(
    events: Vec<AssistantEvent>,
) -> Result<(ConversationMessage, Option<TokenUsage>), RuntimeError> {
    let mut text = String::new();
    let mut blocks = Vec::new();
    let mut finished = false;
    let mut usage = None;
    let mut thinking = String::new();
    let mut finish_reason: Option<String> = None;

    for event in events {
        match event {
            AssistantEvent::TextDelta(delta) => text.push_str(&delta),
            AssistantEvent::ToolUse { id, name, input } => {
                flush_text_block(&mut text, &mut blocks);
                blocks.push(ContentBlock::ToolUse { id, name, input });
            }
            AssistantEvent::Thinking(content) => {
                if !thinking.is_empty() {
                    thinking.push('\n');
                }
                thinking.push_str(&content);
            }
            AssistantEvent::Usage(value) => usage = Some(value),
            AssistantEvent::FinishReason(reason) => {
                // Last-write-wins, mirrors streaming path's
                // `record_finish_reason_from_delta` (T2). Empty strings are
                // treated as a meaningful provider signal — we only skip the
                // event entirely when the provider omits it.
                finish_reason = Some(reason);
            }
            AssistantEvent::MessageStop => {
                finished = true;
            }
        }
    }

    flush_text_block(&mut text, &mut blocks);

    if !finished {
        return Err(RuntimeError::ApiError(
            "assistant stream ended without a message stop event".to_string(),
        ));
    }
    if blocks.is_empty() && thinking.is_empty() {
        return Err(RuntimeError::ApiError(
            "assistant stream produced no content".to_string(),
        ));
    }

    let thinking_result = if thinking.is_empty() {
        None
    } else {
        Some(thinking)
    };

    Ok((
        ConversationMessage {
            role: crate::modules::runtime::session::MessageRole::Assistant,
            blocks,
            usage,
            thinking: thinking_result,
            task_outcome: None,
            degraded_reason: None,
            resume_available: None,
            resume_cursor: None,
            request_id: None,
            finish_reason,
        },
        usage,
    ))
}

fn flush_text_block(text: &mut String, blocks: &mut Vec<ContentBlock>) {
    if !text.is_empty() {
        blocks.push(ContentBlock::Text {
            text: std::mem::take(text),
        });
    }
}

fn format_hook_message(result: &HookRunResult, fallback: &str) -> String {
    if result.messages().is_empty() {
        fallback.to_string()
    } else {
        result.messages().join("\n")
    }
}

fn merge_hook_feedback(messages: &[String], output: String, denied: bool) -> String {
    if messages.is_empty() {
        return output;
    }

    let mut sections = Vec::new();
    if !output.trim().is_empty() {
        sections.push(output);
    }
    let label = if denied {
        "Hook feedback (denied)"
    } else {
        "Hook feedback"
    };
    sections.push(format!("{label}:\n{}", messages.join("\n")));
    sections.join("\n\n")
}

type ToolHandler = Box<dyn FnMut(&str) -> Result<String, ToolError>>;

#[derive(Default)]
pub struct StaticToolExecutor {
    handlers: BTreeMap<String, ToolHandler>,
}

impl StaticToolExecutor {
    /// Build an empty executor with no registered handlers.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a handler closure under `tool_name`.  Builder-style:
    /// returns `self` so multiple `register(...)` calls can be chained.
    #[must_use]
    pub fn register(
        mut self,
        tool_name: impl Into<String>,
        handler: impl FnMut(&str) -> Result<String, ToolError> + 'static,
    ) -> Self {
        self.handlers.insert(tool_name.into(), Box::new(handler));
        self
    }
}

impl ToolExecutor for StaticToolExecutor {
    fn execute(&mut self, tool_name: &str, input: &str) -> Result<String, ToolError> {
        self.handlers
            .get_mut(tool_name)
            .ok_or_else(|| ToolError::new(format!("unknown tool: {tool_name}")))?(input)
    }

    fn get_definitions(&self) -> Vec<ToolDefinition> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use crate::modules::runtime::compact::CompactionConfig;
    use crate::modules::runtime::config::{RuntimeFeatureConfig, RuntimeHookConfig};
    use crate::modules::runtime::conversation::{
        ApiClient, ApiRequest, AssistantEvent, ConversationRuntime, RuntimeError,
        StaticToolExecutor,
    };
    use crate::modules::runtime::permissions::{
        PermissionMode, PermissionPolicy, PermissionPromptDecision, PermissionPrompter,
        PermissionRequest,
    };
    use crate::modules::runtime::prompt::{ProjectContext, SystemPromptBuilder};
    use crate::modules::runtime::session::{ContentBlock, MessageRole, Session};
    use crate::modules::runtime::usage::TokenUsage;
    use std::path::PathBuf;

    struct ScriptedApiClient {
        call_count: usize,
    }

    #[async_trait]
    impl ApiClient for ScriptedApiClient {
        async fn stream(
            &mut self,
            request: ApiRequest,
        ) -> Result<Vec<AssistantEvent>, RuntimeError> {
            self.call_count += 1;
            match self.call_count {
                1 => {
                    assert!(request
                        .messages
                        .iter()
                        .any(|message| message.role == MessageRole::User));
                    Ok(vec![
                        AssistantEvent::TextDelta("Let me calculate that.".to_string()),
                        AssistantEvent::ToolUse {
                            id: "tool-1".to_string(),
                            name: "add".to_string(),
                            input: "2,2".to_string(),
                        },
                        AssistantEvent::Usage(TokenUsage {
                            input_tokens: 20,
                            output_tokens: 6,
                            cache_creation_input_tokens: 1,
                            cache_read_input_tokens: 2,
                        }),
                        AssistantEvent::MessageStop,
                    ])
                }
                2 => {
                    let last_message = request
                        .messages
                        .last()
                        .expect("tool result should be present");
                    assert_eq!(last_message.role, MessageRole::Tool);
                    Ok(vec![
                        AssistantEvent::TextDelta("The answer is 4.".to_string()),
                        AssistantEvent::Usage(TokenUsage {
                            input_tokens: 24,
                            output_tokens: 4,
                            cache_creation_input_tokens: 1,
                            cache_read_input_tokens: 3,
                        }),
                        AssistantEvent::MessageStop,
                    ])
                }
                _ => Err(RuntimeError::ApiError(
                    "unexpected extra API call".to_string(),
                )),
            }
        }
    }

    struct PromptAllowOnce;

    impl PermissionPrompter for PromptAllowOnce {
        fn decide(&mut self, request: &PermissionRequest) -> PermissionPromptDecision {
            assert_eq!(request.tool_name, "add");
            PermissionPromptDecision::Allow
        }
    }

    #[tokio::test]
    async fn runs_user_to_tool_to_result_loop_end_to_end_and_tracks_usage() {
        let api_client = ScriptedApiClient { call_count: 0 };
        let tool_executor = StaticToolExecutor::new().register("add", |input| {
            let total = input
                .split(',')
                .map(|part| part.parse::<i32>().expect("input must be valid integer"))
                .sum::<i32>();
            Ok(total.to_string())
        });
        let permission_policy = PermissionPolicy::new(PermissionMode::WorkspaceWrite);
        let system_prompt = SystemPromptBuilder::new()
            .with_project_context(ProjectContext {
                cwd: PathBuf::from("/tmp/project"),
                current_date: "2026-03-31".to_string(),
                git_status: None,
                git_diff: None,
                instruction_files: Vec::new(),
            })
            .with_os("linux", "6.8")
            .build();
        let mut runtime = ConversationRuntime::new(
            Session::new(),
            api_client,
            tool_executor,
            permission_policy,
            system_prompt,
        );

        let summary = runtime
            .run_turn("what is 2 + 2?", Some(&mut PromptAllowOnce))
            .await
            .expect("conversation loop should succeed");

        assert_eq!(summary.iterations, 2);
        assert_eq!(summary.assistant_messages.len(), 2);
        assert_eq!(summary.tool_results.len(), 1);
        assert_eq!(runtime.session().messages.len(), 4);
        assert_eq!(summary.usage.output_tokens, 10);
        assert!(matches!(
            runtime.session().messages[1].blocks[1],
            ContentBlock::ToolUse { .. }
        ));
        assert!(matches!(
            runtime.session().messages[2].blocks[0],
            ContentBlock::ToolResult {
                is_error: false,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn records_denied_tool_results_when_prompt_rejects() {
        struct RejectPrompter;
        impl PermissionPrompter for RejectPrompter {
            fn decide(&mut self, _request: &PermissionRequest) -> PermissionPromptDecision {
                PermissionPromptDecision::Deny {
                    reason: "not now".to_string(),
                }
            }
        }

        struct SingleCallApiClient;
        #[async_trait]
        impl ApiClient for SingleCallApiClient {
            async fn stream(
                &mut self,
                request: ApiRequest,
            ) -> Result<Vec<AssistantEvent>, RuntimeError> {
                if request
                    .messages
                    .iter()
                    .any(|message| message.role == MessageRole::Tool)
                {
                    return Ok(vec![
                        AssistantEvent::TextDelta("I could not use the tool.".to_string()),
                        AssistantEvent::MessageStop,
                    ]);
                }
                Ok(vec![
                    AssistantEvent::ToolUse {
                        id: "tool-1".to_string(),
                        name: "blocked".to_string(),
                        input: "secret".to_string(),
                    },
                    AssistantEvent::MessageStop,
                ])
            }
        }

        let mut runtime = ConversationRuntime::new(
            Session::new(),
            SingleCallApiClient,
            StaticToolExecutor::new(),
            PermissionPolicy::new(PermissionMode::WorkspaceWrite),
            vec!["system".to_string()],
        );

        let summary = runtime
            .run_turn("use the tool", Some(&mut RejectPrompter))
            .await
            .expect("conversation should continue after denied tool");

        assert_eq!(summary.tool_results.len(), 1);
        assert!(matches!(
            &summary.tool_results[0].blocks[0],
            ContentBlock::ToolResult { is_error: true, output, .. } if output == "not now"
        ));
    }

    #[tokio::test]
    async fn denies_tool_use_when_pre_tool_hook_blocks() {
        struct SingleCallApiClient;
        #[async_trait]
        impl ApiClient for SingleCallApiClient {
            async fn stream(
                &mut self,
                request: ApiRequest,
            ) -> Result<Vec<AssistantEvent>, RuntimeError> {
                if request
                    .messages
                    .iter()
                    .any(|message| message.role == MessageRole::Tool)
                {
                    return Ok(vec![
                        AssistantEvent::TextDelta("blocked".to_string()),
                        AssistantEvent::MessageStop,
                    ]);
                }
                Ok(vec![
                    AssistantEvent::ToolUse {
                        id: "tool-1".to_string(),
                        name: "blocked".to_string(),
                        input: r#"{"path":"secret.txt"}"#.to_string(),
                    },
                    AssistantEvent::MessageStop,
                ])
            }
        }

        let mut runtime = ConversationRuntime::new_with_features(
            Session::new(),
            SingleCallApiClient,
            StaticToolExecutor::new().register("blocked", |_input| {
                panic!("tool should not execute when hook denies")
            }),
            PermissionPolicy::new(PermissionMode::DangerFullAccess),
            vec!["system".to_string()],
            RuntimeFeatureConfig::default().with_hooks(RuntimeHookConfig::new(
                vec![shell_snippet("printf 'blocked by hook'; exit 2")],
                Vec::new(),
            )),
        );

        let summary = runtime
            .run_turn("use the tool", None)
            .await
            .expect("conversation should continue after hook denial");

        assert_eq!(summary.tool_results.len(), 1);
        let ContentBlock::ToolResult {
            is_error, output, ..
        } = &summary.tool_results[0].blocks[0]
        else {
            panic!("expected tool result block");
        };
        assert!(
            *is_error,
            "hook denial should produce an error result: {output}"
        );
        assert!(
            output.contains("denied tool") || output.contains("blocked by hook"),
            "unexpected hook denial output: {output:?}"
        );
    }

    #[tokio::test]
    async fn appends_post_tool_hook_feedback_to_tool_result() {
        struct TwoCallApiClient {
            calls: usize,
        }

        #[async_trait]
        impl ApiClient for TwoCallApiClient {
            async fn stream(
                &mut self,
                request: ApiRequest,
            ) -> Result<Vec<AssistantEvent>, RuntimeError> {
                self.calls += 1;
                match self.calls {
                    1 => Ok(vec![
                        AssistantEvent::ToolUse {
                            id: "tool-1".to_string(),
                            name: "add".to_string(),
                            input: r#"{"lhs":2,"rhs":2}"#.to_string(),
                        },
                        AssistantEvent::MessageStop,
                    ]),
                    2 => {
                        assert!(request
                            .messages
                            .iter()
                            .any(|message| message.role == MessageRole::Tool));
                        Ok(vec![
                            AssistantEvent::TextDelta("done".to_string()),
                            AssistantEvent::MessageStop,
                        ])
                    }
                    _ => Err(RuntimeError::ApiError(
                        "unexpected extra API call".to_string(),
                    )),
                }
            }
        }

        let mut runtime = ConversationRuntime::new_with_features(
            Session::new(),
            TwoCallApiClient { calls: 0 },
            StaticToolExecutor::new().register("add", |_input| Ok("4".to_string())),
            PermissionPolicy::new(PermissionMode::DangerFullAccess),
            vec!["system".to_string()],
            RuntimeFeatureConfig::default().with_hooks(RuntimeHookConfig::new(
                vec![shell_snippet("printf 'pre hook ran'")],
                vec![shell_snippet("printf 'post hook ran'")],
            )),
        );

        let summary = runtime
            .run_turn("use add", None)
            .await
            .expect("tool loop succeeds");

        assert_eq!(summary.tool_results.len(), 1);
        let ContentBlock::ToolResult {
            is_error, output, ..
        } = &summary.tool_results[0].blocks[0]
        else {
            panic!("expected tool result block");
        };
        assert!(
            !*is_error,
            "post hook should preserve non-error result: {output:?}"
        );
        assert!(
            output.contains('4'),
            "tool output missing value: {output:?}"
        );
        assert!(
            output.contains("pre hook ran"),
            "tool output missing pre hook feedback: {output:?}"
        );
        assert!(
            output.contains("post hook ran"),
            "tool output missing post hook feedback: {output:?}"
        );
    }

    #[tokio::test]
    async fn reconstructs_usage_tracker_from_restored_session() {
        struct SimpleApi;
        #[async_trait]
        impl ApiClient for SimpleApi {
            async fn stream(
                &mut self,
                _request: ApiRequest,
            ) -> Result<Vec<AssistantEvent>, RuntimeError> {
                Ok(vec![
                    AssistantEvent::TextDelta("done".to_string()),
                    AssistantEvent::MessageStop,
                ])
            }
        }

        let mut session = Session::new();
        session.messages.push(
            crate::modules::runtime::session::ConversationMessage::assistant_with_usage(
                vec![ContentBlock::Text {
                    text: "earlier".to_string(),
                }],
                Some(TokenUsage {
                    input_tokens: 11,
                    output_tokens: 7,
                    cache_creation_input_tokens: 2,
                    cache_read_input_tokens: 1,
                }),
            ),
        );

        let runtime = ConversationRuntime::new(
            session,
            SimpleApi,
            StaticToolExecutor::new(),
            PermissionPolicy::new(PermissionMode::DangerFullAccess),
            vec!["system".to_string()],
        );

        assert_eq!(runtime.usage().turns(), 1);
        assert_eq!(runtime.usage().cumulative_usage().total_tokens(), 21);
    }

    #[tokio::test]
    async fn compacts_session_after_turns() {
        struct SimpleApi;
        #[async_trait]
        impl ApiClient for SimpleApi {
            async fn stream(
                &mut self,
                _request: ApiRequest,
            ) -> Result<Vec<AssistantEvent>, RuntimeError> {
                Ok(vec![
                    AssistantEvent::TextDelta("done".to_string()),
                    AssistantEvent::MessageStop,
                ])
            }
        }

        let mut runtime = ConversationRuntime::new(
            Session::new(),
            SimpleApi,
            StaticToolExecutor::new(),
            PermissionPolicy::new(PermissionMode::DangerFullAccess),
            vec!["system".to_string()],
        );
        runtime.run_turn("a", None).await.expect("turn a");
        runtime.run_turn("b", None).await.expect("turn b");
        runtime.run_turn("c", None).await.expect("turn c");

        let result = runtime.compact(CompactionConfig {
            preserve_recent_messages: 2,
            max_estimated_tokens: 1,
        });
        assert!(result.summary.contains("Conversation summary"));
        assert_eq!(
            result.compacted_session.messages[0].role,
            MessageRole::System
        );
    }

    #[cfg(windows)]
    fn shell_snippet(script: &str) -> String {
        script.replace('\'', "\"")
    }

    #[cfg(not(windows))]
    fn shell_snippet(script: &str) -> String {
        script.to_string()
    }

    /// Phase 8A.7 — `with_turn_hook` attaches a [`TurnHook`] that
    /// fires once per successful `run_turn`.  We use a counting hook
    /// to assert exactly one invocation per turn and that the hook
    /// observes the in-flight session messages.
    #[tokio::test]
    async fn turn_hook_fires_once_per_turn() {
        use crate::modules::memory::scope::MemoryExecutionScope;
        use crate::modules::runtime::conversation::TurnHook;
        use crate::modules::runtime::session::ConversationMessage;
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Arc;

        struct CountingHook {
            count: AtomicU32,
            last_seen: std::sync::Mutex<usize>,
        }

        impl TurnHook for CountingHook {
            fn on_turn_complete(
                &self,
                _scope: &MemoryExecutionScope,
                _session_id: &str,
                messages: &[ConversationMessage],
            ) {
                self.count.fetch_add(1, Ordering::SeqCst);
                if let Ok(mut guard) = self.last_seen.lock() {
                    *guard = messages.len();
                }
            }
        }

        struct SimpleApi;
        #[async_trait]
        impl ApiClient for SimpleApi {
            async fn stream(
                &mut self,
                _request: ApiRequest,
            ) -> Result<Vec<AssistantEvent>, RuntimeError> {
                Ok(vec![
                    AssistantEvent::TextDelta("ok".to_string()),
                    AssistantEvent::MessageStop,
                ])
            }
        }

        let hook = Arc::new(CountingHook {
            count: AtomicU32::new(0),
            last_seen: std::sync::Mutex::new(0),
        });
        let mut runtime = ConversationRuntime::new(
            Session::new(),
            SimpleApi,
            StaticToolExecutor::new(),
            PermissionPolicy::new(PermissionMode::DangerFullAccess),
            vec!["system".to_string()],
        )
        .with_turn_hook(hook.clone());

        runtime.run_turn("first", None).await.expect("turn 1");
        runtime.run_turn("second", None).await.expect("turn 2");

        assert_eq!(hook.count.load(Ordering::SeqCst), 2);
        let observed = *hook.last_seen.lock().expect("lock");
        assert!(
            observed >= 4,
            "hook should observe >=4 messages after two turns, got {observed}"
        );
    }

    /// Verifies that `with_working_memory` limits the messages sent to the LLM.
    ///
    /// The session is pre-filled with many turns; the API client records how many
    /// messages each request contains.  After one more turn we assert that the
    /// LLM saw at most `max_turns` messages — not the full session history.
    #[tokio::test]
    async fn working_memory_limits_messages_sent_to_llm() {
        use crate::modules::memory::working_memory::WorkingMemory;
        use std::sync::{Arc, Mutex};

        let counts: Arc<Mutex<Vec<usize>>> = Arc::new(Mutex::new(Vec::new()));

        struct MessageCountingApi {
            counts: Arc<Mutex<Vec<usize>>>,
        }

        #[async_trait]
        impl ApiClient for MessageCountingApi {
            async fn stream(
                &mut self,
                request: ApiRequest,
            ) -> Result<Vec<AssistantEvent>, RuntimeError> {
                self.counts
                    .lock()
                    .expect("lock poisoned")
                    .push(request.messages.len());
                Ok(vec![
                    AssistantEvent::TextDelta("ok".to_string()),
                    AssistantEvent::MessageStop,
                ])
            }
        }

        // Build a session with 10 existing messages (5 user + 5 assistant pairs).
        let mut session = Session::new();
        for i in 0..5_u8 {
            session.messages.push(
                crate::modules::runtime::session::ConversationMessage::user_text(format!(
                    "msg {i}"
                )),
            );
            session.messages.push(
                crate::modules::runtime::session::ConversationMessage::assistant(vec![
                    crate::modules::runtime::session::ContentBlock::Text {
                        text: format!("reply {i}"),
                    },
                ]),
            );
        }

        // Window of 4 turns max.
        let wm = WorkingMemory::new(4, 100_000);
        let mut runtime = ConversationRuntime::new(
            session,
            MessageCountingApi {
                counts: Arc::clone(&counts),
            },
            StaticToolExecutor::new(),
            PermissionPolicy::new(
                crate::modules::runtime::permissions::PermissionMode::DangerFullAccess,
            ),
            vec!["system".to_string()],
        )
        .with_working_memory(wm);

        runtime
            .run_turn("new question".to_string(), None)
            .await
            .unwrap();

        // Full session has 10 pre-existing + 1 new user = 11 messages,
        // but the API should only have seen ≤ 4 (the working memory window).
        let snapshot = counts.lock().expect("lock poisoned");
        let sent = snapshot.first().copied().unwrap_or(0);
        assert!(
            sent <= 4,
            "expected ≤ 4 messages sent to LLM (working memory limit), got {sent}"
        );
    }

    struct OnceTextApi;

    #[async_trait]
    impl ApiClient for OnceTextApi {
        async fn stream(
            &mut self,
            _request: ApiRequest,
        ) -> Result<Vec<AssistantEvent>, RuntimeError> {
            Ok(vec![
                AssistantEvent::TextDelta("x".into()),
                AssistantEvent::MessageStop,
            ])
        }
    }

    #[tokio::test]
    async fn undo_checkpoint_restores_messages_before_last_turn() {
        let permission_policy = PermissionPolicy::new(PermissionMode::WorkspaceWrite);
        let system_prompt = SystemPromptBuilder::new()
            .with_project_context(ProjectContext {
                cwd: PathBuf::from("/tmp/project"),
                current_date: "2026-03-31".to_string(),
                git_status: None,
                git_diff: None,
                instruction_files: Vec::new(),
            })
            .with_os("linux", "6.8")
            .build();
        let mut runtime = ConversationRuntime::new(
            Session::new(),
            OnceTextApi,
            StaticToolExecutor::new(),
            permission_policy,
            system_prompt,
        )
        .with_undo_checkpoints(true);

        runtime.run_turn("hi", None).await.expect("turn");
        assert_eq!(runtime.session().messages.len(), 2);
        assert!(runtime.undo_last_checkpoint());
        assert!(runtime.session().messages.is_empty());
        assert!(runtime.redo_last_checkpoint());
        assert_eq!(runtime.session().messages.len(), 2);
    }
}
