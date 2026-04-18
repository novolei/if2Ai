// 8A.3 lays the foundation for 8B+ daily aggregations; consumers
// (compile_today, diary_writer, ticker) ship in subsequent slices.
#![allow(dead_code)]

//! LogicalDay — the system-wide "day boundary" used by every daily
//! aggregation memory pipeline (rolling summary roll-over, compile_today,
//! compile_week, diary writer).
//!
//! A logical day starts at `cutoff_hour` LOCAL time (default `04:00`) and
//! ends at the same hour the next calendar day, so a `02:00` conversation
//! still rolls into "yesterday" rather than fragmenting the morning.
//!
//! Reference implementation: `openhanako/lib/time-utils.js`
//! (`DAY_BOUNDARY_HOUR = 4`).
//!
//! See `docs/design-docs/postCLI/memory-enhancement-from-openhanako-v1.md`
//! §Sprint 1 / T-A3 + §0.5 Δ-13 + Δ-16.

use std::str::FromStr;

use chrono::{DateTime, Datelike, Duration, LocalResult, NaiveDate, TimeZone, Timelike, Utc};
use chrono_tz::Tz;

use crate::modules::runtime::config::MemoryFeatureConfig;

/// Default cutoff hour, mirroring `openhanako`'s `DAY_BOUNDARY_HOUR = 4`.
pub const DEFAULT_CUTOFF_HOUR: u8 = 4;

/// One "logical day" — the conceptual day boundary used by all
/// daily-aggregation memory pipelines.  See module docs for rationale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalDay {
    /// Calendar date the logical day belongs to (in the configured tz).
    pub date: NaiveDate,
    /// Inclusive UTC start (`tz.date.cutoff_hour:00:00 → UTC`).
    pub range_start: DateTime<Utc>,
    /// Inclusive UTC end (`range_start + 24h - 1ms`); kept inclusive so
    /// `BETWEEN` SQL queries against `created_at` are trivial.
    pub range_end: DateTime<Utc>,
    /// `"YYYY-MM-DD"` rendering used in audit events and file paths.
    pub display: String,
}

/// Compute the [`LogicalDay`] for `now`, applying `cutoff_hour` LOCAL
/// time in `tz`.  DST gaps and overlaps are handled without panicking
/// (see [`logical_day_for_date`] for the resolution rules).
///
/// `cutoff_hour` is clamped to `0..=23` — values >= 24 collapse to 23.
#[must_use]
pub fn get_logical_day(now: DateTime<Utc>, cutoff_hour: u8, tz: Tz) -> LogicalDay {
    let cutoff = clamp_cutoff(cutoff_hour);
    let local = now.with_timezone(&tz);
    let date = if local.hour() < u32::from(cutoff) {
        local.date_naive() - Duration::days(1)
    } else {
        local.date_naive()
    };
    logical_day_for_date(date, cutoff, tz)
}

