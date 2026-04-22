//! MEM-MOD-PD0 — Day Awareness prompt block.
//!
//! Gives the agent a stable sense of "today" so it can reason about
//! freshness, time of day, and the boundary between two logical days.
//!
//! The block is intentionally short (≤ 6 lines, ≈ 60–90 tokens) and
//! sits next to the Scenario block (priority 92) — late enough that
//! Soul/Persona anchor identity first, early enough that any tool
//! reasoning sees the date.
//!
//! Time-of-day buckets follow openhanako's convention so future
//! daily-aggregation memories share vocabulary with the LLM:
//!
//! | bucket          | local hour range | label CN  |
//! |-----------------|-----------------|-----------|
//! | `late_night`    | 00–03           | 深夜      |
//! | `early_morning` | 04–06           | 凌晨      |
//! | `morning`       | 07–10           | 上午      |
//! | `midday`        | 11–13           | 中午      |
//! | `afternoon`     | 14–17           | 下午      |
//! | `evening`       | 18–21           | 傍晚      |
//! | `night`         | 22–23           | 夜晚      |
//!
//! "距上次对话" (gap since last turn) is tracked as a TODO — we'll
//! plumb that through in MEM-MOD-P5 when the reflection ticker has
//! a stable per-session `last_turn_at` source. Until then this block
//! only emits the "time anchor" half.

use chrono::{Datelike, Timelike, Utc};
use chrono_tz::Tz;

use crate::modules::runtime::config;
use crate::modules::runtime::logical_day::{get_logical_day, resolve_timezone, LogicalDay};

/// Output of [`render_day_awareness_block`]. Keeping a struct (not a
/// bare `String`) so future extensions (e.g. last-turn gap) don't
/// force a signature change.
#[derive(Debug, Clone)]
pub struct DayAwarenessBlock {
    /// Rendered block content (multi-line markdown-flavoured text).
    pub content: String,
    /// `"YYYY-MM-DD"` slug suitable for `PromptBlockSource.reference`.
    pub day_slug: String,
}

/// Render the Day Awareness block from the global runtime config.
///
/// Returns `None` only in the degenerate case where the config provides
/// no timezone and we cannot determine local hour — in practice we
/// always fall back to UTC, so this currently always returns `Some`.
#[must_use]
pub fn render_day_awareness_block() -> Option<DayAwarenessBlock> {
    let memory = config::current().memory();
    let tz = resolve_timezone(memory.timezone());
    let cutoff = memory.logical_day_cutoff_hour();
    let now = Utc::now();
    let day = get_logical_day(now, cutoff, tz);
    Some(render_for(now, &day, tz, cutoff))
}

fn render_for(
    now: chrono::DateTime<Utc>,
    day: &LogicalDay,
    tz: Tz,
    cutoff_hour: u8,
) -> DayAwarenessBlock {
    let local = now.with_timezone(&tz);
    let hour = local.hour();
    let bucket = bucket_for(hour);
    let weekday = weekday_label(day.date.weekday());
    let tz_name = tz.name();

    // Bilingual CN/EN to match Soul/Persona block style; LLM can use
    // whichever surfaces in user-facing reply.
    let mut lines = Vec::with_capacity(6);
    lines.push("# Day Awareness · 时间感".to_string());
    lines.push(format!(
        "- 今日 (logical day): {} · {} — 凌晨 {:02}:00 之前算前一日",
        day.display, weekday.cn, cutoff_hour,
    ));
    lines.push(format!(
        "- 此刻 / Now: {} {} ({}) · {}",
        local.format("%H:%M"),
        weekday.en,
        tz_name,
        bucket.label_cn,
    ));
    lines.push(format!(
        "- 时段 tag: `{}` (用于 daily aggregations 关联记忆)",
        bucket.slug,
    ));

    DayAwarenessBlock {
        content: lines.join("\n"),
        day_slug: day.display.clone(),
    }
}

#[derive(Debug, Clone, Copy)]
struct TimeBucket {
    slug: &'static str,
    label_cn: &'static str,
}

#[inline]
fn bucket_for(hour: u32) -> TimeBucket {
    match hour {
        0..=3 => TimeBucket { slug: "late_night", label_cn: "深夜" },
        4..=6 => TimeBucket { slug: "early_morning", label_cn: "凌晨" },
        7..=10 => TimeBucket { slug: "morning", label_cn: "上午" },
        11..=13 => TimeBucket { slug: "midday", label_cn: "中午" },
        14..=17 => TimeBucket { slug: "afternoon", label_cn: "下午" },
        18..=21 => TimeBucket { slug: "evening", label_cn: "傍晚" },
        _ => TimeBucket { slug: "night", label_cn: "夜晚" },
    }
}

#[derive(Debug, Clone, Copy)]
struct WeekdayLabel {
    cn: &'static str,
    en: &'static str,
}

#[inline]
fn weekday_label(wd: chrono::Weekday) -> WeekdayLabel {
    use chrono::Weekday::*;
    match wd {
        Mon => WeekdayLabel { cn: "周一", en: "Mon" },
        Tue => WeekdayLabel { cn: "周二", en: "Tue" },
        Wed => WeekdayLabel { cn: "周三", en: "Wed" },
        Thu => WeekdayLabel { cn: "周四", en: "Thu" },
        Fri => WeekdayLabel { cn: "周五", en: "Fri" },
        Sat => WeekdayLabel { cn: "周六", en: "Sat" },
        Sun => WeekdayLabel { cn: "周日", en: "Sun" },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use chrono_tz::Asia::Shanghai;

    #[test]
    fn buckets_cover_full_24h() {
        for h in 0..24 {
            let _ = bucket_for(h);
        }
    }

    #[test]
    fn block_contains_logical_day_and_bucket() {
        let now = Utc.with_ymd_and_hms(2026, 4, 22, 6, 32, 0).unwrap(); // 14:32 Asia/Shanghai
        let day = get_logical_day(now, 4, Shanghai);
        let block = render_for(now, &day, Shanghai, 4);
        assert!(block.content.contains("2026-04-22"));
        assert!(block.content.contains("afternoon"));
        assert!(block.content.contains("Asia/Shanghai"));
        assert_eq!(block.day_slug, "2026-04-22");
    }

    #[test]
    fn before_cutoff_rolls_to_previous_day() {
        // 03:30 Shanghai 2026-04-22 → logical day still 2026-04-21
        let now = Utc.with_ymd_and_hms(2026, 4, 21, 19, 30, 0).unwrap();
        let day = get_logical_day(now, 4, Shanghai);
        let block = render_for(now, &day, Shanghai, 4);
        assert!(block.content.contains("2026-04-21"));
        assert!(block.content.contains("late_night"));
    }

    #[test]
    fn weekday_label_is_localised() {
        // 2026-04-22 is a Wednesday.
        let now = Utc.with_ymd_and_hms(2026, 4, 22, 6, 0, 0).unwrap();
        let day = get_logical_day(now, 4, Shanghai);
        let block = render_for(now, &day, Shanghai, 4);
        assert!(block.content.contains("周三"));
        assert!(block.content.contains("Wed"));
    }
}
