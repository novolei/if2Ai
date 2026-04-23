//! Session history paging and replay from the canonical run event log.
//!
//! MIG-018 keeps `session.json` as a compatibility fallback only.
//! The replay path in this module reads append-only run log entries
//! and builds a UI-oriented projection from those facts.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::modules::runtime::event_log::RunLogEntry;

const DEFAULT_PAGE_LIMIT: usize = 100;
const MAX_PAGE_LIMIT: usize = 500;

/// One page of durable run-log entries for a session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionHistoryEventPage {
    pub session_id: String,
    pub entries: Vec<RunLogEntry>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

/// UI-oriented replay of one conversation message family.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryReplayMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub run_id: String,
    pub occurred_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_outcome: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume_cursor: Option<String>,
}

/// Stable projection reconstructed from a batch of event-log entries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionHistoryReplay {
    pub session_id: String,
    pub messages: Vec<HistoryReplayMessage>,
    pub run_count: usize,
    pub event_count: usize,
}

#[derive(Debug, Default)]
struct ReplayRun {
    run_id: String,
    first_seen_at: String,
    first_seen_seq: u64,
    user_content: Option<String>,
    assistant_text: String,
    thinking: String,
    assistant_first_seen_at: Option<String>,
    assistant_first_seen_seq: u64,
    tools: BTreeMap<String, ReplayTool>,
    status: Option<String>,
    task_outcome: Option<String>,
    degraded_reason: Option<String>,
    resume_cursor: Option<String>,
}

#[derive(Debug, Default)]
struct ReplayTool {
    tool_call_id: String,
    tool_name: Option<String>,
    tool_status: Option<String>,
    content: String,
    first_seen_at: String,
    first_seen_seq: u64,
}

/// Read one page of run-log events for `session_id`.
pub fn read_session_history_event_page(
    base_dir: impl AsRef<Path>,
    session_id: &str,
    limit: Option<usize>,
    cursor: Option<&str>,
) -> io::Result<SessionHistoryEventPage> {
    let limit = limit.unwrap_or(DEFAULT_PAGE_LIMIT).clamp(1, MAX_PAGE_LIMIT);
    let start = parse_cursor(cursor)?;
    let entries = read_all_session_entries(base_dir, session_id)?;
    let page_entries = entries
        .iter()
        .skip(start)
        .take(limit)
        .cloned()
        .collect::<Vec<_>>();
    let next_offset = start.saturating_add(page_entries.len());
    let has_more = next_offset < entries.len();

    Ok(SessionHistoryEventPage {
        session_id: session_id.to_string(),
        entries: page_entries,
        next_cursor: has_more.then(|| format!("offset:{next_offset}")),
        has_more,
    })
}

