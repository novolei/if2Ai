//! `/branch [list|create <name>|switch <name>]`.

use super::super::branch::list_branches_verbose;
use super::super::command::git_ok;
use super::super::error::GitResult;
use super::SlashRequest;

const USAGE: &str = "Usage: /branch list | /branch create <name> | /branch switch <name>";

/// Dispatch a `/branch` request — list, create, or switch.
pub(crate) fn handle(req: &SlashRequest<'_>) -> GitResult<String> {
    let action = req.args.first().copied();
    let target = req.args.get(1).copied().map(str::trim);

    match normalize(action) {
        None | Some("list") => {
            let listing = list_branches_verbose(req.cwd)?;
            let trimmed = listing.trim();
            Ok(if trimmed.is_empty() {
                "Branch\n  Result           no branches found".to_string()
            } else {
                format!("Branch\n  Result           listed\n\n{trimmed}")
            })
        }
        Some("create") => match target.filter(|t| !t.is_empty()) {
            Some(name) => {
                git_ok(req.cwd, &["switch", "-c", name])?;
                Ok(format!(
                    "Branch\n  Result           created and switched\n  Branch           {name}"
                ))
            }
            None => Ok(USAGE.to_string()),
        },
        Some("switch") => match target.filter(|t| !t.is_empty()) {
            Some(name) => {
                git_ok(req.cwd, &["switch", name])?;
                Ok(format!(
                    "Branch\n  Result           switched\n  Branch           {name}"
                ))
            }
            None => Ok(USAGE.to_string()),
        },
        Some(other) => Ok(format!("Unknown /branch action '{other}'. {USAGE}")),
    }
}

fn normalize(action: Option<&str>) -> Option<&str> {
    action.map(str::trim).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::git::test_support::TestRepo;
    use std::path::Path;

    fn req<'a>(cwd: &'a Path, args: Vec<&'a str>) -> SlashRequest<'a> {
        SlashRequest {
            name: "branch",
            args,
            message: None,
            title: None,
            branch_hint: None,
            cwd,
        }
    }

    #[test]
    fn list_default_action_succeeds() {
        let repo = TestRepo::new("slash-branch-list");
        let out = handle(&req(repo.path(), vec![])).expect("list ok");
        assert!(out.contains("Result           listed"));
        assert!(out.contains("main"));
    }

    #[test]
    fn create_then_switch_round_trip() {
        let repo = TestRepo::new("slash-branch-create");
        let out = handle(&req(repo.path(), vec!["create", "feat/x"])).expect("create");
        assert!(out.contains("created and switched"));
        // Switch back to main.
        let out2 = handle(&req(repo.path(), vec!["switch", "main"])).expect("switch back");
        assert!(out2.contains("switched"));
    }

    #[test]
    fn create_without_name_returns_usage() {
        let repo = TestRepo::new("slash-branch-no-name");
        let out = handle(&req(repo.path(), vec!["create"])).expect("usage");
        assert!(out.starts_with("Usage:"));
    }

    #[test]
    fn unknown_action_returns_help() {
        let repo = TestRepo::new("slash-branch-unknown");
        let out = handle(&req(repo.path(), vec!["zzz"])).expect("unknown");
        assert!(out.contains("Unknown /branch action 'zzz'"));
    }
}
