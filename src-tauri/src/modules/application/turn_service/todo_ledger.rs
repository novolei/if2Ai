//! Runtime guards for the durable TodoWrite ledger.
//!
//! TodoWrite is a planning ledger, not artifact evidence. These helpers keep
//! the ledger visible to the LLM and prevent a tool-required run from closing
//! as completed while the ledger still has active or pending work.

use crate::modules::application::prompt_planner::{
    PromptBlockKind, PromptBlockSource, PromptContribution,
};
use crate::modules::runtime::contracts::agent_loop::WorkLoopDecision;
use crate::modules::tools::builtin::todo_write::{load_session_todos, TodoItem};

use super::work_loop;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TodoLedgerStep {
    pub content: String,
    pub active_form: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TodoLedgerSnapshot {
    pub active: Option<TodoLedgerStep>,
    pub pending: Vec<TodoLedgerStep>,
    pub completed_count: usize,
    pub total_count: usize,
}

impl TodoLedgerSnapshot {
    #[must_use]
    pub(super) fn has_unfinished_work(&self) -> bool {
        self.active.is_some() || !self.pending.is_empty()
    }

    #[must_use]
    pub(super) fn unfinished_summary(&self) -> String {
        let active = self
            .active
            .as_ref()
            .map(|step| step.label())
            .unwrap_or_else(|| "none".to_string());
        let pending = self
            .pending
            .iter()
            .map(TodoLedgerStep::label)
            .collect::<Vec<_>>();
        if pending.is_empty() {
            format!(
                "active={active}; completed={}/{}",
                self.completed_count, self.total_count
            )
        } else {
            format!(
                "active={active}; pending={}; completed={}/{}",
                pending.join(" | "),
                self.completed_count,
                self.total_count
            )
        }
    }
}

impl TodoLedgerStep {
    #[must_use]
    fn from_item(item: &TodoItem) -> Self {
        Self {
            content: item.content().to_string(),
            active_form: item.active_form().to_string(),
            status: item.status().to_string(),
        }
    }

    #[must_use]
    fn label(&self) -> String {
        if self.active_form.trim().is_empty() {
            self.content.clone()
        } else {
            format!("{} ({})", self.content, self.active_form)
        }
    }
}

#[must_use]
pub(super) fn load_snapshot(session_id: &str) -> Option<TodoLedgerSnapshot> {
    let todos = match load_session_todos(Some(session_id)) {
        Ok(todos) => todos,
        Err(error) => {
            tracing::warn!(
                session_id,
                "[todo_ledger] failed to load durable todo ledger: {}",
                error
            );
            return None;
        }
    };
    if todos.is_empty() {
        return None;
    }
    let active = todos
        .iter()
        .find(|item| item.status() == "in_progress")
        .map(TodoLedgerStep::from_item);
    let pending = todos
        .iter()
        .filter(|item| item.status() == "pending")
        .map(TodoLedgerStep::from_item)
        .collect::<Vec<_>>();
    let completed_count = todos
        .iter()
        .filter(|item| item.status() == "completed")
        .count();
    Some(TodoLedgerSnapshot {
        active,
        pending,
        completed_count,
        total_count: todos.len(),
    })
}

#[must_use]
pub(super) fn prompt_contribution(
    work_loop_decision: &WorkLoopDecision,
    session_id: Option<&str>,
) -> Option<PromptContribution> {
    if !work_loop::requires_tool_execution_evidence(work_loop_decision) {
        return None;
    }
    let session_id = session_id?;
    let snapshot = load_snapshot(session_id)?;
    if !snapshot.has_unfinished_work() {
        return None;
    }
    Some(PromptContribution {
        kind: PromptBlockKind::CodingContext,
        title: "Todo Ledger Enforcement".to_string(),
        body: format!(
            "[todo_ledger_enforcement]\nCurrent durable TodoWrite ledger: {}.\nTreat the active todo as the next required tool-backed step. Do not mark a todo completed until a relevant non-TodoWrite tool succeeds for that step. After a real tool completes the active step, call TodoWrite to mark it completed and move the next pending step to in_progress. If a tool call fails validation, repair the arguments and retry instead of abandoning the todo.",
            snapshot.unfinished_summary()
        ),
        source: PromptBlockSource {
            subsystem: "work_loop".to_string(),
            reference: Some("turn_service.todo_ledger".to_string()),
        },
    })
}

#[must_use]
pub(super) fn post_mutation_update_message(session_id: &str) -> Option<String> {
    let snapshot = load_snapshot(session_id)?;
    if !snapshot.has_unfinished_work() {
        return None;
    }
    Some(format!(
        "[todo_ledger_step_evidence]\nA non-TodoWrite mutating tool just completed. Reconcile TodoWrite now: mark the finished active step completed only if the tool result actually satisfies it, then set the next pending step to in_progress and continue. Current ledger: {}.",
        snapshot.unfinished_summary()
    ))
}

#[must_use]
pub(super) fn terminal_status_with_todo_ledger(
    work_loop_decision: &WorkLoopDecision,
    terminal_status: Option<&'static str>,
    snapshot: Option<&TodoLedgerSnapshot>,
) -> (Option<&'static str>, Option<String>) {
    if !work_loop::requires_tool_execution_evidence(work_loop_decision) {
        return (terminal_status, None);
    }
    let Some(snapshot) = snapshot else {
        return (terminal_status, None);
    };
    if !snapshot.has_unfinished_work() {
        return (terminal_status, None);
    }
    let next_status = if matches!(
        terminal_status,
        None | Some("model_stop") | Some("model_stop_no_tools")
    ) {
        Some("todo_ledger_incomplete")
    } else {
        terminal_status
    };
    (
        next_status,
        Some(format!(
            "Todo ledger still has unfinished tool-backed steps: {}",
            snapshot.unfinished_summary()
        )),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::runtime::contracts::agent_loop::{WorkLoopDecision, WorkLoopKind};

    fn tool_required_work_loop() -> WorkLoopDecision {
        WorkLoopDecision {
            loop_kind: WorkLoopKind::AutonomousWork,
            reason_codes: vec!["tool_required_work_intent".to_string()],
            requires_confirmation: false,
            route_hint: None,
        }
    }

    fn unfinished_snapshot() -> TodoLedgerSnapshot {
        TodoLedgerSnapshot {
            active: Some(TodoLedgerStep {
                content: "Create bubble game file".to_string(),
                active_form: "Creating bubble game file".to_string(),
                status: "in_progress".to_string(),
            }),
            pending: vec![TodoLedgerStep {
                content: "Test game".to_string(),
                active_form: "Testing game".to_string(),
                status: "pending".to_string(),
            }],
            completed_count: 0,
            total_count: 2,
        }
    }

    #[test]
    fn todo_ledger_incomplete_forces_terminal_status() {
        let (status, warning) = terminal_status_with_todo_ledger(
            &tool_required_work_loop(),
            Some("model_stop"),
            Some(&unfinished_snapshot()),
        );
        assert_eq!(status, Some("todo_ledger_incomplete"));
        assert!(warning
            .as_deref()
            .is_some_and(|value| value.contains("Create bubble game file")));
    }

    #[test]
    fn todo_ledger_complete_keeps_terminal_status() {
        let snapshot = TodoLedgerSnapshot {
            active: None,
            pending: Vec::new(),
            completed_count: 2,
            total_count: 2,
        };
        let (status, warning) = terminal_status_with_todo_ledger(
            &tool_required_work_loop(),
            Some("model_stop"),
            Some(&snapshot),
        );
        assert_eq!(status, Some("model_stop"));
        assert!(warning.is_none());
    }

    #[test]
    fn todo_ledger_does_not_override_specific_failure_status() {
        let (status, warning) = terminal_status_with_todo_ledger(
            &tool_required_work_loop(),
            Some("invalid_tool_args_repeated"),
            Some(&unfinished_snapshot()),
        );
        assert_eq!(status, Some("invalid_tool_args_repeated"));
        assert!(warning.is_some());
    }
}