/// Replay event-log entries into a stable conversation projection.
#[must_use]
pub fn replay_session_history(
    session_id: impl Into<String>,
    entries: &[RunLogEntry],
) -> SessionHistoryReplay {
    let session_id = session_id.into();
    let mut runs: BTreeMap<String, ReplayRun> = BTreeMap::new();

    for entry in entries {
        apply_entry(&mut runs, entry);
    }

    let run_count = runs.len();
    let mut ordered_messages = Vec::new();
    for run in runs.into_values() {
        if let Some(user_content) = run.user_content {
            ordered_messages.push((
                run.first_seen_at.clone(),
                run.first_seen_seq,
                0_u8,
                format!("history-user-{}", run.run_id),
                HistoryReplayMessage {
                    id: format!("history-user-{}", run.run_id),
                    role: "user".to_string(),
                    content: user_content,
                    run_id: run.run_id.clone(),
                    occurred_at: run.first_seen_at.clone(),
                    thinking: None,
                    tool_call_id: None,
                    tool_name: None,
                    tool_status: None,
                    task_outcome: None,
                    degraded_reason: None,
                    resume_cursor: None,
                },
            ));
        }

        if !run.assistant_text.is_empty()
            || !run.thinking.is_empty()
            || run.status.is_some()
            || run.task_outcome.is_some()
        {
            let occurred_at = run
                .assistant_first_seen_at
                .clone()
                .unwrap_or_else(|| run.first_seen_at.clone());
            ordered_messages.push((
                occurred_at.clone(),
                run.assistant_first_seen_seq,
                1_u8,
                format!("history-assistant-{}", run.run_id),
                HistoryReplayMessage {
                    id: format!("history-assistant-{}", run.run_id),
                    role: "assistant".to_string(),
                    content: run.assistant_text,
                    run_id: run.run_id.clone(),
                    occurred_at,
                    thinking: (!run.thinking.is_empty()).then_some(run.thinking),
                    tool_call_id: None,
                    tool_name: None,
                    tool_status: None,
                    task_outcome: run.task_outcome.clone(),
                    degraded_reason: run.degraded_reason.clone(),
                    resume_cursor: run.resume_cursor.clone(),
                },
            ));
        }

        for tool in run.tools.into_values() {
            let id = format!("history-tool-{}-{}", run.run_id, tool.tool_call_id);
            ordered_messages.push((
                tool.first_seen_at.clone(),
                tool.first_seen_seq,
                2_u8,
                id.clone(),
                HistoryReplayMessage {
                    id,
                    role: "tool".to_string(),
                    content: tool.content,
                    run_id: run.run_id.clone(),
                    occurred_at: tool.first_seen_at,
                    thinking: None,
                    tool_call_id: Some(tool.tool_call_id),
                    tool_name: tool.tool_name,
                    tool_status: tool.tool_status,
                    task_outcome: run.task_outcome.clone(),
                    degraded_reason: run.degraded_reason.clone(),
                    resume_cursor: run.resume_cursor.clone(),
                },
            ));
        }
    }
    ordered_messages.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| left.3.cmp(&right.3))
    });
    let messages = ordered_messages
        .into_iter()
        .map(|(_, _, _, _, message)| message)
        .collect();

    SessionHistoryReplay {
        session_id,
        run_count,
        event_count: entries.len(),
        messages,
    }
}

fn read_all_session_entries(
    base_dir: impl AsRef<Path>,
    session_id: &str,
) -> io::Result<Vec<RunLogEntry>> {
    let session_dir = run_log_session_dir(base_dir.as_ref(), session_id);
    if !session_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in std::fs::read_dir(&session_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) == Some("jsonl") {
            files.push(path);
        }
    }
    files.sort();

    let mut entries = Vec::new();
    for path in files {
        let raw = std::fs::read_to_string(&path)?;
        for (line_index, line) in raw.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let entry = serde_json::from_str::<RunLogEntry>(line).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "parse run log entry {}:{}: {error}",
                        path.display(),
                        line_index + 1
                    ),
                )
            })?;
            entries.push(entry);
        }
    }

    entries.sort_by(|left, right| {
        left.occurred_at
            .cmp(&right.occurred_at)
            .then_with(|| left.run_id.cmp(&right.run_id))
            .then_with(|| left.seq.cmp(&right.seq))
            .then_with(|| left.event_id.cmp(&right.event_id))
    });
    Ok(entries)
}

fn run_log_session_dir(base_dir: &Path, session_id: &str) -> std::path::PathBuf {
    base_dir.join("runtime").join("run-log").join(session_id)
}

fn parse_cursor(cursor: Option<&str>) -> io::Result<usize> {
    let Some(cursor) = cursor else {
        return Ok(0);
    };
    let Some(raw) = cursor.strip_prefix("offset:") else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid history cursor: {cursor}"),
        ));
    };
    raw.parse::<usize>()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))
}