/// Build a [`LogicalDay`] for an explicit calendar `date` (already
/// expressed in `tz`), without inspecting wall-clock time.
///
/// DST handling:
///   * `LocalResult::Single`        → use the unique value;
///   * `LocalResult::Ambiguous`     → take the earlier instant (fall-back);
///   * `LocalResult::None` (gap)    → advance the clock by 1h and retry;
///     if that still fails we log and treat the naive datetime as UTC so
///     the function never panics.
#[must_use]
pub fn logical_day_for_date(date: NaiveDate, cutoff_hour: u8, tz: Tz) -> LogicalDay {
    let cutoff = clamp_cutoff(cutoff_hour);
    let naive_start = date
        .and_hms_opt(u32::from(cutoff), 0, 0)
        .unwrap_or_else(|| {
            tracing::warn!(
                cutoff_hour = cutoff,
                date = %date,
                "logical_day: cutoff_hour produced invalid NaiveDateTime; using 00:00"
            );
            // Safe: hour=0/min=0/sec=0 is always valid for any NaiveDate.
            date.and_hms_opt(0, 0, 0).unwrap_or_else(|| {
                NaiveDate::from_ymd_opt(1970, 1, 1)
                    .and_then(|d| d.and_hms_opt(0, 0, 0))
                    .unwrap_or_default()
            })
        });

    let range_start = match tz.from_local_datetime(&naive_start) {
        LocalResult::Single(dt) => dt.with_timezone(&Utc),
        LocalResult::Ambiguous(earlier, _later) => {
            tracing::warn!(
                date = %date,
                cutoff_hour = cutoff,
                tz = tz.name(),
                "logical_day: ambiguous local time at DST fall-back; using earliest"
            );
            earlier.with_timezone(&Utc)
        }
        LocalResult::None => {
            tracing::warn!(
                date = %date,
                cutoff_hour = cutoff,
                tz = tz.name(),
                "logical_day: non-existent local time at DST spring-forward; advancing 1h"
            );
            match tz.from_local_datetime(&(naive_start + Duration::hours(1))) {
                LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt.with_timezone(&Utc),
                LocalResult::None => {
                    tracing::warn!(
                        date = %date,
                        tz = tz.name(),
                        "logical_day: 2nd DST attempt also gap; treating naive as UTC"
                    );
                    Utc.from_utc_datetime(&naive_start)
                }
            }
        }
    };

    let range_end = range_start + Duration::hours(24) - Duration::milliseconds(1);
    let display = format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day());

    LogicalDay {
        date,
        range_start,
        range_end,
        display,
    }
}

/// Convenience wrapper that reads the cutoff + timezone from
/// [`MemoryFeatureConfig::default`] and computes the current logical
/// day.  Once `RuntimeConfig` exposes a global accessor (TODO(8A.x):
/// `runtime::config::current()`), route through that instead so user
/// overrides take effect without an app restart.
#[must_use]
pub fn get_today() -> LogicalDay {
    let memory = MemoryFeatureConfig::default();
    let tz = resolve_timezone(memory.timezone());
    get_logical_day(Utc::now(), memory.logical_day_cutoff_hour(), tz)
}

/// Parse an IANA timezone name; on failure log a warning and fall back
/// to `chrono_tz::UTC`.  `None` (= no user override) → UTC as well.
#[must_use]
pub fn resolve_timezone(name: Option<&str>) -> Tz {
    let Some(name) = name else {
        return chrono_tz::UTC;
    };
    match Tz::from_str(name) {
        Ok(tz) => tz,
        Err(err) => {
            tracing::warn!(
                tz = name,
                error = %err,
                "logical_day: unknown IANA timezone; falling back to UTC"
            );
            chrono_tz::UTC
        }
    }
}

#[inline]
fn clamp_cutoff(hour: u8) -> u8 {
    hour.min(23)
}

#[cfg(test)]
mod tests {
    use super::{
        clamp_cutoff, get_logical_day, get_today, logical_day_for_date, resolve_timezone,
        DEFAULT_CUTOFF_HOUR,
    };
    use chrono::{Duration, NaiveDate, TimeZone, Utc};
    use chrono_tz::{America::New_York, Asia::Shanghai};

