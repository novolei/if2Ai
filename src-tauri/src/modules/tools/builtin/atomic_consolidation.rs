//! FEAT-BR-003 — Tool atomicity consolidation.
//!
//! Static alias table that re-routes legacy fine-grained tool names
//! (18 entries) onto the 6 canonical "atomic" tool names per the
//! `.qoder/specs/if2ai-agent-evolution-report.md` Phase 7 plan:
//!
//! - **3 memory atoms** — `memory_read` / `memory_write` / `memory_search`
//! - **1 cron atom** — `schedule_manage`
//! - **2 skill atoms** — `skill_find` / `skill_use`
//!
//! By Pack contract this module is **read-only mapping** — it does
//! NOT mutate any existing tool implementation, registration, or
//! `ToolDefinition` runtime. The wire-up Pack will consume
//! [`resolve_alias`] inside the tool dispatcher.

#![allow(dead_code)]

/// Single legacy → atomic mapping entry. `&'static str` everywhere so
/// the table is purely static (zero allocation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ToolAlias {
    pub legacy_name: &'static str,
    pub atomic_name: &'static str,
}

/// Atomic tool names enumerated in declaration order. `list_atomic_tools`
/// returns these de-duplicated.
pub const ATOMIC_MEMORY_READ: &str = "memory_read";
pub const ATOMIC_MEMORY_WRITE: &str = "memory_write";
pub const ATOMIC_MEMORY_SEARCH: &str = "memory_search";
pub const ATOMIC_SCHEDULE_MANAGE: &str = "schedule_manage";
pub const ATOMIC_SKILL_FIND: &str = "skill_find";
pub const ATOMIC_SKILL_USE: &str = "skill_use";

/// Normative alias table — kept in spec order so `git diff` reads
/// like the Pack table.
pub const TOOL_ALIASES: &[ToolAlias] = &[
    // memory ×6 → 3 atomic
    ToolAlias {
        legacy_name: "memory_recall",
        atomic_name: ATOMIC_MEMORY_READ,
    },
    ToolAlias {
        legacy_name: "memory_export",
        atomic_name: ATOMIC_MEMORY_READ,
    },
    ToolAlias {
        legacy_name: "memory_pin",
        atomic_name: ATOMIC_MEMORY_WRITE,
    },
    ToolAlias {
        legacy_name: "memory_compile",
        atomic_name: ATOMIC_MEMORY_WRITE,
    },
    ToolAlias {
        legacy_name: "memory_query",
        atomic_name: ATOMIC_MEMORY_SEARCH,
    },
    ToolAlias {
        legacy_name: "memory_search",
        atomic_name: ATOMIC_MEMORY_SEARCH,
    },
    // cron ×5 → 1 atomic
    ToolAlias {
        legacy_name: "cron_create",
        atomic_name: ATOMIC_SCHEDULE_MANAGE,
    },
    ToolAlias {
        legacy_name: "cron_list",
        atomic_name: ATOMIC_SCHEDULE_MANAGE,
    },
    ToolAlias {
        legacy_name: "cron_pause",
        atomic_name: ATOMIC_SCHEDULE_MANAGE,
    },
    ToolAlias {
        legacy_name: "cron_resume",
        atomic_name: ATOMIC_SCHEDULE_MANAGE,
    },
    ToolAlias {
        legacy_name: "cron_delete",
        atomic_name: ATOMIC_SCHEDULE_MANAGE,
    },
    // skill ×7 → 2 atomic
    ToolAlias {
        legacy_name: "skill_list",
        atomic_name: ATOMIC_SKILL_FIND,
    },
    ToolAlias {
        legacy_name: "skill_search",
        atomic_name: ATOMIC_SKILL_FIND,
    },
    ToolAlias {
        legacy_name: "skill_export",
        atomic_name: ATOMIC_SKILL_FIND,
    },
    ToolAlias {
        legacy_name: "skill_load",
        atomic_name: ATOMIC_SKILL_USE,
    },
    ToolAlias {
        legacy_name: "skill_run",
        atomic_name: ATOMIC_SKILL_USE,
    },
    ToolAlias {
        legacy_name: "skill_install",
        atomic_name: ATOMIC_SKILL_USE,
    },
    ToolAlias {
        legacy_name: "skill_remove",
        atomic_name: ATOMIC_SKILL_USE,
    },
];

/// Resolve a legacy tool name to its atomic equivalent. Returns `None`
/// for unknown names so callers can pass them through unchanged.
#[must_use]
pub fn resolve_alias(legacy_name: &str) -> Option<&'static str> {
    TOOL_ALIASES
        .iter()
        .find(|a| a.legacy_name == legacy_name)
        .map(|a| a.atomic_name)
}

/// Number of legacy aliases defined. Stable counter for tests.
#[must_use]
pub const fn alias_count() -> usize {
    TOOL_ALIASES.len()
}

/// Distinct atomic tool names in declaration order. Length is always
/// 6 by Pack contract.
#[must_use]
pub fn list_atomic_tools() -> Vec<&'static str> {
    let mut seen: Vec<&'static str> = Vec::new();
    for alias in TOOL_ALIASES {
        if !seen.contains(&alias.atomic_name) {
            seen.push(alias.atomic_name);
        }
    }
    seen
}
