//! P2-12 — In-process fan-out for subtask / turn tick lines (MVP).
//!
//! A future pack can bridge this to the main chat stream / IPC; today we
//! only buffer short text lines keyed by `session_id` for tests + diagnostics.

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};

const MAX_PER_SESSION: usize = 64;

struct Hub {
    lines: HashMap<String, VecDeque<String>>,
}

static HUB: OnceLock<Mutex<Hub>> = OnceLock::new();

fn hub() -> &'static Mutex<Hub> {
    HUB.get_or_init(|| {
        Mutex::new(Hub {
            lines: HashMap::new(),
        })
    })
}

/// Push one diagnostic line for a session (best-effort).
///
/// Also fans out to the lightweight [`observability`] surface
/// (no-op unless `IF2AI_OBSERVER=log`) so external observers /
/// future IPC bridges see the line without polling `drain_lines`.
pub fn publish_line(session_id: &str, line: impl Into<String>) {
    if std::env::var("IF2AI_JOB_MONITOR")
        .map(|v| v == "0" || v.eq_ignore_ascii_case("false"))
        .unwrap_or(false)
    {
        return;
    }
    let line = line.into();
    crate::modules::observability::emit(
        "job_monitor.line",
        &format!("session={session_id} line={line}"),
    );
    let Ok(mut g) = hub().lock() else {
        return;
    };
    let q = g.lines.entry(session_id.to_string()).or_default();
    q.push_back(line);
    while q.len() > MAX_PER_SESSION {
        q.pop_front();
    }
}

/// Drain queued lines for UI / replay (non-destructive peek would be another API).
#[must_use]
pub fn drain_lines(session_id: &str) -> Vec<String> {
    let Ok(mut g) = hub().lock() else {
        return Vec::new();
    };
    g.lines
        .remove(session_id)
        .map(|q| q.into_iter().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_then_drain_round_trip() {
        publish_line("sess-job-monitor-test", "tick-a");
        publish_line("sess-job-monitor-test", "tick-b");
        let drained = drain_lines("sess-job-monitor-test");
        assert_eq!(drained, vec!["tick-a", "tick-b"]);
        assert!(drain_lines("sess-job-monitor-test").is_empty());
    }
}