fn apply_entry(runs: &mut BTreeMap<String, ReplayRun>, entry: &RunLogEntry) {
    let run = runs
        .entry(entry.run_id.clone())
        .or_insert_with(|| ReplayRun {
            run_id: entry.run_id.clone(),
            first_seen_at: entry.occurred_at.clone(),
            first_seen_seq: entry.seq,
            ..ReplayRun::default()
        });

    match entry.event_type.as_str() {
        "run_started" => {
            run.user_content = json_string(&entry.payload, "message_preview")
                .filter(|value| !value.is_empty())
                .or_else(|| Some("[message unavailable in run log]".to_string()));
        }
        "text_delta" => {
            mark_assistant_seen(run, entry);
            if let Some(text) = json_string(&entry.payload, "text") {
                run.assistant_text.push_str(&text);
            }
        }
        "thinking_started" | "thinking_start" => {
            mark_assistant_seen(run, entry);
        }
        "thinking_delta" => {
            mark_assistant_seen(run, entry);
            if let Some(thinking) = json_string(&entry.payload, "thinking") {
                run.thinking.push_str(&thinking);
            }
        }
        "final_text_override" => {
            mark_assistant_seen(run, entry);
            run.assistant_text = json_string(&entry.payload, "text").unwrap_or_default();
        }
        "tool_call_queued"
        | "tool_call_running"
        | "tool_call_completed"
        | "tool_call_failed"
        | "tool_call_update" => apply_tool_event(run, entry),
        "stream_complete" => {
            mark_assistant_seen(run, entry);
            run.status = Some("completed".to_string());
            run.task_outcome = json_string(&entry.payload, "task_outcome");
            run.degraded_reason = json_string(&entry.payload, "degraded_reason");
            run.resume_cursor = json_string(&entry.payload, "resume_cursor");
        }
        "stream_error" => {
            mark_assistant_seen(run, entry);
            run.status = Some("failed".to_string());
            run.task_outcome =
                json_string(&entry.payload, "task_outcome").or_else(|| Some("failed".to_string()));
            run.degraded_reason = json_string(&entry.payload, "degraded_reason")
                .or_else(|| json_string(&entry.payload, "tool_result"))
                .or_else(|| json_string(&entry.payload, "reason"));
            run.resume_cursor = json_string(&entry.payload, "resume_cursor");
        }
        _ => {}
    }
}

fn mark_assistant_seen(run: &mut ReplayRun, entry: &RunLogEntry) {
    if run.assistant_first_seen_at.is_none() {
        run.assistant_first_seen_at = Some(entry.occurred_at.clone());
        run.assistant_first_seen_seq = entry.seq;
    }
}

fn apply_tool_event(run: &mut ReplayRun, entry: &RunLogEntry) {
    let tool_call_id = json_string(&entry.payload, "tool_call_id")
        .or_else(|| entry.tool_call_id.clone())
        .unwrap_or_else(|| format!("tool-{}", entry.seq));
    let tool = run
        .tools
        .entry(tool_call_id.clone())
        .or_insert_with(|| ReplayTool {
            tool_call_id,
            first_seen_at: entry.occurred_at.clone(),
            first_seen_seq: entry.seq,
            ..ReplayTool::default()
        });
    tool.tool_name = json_string(&entry.payload, "tool_name").or_else(|| tool.tool_name.clone());
    tool.tool_status =
        json_string(&entry.payload, "tool_status").or_else(|| tool.tool_status.clone());
    if let Some(result) = json_string(&entry.payload, "tool_result") {
        tool.content = result;
    }
}

