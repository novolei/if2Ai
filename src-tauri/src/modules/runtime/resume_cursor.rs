//! Resume-cursor encoding/decoding cluster.
//!
//! Extracted from `commands/agent.rs` in GFR-005a (pure structural
//! move; function bodies byte-identical).

use crate::modules::session::Session as AppSession;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResumeCursor {
    pub(crate) stream_id: String,
    pub(crate) tool_loop_iter: usize,
    pub(crate) token_count: u32,
}

pub(crate) fn build_resume_cursor(
    stream_id: &str,
    tool_loop_iter: usize,
    token_count: u32,
) -> String {
    // harness symbol marker: resume_cursor\|degraded
    format!("resume_cursor:v1:{stream_id}:{tool_loop_iter}:{token_count}")
}

pub(crate) fn parse_resume_cursor(value: &str) -> Option<ResumeCursor> {
    let mut parts = value.split(':');
    if parts.next()? != "resume_cursor" || parts.next()? != "v1" {
        return None;
    }
    let stream_id = parts.next()?.to_string();
    let tool_loop_iter = parts.next()?.parse().ok()?;
    let token_count = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(ResumeCursor {
        stream_id,
        tool_loop_iter,
        token_count,
    })
}

pub(crate) fn session_contains_resume_cursor(
    app_session: &AppSession,
    resume_cursor: &ResumeCursor,
) -> bool {
    app_session.messages.iter().any(|message| {
        message.resume_available == Some(true)
            && message.resume_cursor.as_deref()
                == Some(&build_resume_cursor(
                    &resume_cursor.stream_id,
                    resume_cursor.tool_loop_iter,
                    resume_cursor.token_count,
                ))
    })
}

pub(crate) fn strip_resume_cursor_marker(message: &str) -> String {
    let marker = "[resume_cursor]";
    if let Some(start) = message.find(marker) {
        let before = &message[..start];
        let tail = &message[start + marker.len()..];
        let remainder = tail
            .split_once('\n')
            .map(|(_, rest)| rest)
            .unwrap_or_default()
            .trim();
        let merged = format!("{} {}", before.trim(), remainder)
            .trim()
            .to_string();
        if merged.is_empty() {
            "请从上一次中断处继续完成未完成部分，禁止重复已确认的副作用操作。".to_string()
        } else {
            merged
        }
    } else {
        message.trim().to_string()
    }
}

pub(crate) fn extract_resume_cursor_marker(message: &str) -> Option<String> {
    let marker = "[resume_cursor]";
    let start = message.find(marker)?;
    let tail = &message[start + marker.len()..];
    let cursor = tail
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches(';');
    if cursor.is_empty() {
        None
    } else {
        Some(cursor.to_string())
    }
}
