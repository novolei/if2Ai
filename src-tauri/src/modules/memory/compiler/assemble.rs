//! Assemble `today.md` / `week.md` / `longterm.md` / `facts.md` →
//! `memory.md` (Phase 8B.4 / T-C4).
//!
//! Synchronous file I/O — no LLM, no async.  Reads the four
//! per-section `*.md` files, wraps each in a `## <Section title>`
//! header (placeholders for empty bodies), concatenates them in the
//! order `facts → today → week → longterm`, applies a 5000-char cap
//! per v2 §0.5 Δ-17 (truncating from the *lowest* priority section
//! upwards: longterm → week → today, never facts), and atomically
//! writes `memory.md`.  Emits `memory_assembled` audit on success.
//!
//! Section titles are bilingual (zh-CN vs English) per v2 §0.5 Δ-13;
//! the caller passes `is_zh` once after reading
//! [`crate::modules::runtime::locale::is_zh`].

#![allow(dead_code)] // first production caller is MemoryCompiler::assemble (8B.5+)

use std::path::Path;

use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};
use crate::modules::memory::compiler::CompilePaths;
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::MemoryError;

/// Hard cap on the assembled `memory.md` body in chars per v2 §0.5
/// Δ-17.  Sections beyond this budget are truncated from lowest
/// priority (longterm) upward.
pub const MEMORY_MD_MAX_CHARS: usize = 5000;
/// Per-section floor — never truncate below `## <title>\n\n<placeholder>`
/// so the assembled file always retains the four-section shape.
const SECTION_FLOOR_CHARS: usize = 32;
/// Marker appended to a truncated section so downstream readers can
/// distinguish "this is all there is" from "we cut it for budget".
const TRUNCATED_MARKER: &str = "\n…(truncated)";