    #[test]
    fn boundary_at_cutoff_returns_same_day() {
        // 04:00:00 Asia/Shanghai = 2024-01-15 04:00 UTC+8 = 2024-01-14T20:00:00Z
        let now = Utc.with_ymd_and_hms(2024, 1, 14, 20, 0, 0).unwrap();
        let day = get_logical_day(now, DEFAULT_CUTOFF_HOUR, Shanghai);
        assert_eq!(day.date, NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
        assert_eq!(day.display, "2024-01-15");
    }

    #[test]
    fn boundary_before_cutoff_rolls_to_previous_day() {
        // 2024-01-14T19:59:59Z = 2024-01-15 03:59:59 Asia/Shanghai (hour 3 < 4)
        // → rolls to 前一日 (2024-01-14).
        let now = Utc.with_ymd_and_hms(2024, 1, 14, 19, 59, 59).unwrap();
        let day = get_logical_day(now, DEFAULT_CUTOFF_HOUR, Shanghai);
        assert_eq!(day.date, NaiveDate::from_ymd_opt(2024, 1, 14).unwrap());
        assert_eq!(day.display, "2024-01-14");
    }

    #[test]
    fn boundary_after_cutoff_returns_same_day() {
        // 12:00 Asia/Shanghai = 2024-01-15T04:00:00Z → 当日 (2024-01-15)
        let now = Utc.with_ymd_and_hms(2024, 1, 15, 4, 0, 0).unwrap();
        let day = get_logical_day(now, DEFAULT_CUTOFF_HOUR, Shanghai);
        assert_eq!(day.date, NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
    }

    #[test]
    fn dst_spring_forward_does_not_panic() {
        // America/New_York 2024-03-10: clocks jump 02:00 → 03:00.
        // Asking for cutoff_hour = 2 lands the boundary inside the DST gap.
        let date = NaiveDate::from_ymd_opt(2024, 3, 10).unwrap();
        let day = logical_day_for_date(date, 2, New_York);
        assert_eq!(day.date, date);
        assert_eq!(day.display, "2024-03-10");
        // After DST advance: 03:00 EDT = 07:00 UTC.
        let expected_start = Utc.with_ymd_and_hms(2024, 3, 10, 7, 0, 0).unwrap();
        assert_eq!(day.range_start, expected_start);
    }

    #[test]
    fn dst_fall_back_picks_earliest_when_ambiguous() {
        // America/New_York 2024-11-03: clocks fall back 02:00 → 01:00.
        // 01:30 happens twice; with cutoff = 1 we hit the ambiguous slot.
        let date = NaiveDate::from_ymd_opt(2024, 11, 3).unwrap();
        let day = logical_day_for_date(date, 1, New_York);
        assert_eq!(day.date, date);
        // 01:00 EDT (the *earlier* of the two) = 05:00 UTC.
        let expected_start = Utc.with_ymd_and_hms(2024, 11, 3, 5, 0, 0).unwrap();
        assert_eq!(day.range_start, expected_start);
    }

    #[test]
    fn unknown_tz_falls_back_to_utc() {
        let tz = resolve_timezone(Some("Not/A_Real_Zone"));
        assert_eq!(tz, chrono_tz::UTC);
    }

    #[test]
    fn none_tz_resolves_to_utc() {
        assert_eq!(resolve_timezone(None), chrono_tz::UTC);
    }

    #[test]
    fn known_tz_resolves_correctly() {
        let tz = resolve_timezone(Some("Asia/Shanghai"));
        assert_eq!(tz, Shanghai);
    }

    #[test]
    fn range_end_is_one_day_minus_one_ms_after_start() {
        let date = NaiveDate::from_ymd_opt(2024, 6, 1).unwrap();
        let day = logical_day_for_date(date, DEFAULT_CUTOFF_HOUR, chrono_tz::UTC);
        assert_eq!(
            day.range_end - day.range_start,
            Duration::hours(24) - Duration::milliseconds(1)
        );
    }

    #[test]
    fn display_format_matches_yyyy_mm_dd() {
        let date = NaiveDate::from_ymd_opt(2024, 2, 9).unwrap();
        let day = logical_day_for_date(date, DEFAULT_CUTOFF_HOUR, chrono_tz::UTC);
        assert_eq!(day.display, "2024-02-09");
    }

    #[test]
    fn cutoff_hour_clamps_to_23() {
        assert_eq!(clamp_cutoff(0), 0);
        assert_eq!(clamp_cutoff(23), 23);
        assert_eq!(clamp_cutoff(24), 23);
        assert_eq!(clamp_cutoff(255), 23);
    }

    #[test]
    fn get_today_does_not_panic_with_default_config() {
        let _ = get_today();
    }
}
