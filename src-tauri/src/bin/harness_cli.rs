//! harness-cli — Command-line interface for If2Ai harness observability.
//!
//! Reads session trace files (JSONL) from the default trace directory
//! (`~/.if2ai/traces/`) and provides commands to inspect and summarise
//! recorded agent sessions.
//!
//! # Usage
//!
//! ```text
//! harness-cli list                     # list all recorded sessions
//! harness-cli show <session-id>        # print all events for a session
//! harness-cli stats <session-id>       # aggregate telemetry for a session
//! harness-cli stats --all              # aggregate telemetry across all sessions
//! ```
//!
//! # Note
//! This binary operates in read-only mode — it never writes to the trace
//! directory. It does not need the Tauri app to be running.

use std::collections::HashMap;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let trace_dir = default_trace_dir();

    let command = args.get(1).map(String::as_str).unwrap_or("help");

    match command {
        "list" => cmd_list(&trace_dir),
        "show" => {
            let session_id = args.get(2).map(String::as_str).unwrap_or("");
            if session_id.is_empty() {
                eprintln!("Usage: harness-cli show <session-id>");
                std::process::exit(1);
            }
            cmd_show(&trace_dir, session_id);
        }
        "stats" => {
            let arg = args.get(2).map(String::as_str).unwrap_or("");
            if arg == "--all" {
                cmd_stats_all(&trace_dir);
            } else if !arg.is_empty() {
                cmd_stats(&trace_dir, arg);
            } else {
                eprintln!("Usage: harness-cli stats <session-id|--all>");
                std::process::exit(1);
            }
        }
        _ => print_help(),
    }
}

/// Return the default trace directory: `~/.if2ai/traces/`.
fn default_trace_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".if2ai")
        .join("traces")
}

/// List all session IDs for which trace files exist.
fn cmd_list(trace_dir: &Path) {
    let entries = list_trace_files(trace_dir);
    if entries.is_empty() {
        println!("No trace files found in {:?}", trace_dir);
        return;
    }
    println!("{} sessions found in {:?}:", entries.len(), trace_dir);
    for (id, path) in &entries {
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        println!("  {id}  ({size} bytes)");
    }
}

/// Print all events for a session, pretty-printed.
fn cmd_show(trace_dir: &Path, session_id: &str) {
    let path = trace_dir.join(format!("{session_id}.jsonl"));
    let events = read_trace(&path);
    if events.is_empty() {
        eprintln!("No trace file or no events found for session {session_id:?}");
        std::process::exit(1);
    }
    for (i, event) in events.iter().enumerate() {
        println!("{:4}. {}", i + 1, event);
    }
}

/// Print aggregated statistics for a single session.
fn cmd_stats(trace_dir: &Path, session_id: &str) {
    let path = trace_dir.join(format!("{session_id}.jsonl"));
    let events = read_trace(&path);
    if events.is_empty() {
        eprintln!("No trace file or no events found for session {session_id:?}");
        std::process::exit(1);
    }
    let stats = aggregate(&events);
    print_stats(session_id, &stats);
}

/// Print aggregated statistics across all sessions.
fn cmd_stats_all(trace_dir: &Path) {
    let entries = list_trace_files(trace_dir);
    if entries.is_empty() {
        println!("No trace files found in {:?}", trace_dir);
        return;
    }
    for (id, path) in &entries {
        let events = read_trace(path);
        let stats = aggregate(&events);
        print_stats(id, &stats);
        println!();
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Return (session_id, path) pairs for all `.jsonl` files in `dir`.
fn list_trace_files(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return vec![];
    };
    read_dir
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension()?.to_str()? == "jsonl" {
                let id = path.file_stem()?.to_string_lossy().into_owned();
                Some((id, path))
            } else {
                None
            }
        })
        .collect()
}

/// Read all JSON lines from a trace file into a `Vec<serde_json::Value>`.
fn read_trace(path: &Path) -> Vec<serde_json::Value> {
    let Ok(file) = std::fs::File::open(path) else {
        return vec![];
    };
    let mut events = Vec::new();
    for line in io::BufReader::new(file).lines() {
        let Ok(text) = line else {
            break;
        };
        if text.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            events.push(v);
        }
    }
    events
}

/// Simple aggregated stats from raw event JSON objects.
struct Stats {
    turn_count: u64,
    tool_calls: HashMap<String, u64>,
    total_tokens: u64,
    compaction_events: u64,
}

fn aggregate(events: &[serde_json::Value]) -> Stats {
    let mut stats = Stats {
        turn_count: 0,
        tool_calls: HashMap::new(),
        total_tokens: 0,
        compaction_events: 0,
    };
    for event in events {
        let et = event
            .get("event_type")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        match et {
            "turn_finished" => {
                stats.turn_count += 1;
                if let Some(t) = event.get("tokens_used").and_then(|v| v.as_u64()) {
                    stats.total_tokens += t;
                }
            }
            "tool_called" => {
                let name = event
                    .get("tool_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                *stats.tool_calls.entry(name).or_insert(0) += 1;
            }
            "context_compacted" => {
                stats.compaction_events += 1;
            }
            _ => {}
        }
    }
    stats
}

fn print_stats(session_id: &str, stats: &Stats) {
    println!("Session: {session_id}");
    println!("  Turns completed:     {}", stats.turn_count);
    println!("  Total tokens used:   {}", stats.total_tokens);
    println!("  Compaction events:   {}", stats.compaction_events);
    println!("  Tool calls:");
    let mut tools: Vec<_> = stats.tool_calls.iter().collect();
    tools.sort_by_key(|(_, v)| std::cmp::Reverse(**v));
    for (name, count) in tools {
        println!("    {name}: {count}");
    }
    if stats.tool_calls.is_empty() {
        println!("    (none)");
    }
}

fn print_help() {
    println!(
        r#"harness-cli — If2Ai agent loop trace inspector

USAGE:
  harness-cli list                  List all recorded sessions
  harness-cli show <session-id>     Print all events for a session
  harness-cli stats <session-id>    Aggregate telemetry for a session
  harness-cli stats --all           Aggregate telemetry across all sessions

Traces are read from ~/.if2ai/traces/*.jsonl (read-only).
"#
    );
}
