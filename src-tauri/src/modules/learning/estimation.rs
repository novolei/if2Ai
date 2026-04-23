//! P2-11 — Lightweight running average of turn duration (ms) for estimation hooks.
//!
//! Future packs can fuse with [`trajectory_score`](super::trajectory_score) and
//! provider billing metadata.

use std::sync::{Mutex, OnceLock};

static TURN_MS_AVG: OnceLock<Mutex<(u64, u64)>> = OnceLock::new();

fn cell() -> &'static Mutex<(u64, u64)> {
    TURN_MS_AVG.get_or_init(|| Mutex::new((0, 0)))
}

/// Update exponential-ish moving average: `avg = (avg * n + sample) / (n+1)` capped.
pub fn record_turn_duration_ms(sample_ms: u64) {
    if !std::env::var("IF2AI_ESTIMATION_LEARNER")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return;
    }
    let Ok(mut g) = cell().lock() else {
        return;
    };
    let (sum, n) = *g;
    let n1 = n.saturating_add(1).min(10_000);
    let sum1 = sum.saturating_add(sample_ms);
    *g = (sum1, n1);
}

#[must_use]
pub fn avg_turn_duration_ms() -> Option<u64> {
    let g = cell().lock().ok()?;
    let (sum, n) = *g;
    if n == 0 {
        None
    } else {
        Some(sum / n)
    }
}
