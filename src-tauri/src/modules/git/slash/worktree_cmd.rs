//! `/worktree [list|add <path> [branch]|remove <path>|prune]`.

use std::path::Path;

use super::super::error::GitResult;
use super::super::worktree;
use super::SlashRequest;

const USAGE: &str = "Usage: /worktree list \
| /worktree add <path> [branch] \
| /worktree remove <path> \
| /worktree prune";

/// Dispatch a `/worktree` request — list, add, remove, or prune.
pub(crate) fn handle(req: &SlashRequest<'_>) -> GitResult<String> {
    let action = req
        .args
        .first()
        .copied()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let path = req
        .args
        .get(1)
        .copied()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let branch = req
        .args
        .get(2)
        .copied()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    match action {
        None | Some("list") => {
            let listing = worktree::list(req.cwd)?;
            Ok(if listing.is_empty() {
                "Worktree\n  Result           no worktrees found".to_string()
            } else {
                format!("Worktree\n  Result           listed\n\n{listing}")
            })
        }
        Some("add") => match path {
            Some(p) => {
                let target = Path::new(p);
                if let Some(branch) = branch {
                    worktree::add_with_branch(req.cwd, target, branch)?;
                    Ok(format!(
                        "Worktree\n  Result           added\n  Path             {p}\n  Branch           {branch}"
                    ))
                } else {
                    worktree::add(req.cwd, target)?;
                    Ok(format!(
                        "Worktree\n  Result           added\n  Path             {p}"
                    ))
                }
            }
            None => Ok(USAGE.to_string()),
        },
        Some("remove") => match path {
            Some(p) => {
                worktree::remove(req.cwd, Path::new(p))?;
                Ok(format!(
                    "Worktree\n  Result           removed\n  Path             {p}"
                ))
            }
            None => Ok(USAGE.to_string()),
        },
        Some("prune") => {
            worktree::prune(req.cwd)?;
            Ok("Worktree\n  Result           pruned".to_string())
        }
        Some(other) => Ok(format!("Unknown /worktree action '{other}'. {USAGE}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::git::test_support::TestRepo;

    fn req<'a>(cwd: &'a Path, args: Vec<&'a str>) -> SlashRequest<'a> {
        SlashRequest {
            name: "worktree",
            args,
            message: None,
            title: None,
            branch_hint: None,
            cwd,
        }
    }

    #[test]
    fn list_default_returns_listing() {
        let repo = TestRepo::new("slash-worktree-list");
        let out = handle(&req(repo.path(), vec![])).expect("list ok");
        assert!(out.contains("Result           listed"));
    }

    #[test]
    fn add_without_path_returns_usage() {
        let repo = TestRepo::new("slash-worktree-no-path");
        let out = handle(&req(repo.path(), vec!["add"])).expect("usage");
        assert!(out.starts_with("Usage:"));
    }

    #[test]
    fn unknown_action_returns_help() {
        let repo = TestRepo::new("slash-worktree-unknown");
        let out = handle(&req(repo.path(), vec!["bogus"])).expect("unknown");
        assert!(out.contains("Unknown /worktree action 'bogus'"));
    }

    #[test]
    fn add_with_path_and_branch_creates_worktree() {
        let repo = TestRepo::new("slash-worktree-add");
        // Place the new worktree inside the TempDir so cleanup is
        // tied to TestRepo's drop (Issue P2 from the worktree review).
        let target = repo.path().join("wt-add");
        let target_str = target.to_string_lossy().into_owned();
        let out = handle(&req(
            repo.path(),
            vec!["add", target_str.as_str(), "feature/wt"],
        ))
        .expect("add ok");
        assert!(out.contains("Result           added"));
        assert!(out.contains("feature/wt"));

        // Sanity check: the worktree shows up in the listing.
        let listing = handle(&req(repo.path(), vec!["list"])).expect("list");
        assert!(listing.contains(target_str.as_str()) || listing.contains("feature/wt"));
    }
}