/// Concatenate the four compile artefacts into `memory.md`.
///
/// Returns `Ok(())` on success; never returns `Skipped` because
/// assembly is cheap (synchronous file I/O) and the orchestrator
/// always wants a fresh `memory.md` after at least one
/// `compile_*` rewrote anything.
pub fn assemble(
    paths: &CompilePaths,
    scope: &MemoryExecutionScope,
    is_zh: bool,
) -> Result<(), MemoryError> {
    let read = |p: &Path| -> String {
        std::fs::read_to_string(p)
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    let facts = read(&paths.facts_md);
    let today = read(&paths.today_md);
    let week = read(&paths.week_md);
    let longterm = read(&paths.longterm_md);

    let titles: [&str; 4] = if is_zh {
        ["重要事实", "今天", "最近一周", "长期情况"]
    } else {
        ["Key facts", "Today", "Past week", "Long-term context"]
    };
    let placeholder = if is_zh { "（暂无）" } else { "(none)" };

    let mut sections = [
        format_section(titles[0], &facts, placeholder),
        format_section(titles[1], &today, placeholder),
        format_section(titles[2], &week, placeholder),
        format_section(titles[3], &longterm, placeholder),
    ];

    truncate_to_budget(&mut sections);

    let mut body = sections.join("\n\n");
    body.push('\n');
    atomic_write(&paths.memory_md, &body)?;

    let chars = body.chars().count();
    let audit_ctx = AuditContext::from_scope(scope);
    MemoryAuditEmitter::memory_assembled(&audit_ctx, chars, &titles);
    Ok(())
}

/// Wrap a section body in its `## <title>` header, substituting the
/// localised placeholder when the body is empty.
fn format_section(title: &str, body: &str, placeholder: &str) -> String {
    let body = if body.is_empty() { placeholder } else { body };
    format!("## {title}\n\n{body}")
}

/// Apply v2 §0.5 Δ-17: when total chars exceed [`MEMORY_MD_MAX_CHARS`],
/// shrink sections from lowest priority upward
/// (longterm → week → today), never touching facts.  Each pass cuts a
/// section to two-thirds of its current size; we stop when we are
/// under the cap or every shrinkable section has hit
/// [`SECTION_FLOOR_CHARS`].
fn truncate_to_budget(sections: &mut [String; 4]) {
    // Section indices in priority order: facts(0) > today(1) > week(2) > longterm(3).
    // Truncate lowest priority first → traverse [3, 2, 1].  Facts is
    // intentionally excluded.
    const TRUNCATE_ORDER: [usize; 3] = [3, 2, 1];

    loop {
        let total = total_chars(sections);
        if total <= MEMORY_MD_MAX_CHARS {
            return;
        }
        let mut shrunk_any = false;
        for &i in &TRUNCATE_ORDER {
            let current_chars = sections[i].chars().count();
            if current_chars <= SECTION_FLOOR_CHARS {
                continue;
            }
            let target = ((current_chars * 2) / 3).max(SECTION_FLOOR_CHARS);
            let truncated: String = sections[i].chars().take(target).collect();
            sections[i] = format!("{truncated}{TRUNCATED_MARKER}");
            shrunk_any = true;
            if total_chars(sections) <= MEMORY_MD_MAX_CHARS {
                return;
            }
        }
        if !shrunk_any {
            return;
        }
    }
}

/// Sum chars across all sections, including the `\n\n` joiners that
/// `assemble()` will insert between them, so the budget check matches
/// the final on-disk size.
fn total_chars(sections: &[String; 4]) -> usize {
    let body_chars: usize = sections.iter().map(|s| s.chars().count()).sum();
    // 3 joiners of "\n\n" (2 chars each) + trailing "\n" added in assemble().
    body_chars + 3 * 2 + 1
}

/// Atomic file write: tmp + rename.  Mirrors
/// [`crate::modules::memory::compiler::today`]'s helper so a crash
/// mid-write cannot corrupt `memory.md`.
fn atomic_write(output_path: &Path, content: &str) -> Result<(), MemoryError> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| MemoryError::Generic(format!("assemble: mkdir {parent:?}: {e}")))?;
    }
    let mut tmp = output_path.to_path_buf().into_os_string();
    tmp.push(".tmp");
    let tmp_path = std::path::PathBuf::from(tmp);
    std::fs::write(&tmp_path, content)
        .map_err(|e| MemoryError::Generic(format!("assemble tmp write {tmp_path:?}: {e}")))?;
    std::fs::rename(&tmp_path, output_path).map_err(|e| {
        MemoryError::Generic(format!(
            "assemble rename {tmp_path:?} -> {output_path:?}: {e}"
        ))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn empty_paths_writes_four_section_placeholders_zh() {
        let dir = tempdir().expect("tempdir");
        let paths = CompilePaths::from_scope_root(dir.path());
        assemble(&paths, &MemoryExecutionScope::global(), true).expect("assemble");
        let body = std::fs::read_to_string(&paths.memory_md).expect("read");
        assert!(body.contains("## 重要事实"));
        assert!(body.contains("## 今天"));
        assert!(body.contains("## 最近一周"));
        assert!(body.contains("## 长期情况"));
        assert!(body.contains("（暂无）"));
    }

    #[test]
    fn empty_paths_writes_four_section_placeholders_en() {
        let dir = tempdir().expect("tempdir");
        let paths = CompilePaths::from_scope_root(dir.path());
        assemble(&paths, &MemoryExecutionScope::global(), false).expect("assemble");
        let body = std::fs::read_to_string(&paths.memory_md).expect("read");
        assert!(body.contains("## Key facts"));
        assert!(body.contains("## Today"));
        assert!(body.contains("## Past week"));
        assert!(body.contains("## Long-term context"));
        assert!(body.contains("(none)"));
    }

    #[test]
    fn populated_sections_appear_in_priority_order() {
        let dir = tempdir().expect("tempdir");
        let paths = CompilePaths::from_scope_root(dir.path());
        std::fs::write(&paths.facts_md, "fact alpha").expect("w facts");
        std::fs::write(&paths.today_md, "today beta").expect("w today");
        std::fs::write(&paths.week_md, "week gamma").expect("w week");
        std::fs::write(&paths.longterm_md, "longterm delta").expect("w lt");
        assemble(&paths, &MemoryExecutionScope::global(), true).expect("assemble");
        let body = std::fs::read_to_string(&paths.memory_md).expect("read");
        let pos_facts = body.find("fact alpha").expect("facts present");
        let pos_today = body.find("today beta").expect("today present");
        let pos_week = body.find("week gamma").expect("week present");
        let pos_longterm = body.find("longterm delta").expect("longterm present");
        assert!(pos_facts < pos_today, "facts before today");
        assert!(pos_today < pos_week, "today before week");
        assert!(pos_week < pos_longterm, "week before longterm");
    }

    #[test]
    fn truncation_5000_keeps_facts_intact() {
        let dir = tempdir().expect("tempdir");
        let paths = CompilePaths::from_scope_root(dir.path());
        let big = "x".repeat(2000);
        std::fs::write(&paths.facts_md, &big).expect("w facts");
        std::fs::write(&paths.today_md, &big).expect("w today");
        std::fs::write(&paths.week_md, &big).expect("w week");
        std::fs::write(&paths.longterm_md, &big).expect("w lt");
        assemble(&paths, &MemoryExecutionScope::global(), true).expect("assemble");
        let body = std::fs::read_to_string(&paths.memory_md).expect("read");
        // Body must fit within the 5000-char cap (plus a small
        // overhead headroom for the `…(truncated)` marker on a
        // section we just truncated past the boundary check).
        let chars = body.chars().count();
        assert!(
            chars <= MEMORY_MD_MAX_CHARS + TRUNCATED_MARKER.chars().count(),
            "body chars {chars} exceeds budget"
        );
        // Facts section must retain all 2000 'x's untouched.
        let facts_section_end = body.find("## 今天").expect("today header present");
        let facts_part = &body[..facts_section_end];
        assert!(
            facts_part.matches('x').count() >= 2000,
            "facts section must not be truncated"
        );
        assert!(
            !facts_part.contains(TRUNCATED_MARKER.trim()),
            "facts must not carry truncated marker"
        );
        // longterm must be the first to be cut → carries the marker.
        assert!(
            body.contains(TRUNCATED_MARKER.trim()),
            "at least one section must show truncation"
        );
    }

    #[test]
    fn assemble_overwrites_existing_memory_md() {
        let dir = tempdir().expect("tempdir");
        let paths = CompilePaths::from_scope_root(dir.path());
        std::fs::write(&paths.memory_md, "stale content from previous run").expect("seed");
        std::fs::write(&paths.facts_md, "new fact").expect("w facts");
        assemble(&paths, &MemoryExecutionScope::global(), true).expect("assemble");
        let body = std::fs::read_to_string(&paths.memory_md).expect("read");
        assert!(body.contains("new fact"));
        assert!(!body.contains("stale content"));
    }

    #[test]
    fn format_section_substitutes_placeholder_when_empty() {
        let s = format_section("Today", "", "(none)");
        assert_eq!(s, "## Today\n\n(none)");
        let s2 = format_section("Today", "real body", "(none)");
        assert_eq!(s2, "## Today\n\nreal body");
    }
}
