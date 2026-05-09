//! Refresh step — identifies stale entries.
//!
//! Today this step is a **stub**: it counts entries whose
//! `last_validated_at` is older than `STALE_DAYS` and reports them as
//! `examined`. `mutated` is always 0 because the in-place re-embedding
//! API does not exist yet on vnext (embeddings live in the vector
//! provider, not on `MemoryEntry`). When that API ships (Wave A.5),
//! this step will re-embed and bump `last_validated_at`.

use chrono::{Duration, Utc};

use crate::modules::memory::SharedMemoryProvider;

use super::report::{StepError, StepOutcome};

/// Entries whose `last_validated_at` is older than this are considered stale.
pub const STALE_DAYS: i64 = 30;

pub async fn run(provider: &SharedMemoryProvider, max_entries: usize) -> StepOutcome {
    let started = std::time::Instant::now();
    let cutoff = Utc::now() - Duration::days(STALE_DAYS);

    let entries = match provider.as_ref().export(None).await {
        Ok(v) => v,
        Err(e) => {
            return StepOutcome {
                step: "refresh".into(),
                examined: 0,
                mutated: 0,
                duration_ms: started.elapsed().as_millis() as u64,
                error: Some(StepError {
                    kind: "provider".into(),
                    message: format!("export failed: {e}"),
                }),
                extracted_count: None,
                rejected_count: None,
            };
        }
    };

    let stale_count = entries
        .into_iter()
        .take(max_entries)
        .filter(|e| e.last_validated_at.is_none_or(|t| t < cutoff))
        .count();

    StepOutcome {
        step: "refresh".into(),
        examined: stale_count,
        mutated: 0, // re-embedding API not yet available — Wave A.5
        duration_ms: started.elapsed().as_millis() as u64,
        error: None,
        extracted_count: None,
        rejected_count: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_days_constant_is_30() {
        assert_eq!(STALE_DAYS, 30);
    }
}
