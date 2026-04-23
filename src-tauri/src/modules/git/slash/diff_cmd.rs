//! `/diff` — show staged + unstaged diffs.
//!
//! Default mode is `--stat` (one line per file: `+N -N`) so the chat
//! transcript / event log never gets blown up by a multi-MB patch.
//! Pass `full` as an argument (`/diff full`) to opt into the complete
//! unified patch.

use super::super::error::GitResult;
use super::super::status::{self, DiffMode};
use super::SlashRequest;

/// Dispatch a `/diff` request — render staged + unstaged diff text.
pub(crate) fn handle(req: &SlashRequest<'_>) -> GitResult<String> {
    let full_requested = req
        .args
        .iter()
        .any(|arg| arg.eq_ignore_ascii_case("full") || *arg == "--full");
    let mode = if full_requested {
        DiffMode::Full
    } else {
        DiffMode::Stat
    };
    let label = if full_requested {
        "computed (full patch)"
    } else {
        "computed (--stat)"
    };
    Ok(match status::read_diff_with_mode(req.cwd, mode)? {
        Some(diff) => format!("Diff\n  Result           {label}\n\n{diff}"),
        None => "Diff\n  Result           clean (no staged or unstaged changes)".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::git::test_support::TestRepo;
    use std::path::Path;

    fn req(cwd: &Path) -> SlashRequest<'_> {
        SlashRequest {
            name: "diff",
            args: vec![],
            message: None,
            title: None,
            branch_hint: None,
            cwd,
        }
    }

    #[test]
    fn diff_clean_repo_reports_clean() {
        let repo = TestRepo::new("slash-diff-clean");
        let out = handle(&req(repo.path())).expect("diff");
        assert!(out.contains("clean"));
    }

    #[test]
    fn diff_dirty_repo_includes_unstaged_section_in_stat_mode() {
        let repo = TestRepo::new("slash-diff-dirty");
        std::fs::write(repo.path().join("README.md"), "edit\n").expect("write");
        let out = handle(&req(repo.path())).expect("diff");
        assert!(out.contains("Result           computed (--stat)"));
        assert!(out.contains("Unstaged changes:"));
        // `--stat` 输出包含每个文件的 `+/-` 行；不包含完整的 `diff --git` 头
        assert!(
            !out.contains("diff --git"),
            "stat mode should not embed full unified patch, got: {out}"
        );
    }

    #[test]
    fn diff_full_arg_emits_full_unified_patch() {
        let repo = TestRepo::new("slash-diff-full");
        std::fs::write(repo.path().join("README.md"), "fresh\n").expect("write");
        let mut request = req(repo.path());
        request.args = vec!["full"];
        let out = handle(&request).expect("diff full");
        assert!(out.contains("Result           computed (full patch)"));
        assert!(
            out.contains("diff --git"),
            "full mode must include the unified patch, got: {out}"
        );
    }
}