fn json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::runtime::event_log::RunEventLogger;
    use std::collections::BTreeSet;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_root(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        std::env::temp_dir().join(format!("if2ai-history-{label}-{nanos}"))
    }

    fn log_entry(
        run_id: &str,
        seq: u64,
        event_type: &str,
        occurred_at: &str,
        payload: serde_json::Value,
    ) -> RunLogEntry {
        RunLogEntry {
            event_id: format!("{run_id}-{seq}"),
            session_id: "session-order".to_string(),
            run_id: run_id.to_string(),
            seq,
            event_type: event_type.to_string(),
            occurred_at: occurred_at.to_string(),
            payload,
            causation_id: None,
            correlation_id: None,
            tool_call_id: None,
            attempt_id: None,
        }
    }

    #[tokio::test]
    async fn history_paging_has_no_duplicates_or_gaps() {
        let root = unique_temp_root("paging");
        let logger = RunEventLogger::for_base_dir(&root, "session-1", "run-1");
        for idx in 0..5 {
            logger
                .append("text_delta", serde_json::json!({ "text": idx.to_string() }))
                .await;
        }

        let first =
            read_session_history_event_page(&root, "session-1", Some(2), None).expect("first page");
        let second = read_session_history_event_page(
            &root,
            "session-1",
            Some(2),
            first.next_cursor.as_deref(),
        )
        .expect("second page");
        let third = read_session_history_event_page(
            &root,
            "session-1",
            Some(2),
            second.next_cursor.as_deref(),
        )
        .expect("third page");

        assert!(first.has_more);
        assert!(second.has_more);
        assert!(!third.has_more);

        let all = first
            .entries
            .iter()
            .chain(second.entries.iter())
            .chain(third.entries.iter())
            .map(|entry| entry.event_id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(all.len(), 5);
    }

    #[tokio::test]
    async fn history_replay_is_stable_for_same_events() {
        let root = unique_temp_root("replay");
        let logger = RunEventLogger::for_base_dir(&root, "session-2", "run-1");
        logger
            .append(
                "run_started",
                serde_json::json!({ "message_preview": "hello" }),
            )
            .await;
        logger
            .append("thinking_delta", serde_json::json!({ "thinking": "plan" }))
            .await;
        logger
            .append("text_delta", serde_json::json!({ "text": "answer" }))
            .await;
        logger
            .append(
                "tool_call_completed",
                serde_json::json!({
                    "tool_call_id": "tool-1",
                    "tool_name": "bash",
                    "tool_status": "completed",
                    "tool_result": "ok"
                }),
            )
            .await;
        logger
            .append(
                "stream_complete",
                serde_json::json!({ "task_outcome": "completed" }),
            )
            .await;

        let page = read_session_history_event_page(&root, "session-2", Some(20), None)
            .expect("history page");
        let first = replay_session_history("session-2", &page.entries);
        let second = replay_session_history("session-2", &page.entries);

        assert_eq!(first, second);
        assert_eq!(first.messages.len(), 3);
        assert_eq!(first.messages[0].role, "user");
        assert_eq!(first.messages[0].content, "hello");
        assert_eq!(first.messages[1].role, "assistant");
        assert_eq!(first.messages[1].content, "answer");
        assert_eq!(first.messages[1].thinking.as_deref(), Some("plan"));
        assert_eq!(first.messages[2].role, "tool");
        assert_eq!(first.messages[2].content, "ok");
    }

    #[test]
    fn history_replay_preserves_cross_run_event_order() {
        let entries = vec![
            log_entry(
                "run-b",
                1,
                "run_started",
                "2026-04-23T00:00:01.000Z",
                serde_json::json!({ "message_preview": "first question" }),
            ),
            log_entry(
                "run-b",
                2,
                "text_delta",
                "2026-04-23T00:00:02.000Z",
                serde_json::json!({ "text": "first answer" }),
            ),
            log_entry(
                "run-a",
                1,
                "run_started",
                "2026-04-23T00:00:03.000Z",
                serde_json::json!({ "message_preview": "second question" }),
            ),
            log_entry(
                "run-a",
                2,
                "text_delta",
                "2026-04-23T00:00:04.000Z",
                serde_json::json!({ "text": "second answer" }),
            ),
        ];

        let replay = replay_session_history("session-order", &entries);

        let contents = replay
            .messages
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            contents,
            vec![
                "first question",
                "first answer",
                "second question",
                "second answer"
            ]
        );
        assert_eq!(replay.run_count, 2);
    }

    #[test]
    fn history_replay_preserves_tool_first_seen_order() {
        let entries = vec![
            log_entry(
                "run-1",
                1,
                "run_started",
                "2026-04-23T00:00:01.000Z",
                serde_json::json!({ "message_preview": "question" }),
            ),
            log_entry(
                "run-1",
                2,
                "tool_call_completed",
                "2026-04-23T00:00:02.000Z",
                serde_json::json!({
                    "tool_call_id": "tool-b",
                    "tool_name": "bash",
                    "tool_status": "completed",
                    "tool_result": "first tool"
                }),
            ),
            log_entry(
                "run-1",
                3,
                "tool_call_completed",
                "2026-04-23T00:00:03.000Z",
                serde_json::json!({
                    "tool_call_id": "tool-a",
                    "tool_name": "bash",
                    "tool_status": "completed",
                    "tool_result": "second tool"
                }),
            ),
        ];

        let replay = replay_session_history("session-order", &entries);
        let tool_contents = replay
            .messages
            .iter()
            .filter(|message| message.role == "tool")
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>();
        assert_eq!(tool_contents, vec!["first tool", "second tool"]);
    }
}
