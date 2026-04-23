# Git Workbench — Staff UX Spec

> Companion to `git_module_migration` plan (`.cursor/plans/git_module_migration_*.plan.md`) §10.
> Owner: Staff Frontend UI/UX. Status: draft for `fe_implement` consumption.
> Last updated: 2026-04-23.

This document is the **single source of truth** that the front-end engineer (the `fe_implement` TODO) follows when wiring the Tauri IPC produced by `commands/git.rs` into the React UI. Any deviation must be logged here first.

---

## 1. Information architecture

| Surface                                        | Purpose                                                                   | Reuse / new                                                                                             |
| ---------------------------------------------- | ------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| Slash menu (existing chat composer)            | Primary entry for ad-hoc git ops; auto-completion already wired.          | Reuse: extend `suggest_slash_commands` registration only.                                               |
| **Git Workbench** drawer (new, project-scoped) | Status / diff / branch list / worktree list at a glance + write actions.  | New panel, mounted on the right side of the project view, following the existing memory drawer pattern. |
| Commit dialog (new, modal)                     | Edit commit message → commit (and optionally push + open PR).             | New modal triggered from drawer or `/commit` slash.                                                     |
| `gh` not-installed banner                      | Single-line notice in the drawer header when `gh_available()` is `false`. | New small component.                                                                                    |

Out of scope for the first cut: settings page, identity page integration, multi-project diff comparison.

---

## 2. User journeys

Each row lists the trigger, success path, failure path, and cancel path. All write paths require confirmation when the action is irreversible (commit, branch switch with unstaged changes, worktree remove, force-push variants — none in v1).

| Journey            | Trigger                                       | Success                                          | Failure                                                                                      | Cancel                    |
| ------------------ | --------------------------------------------- | ------------------------------------------------ | -------------------------------------------------------------------------------------------- | ------------------------- |
| View status        | Drawer open / `/status` (future)              | Snapshot rendered with branch header + file list | "Not a git repository" empty state                                                           | n/a                       |
| View diff          | Drawer "Diff" tab / `/diff`                   | Side-by-side or single-pane patch view           | "Working tree clean" empty state                                                             | n/a                       |
| List branches      | Drawer "Branches" tab / `/branch list`        | Verbose listing with current branch starred      | Surface error string verbatim                                                                | n/a                       |
| Create branch      | "+" button / `/branch create <name>`          | Toast "Created and switched to <name>"           | Inline form error                                                                            | Esc closes form           |
| Switch branch      | Branch row click / `/branch switch <name>`    | Switch + drawer refresh                          | Surface error string + remain on current branch                                              | Esc dismisses confirm     |
| List worktrees     | Drawer "Worktrees" tab / `/worktree list`     | Listing                                          | empty state                                                                                  | n/a                       |
| Add worktree       | Drawer "+ worktree" / `/worktree add <p> [b]` | Refresh list                                     | Inline error                                                                                 | Esc                       |
| Remove worktree    | Row delete + confirm / `/worktree remove <p>` | Refresh list                                     | Surface error                                                                                | Confirm dialog has Cancel |
| Prune worktrees    | Drawer header overflow menu                   | "Pruned" toast                                   | Surface error                                                                                | n/a                       |
| Commit             | Drawer "Commit" CTA / `/commit` (UI form)     | "Created" toast referencing message              | "no workspace changes" → idempotent skipped toast (NOT an error); other → error toast        | Modal Cancel              |
| Open PR            | Drawer "PR" CTA / `/pr`                       | "Created" or "Existing" with URL chip + Copy     | When `gh` missing → render draft body modal with "Copy" only, no error                       | Modal Cancel              |
| Open Issue         | Drawer "Issue" CTA / `/issue`                 | "Created" with URL chip + Copy                   | When `gh` missing → draft modal, no error                                                    | Modal Cancel              |
| Commit + Push + PR | Drawer "Commit + PR" CTA / `/commit-push-pr`  | Rendered multi-line response (created/existing)  | When `gh` missing → blocking error "gh required"; "no branch changes" → benign skipped state | Modal Cancel              |

---

## 3. Component / state inventory

For every panel/modal: **Loading**, **Empty**, **Error**, **Partial** (e.g. only staged diff present).

- `GitDrawer.tsx` (host, owns project workdir + tabs).
- `StatusPanel.tsx` — text snapshot, monospace; loading skeleton matches identity-page skeletons.
- `DiffPanel.tsx` — split between `Staged` and `Unstaged` sections; Empty when both are absent.
- `BranchListPanel.tsx` — virtual list; row primary action is **switch**.
- `WorktreeListPanel.tsx` — same shape as `BranchListPanel`.
- `CommitModal.tsx` — textarea with monospace + character counter; primary "Commit" disabled when blank.
- `PullRequestModal.tsx` — title input + body textarea; renders `gh missing → draft mode` flag from `gh_available()`.
- `IssueModal.tsx` — same pattern as `PullRequestModal.tsx`, shorter form.
- `CommitPushPrModal.tsx` — composite of CommitModal + PR fields with explicit `gh required` warning.

**Empty / `null` rule** (matches Rust `Option<String>`):
- `git_status` / `git_diff` returning `null` → render the panel's Empty state (NOT an error toast).
- `git_branches` / `git_worktrees` returning empty trimmed string → "No branches/worktrees" centered text.

---

## 4. IPC contract (matches `commands/git.rs`)

All commands are async. **Every command takes `cwd` at the top level of the invoke argument object** — never nested. The front-end derives `cwd` exactly once, in a custom hook `useProjectWorkdir()`, from the active project's `workdir` field. Sessions never override it.

| Command               | Args                                        | Returns                                       | Error notes                                                                                           |
| --------------------- | ------------------------------------------- | --------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `git_status`          | `{ cwd }`                                   | `string \| null`                              | n/a (best-effort)                                                                                     |
| `git_diff`            | `{ cwd }`                                   | `string \| null`                              | n/a (best-effort)                                                                                     |
| `git_branches`        | `{ cwd }`                                   | `string`                                      | NotARepository surfaced as flat string                                                                |
| `git_current_branch`  | `{ cwd }`                                   | `string`                                      | `NoBranch` for detached HEAD — render "(detached)" badge                                              |
| `git_default_branch`  | `{ cwd }`                                   | `string`                                      | falls back through origin → main → master → init.defaultBranch → current                              |
| `git_worktrees`       | `{ cwd }`                                   | `string`                                      | n/a                                                                                                   |
| `git_add_worktree`    | `{ cwd, target, branch?: string }`          | `void`                                        | Existing branch → checkout, else `-b`                                                                 |
| `git_remove_worktree` | `{ cwd, target }`                           | `void`                                        | n/a                                                                                                   |
| `git_prune_worktrees` | `{ cwd }`                                   | `void`                                        | n/a                                                                                                   |
| `git_commit`          | `{ cwd, message }`                          | `{ status: "created" \| "skipped"; message }` | `status="skipped"` is **not** an error; render benign toast                                           |
| `git_commit_push_pr`  | `{ cwd, title, body, branchHint?: string }` | `string` (formatted human-readable response)  | gh missing → blocking error                                                                           |
| `gh_available`        | `{}`                                        | `boolean`                                     | drives the draft-fallback decision in the UI                                                          |
| `gh_create_pr`        | `{ cwd, title, body, base?: string }`       | `{ url, wasExisting, base }`                  | When `gh` missing the call errors; UI must check `gh_available()` first to choose live vs. draft path |
| `gh_create_issue`     | `{ cwd, title, body }`                      | `string` (URL)                                | Same gh-missing rule                                                                                  |

**Error rendering rule**: surface errors as a single-line toast with the exact backend string in a "Show details" expander. Do not parse the string.

---

## 5. Accessibility & keyboard

- Drawer focus trap; `Esc` closes (only when no modal is on top).
- All buttons have `aria-label`; row-action menus have `aria-haspopup="menu"`.
- Tab order inside CommitModal: title (if any) → message → Cancel → Commit.
- `Cmd/Ctrl+Enter` inside CommitModal submits when the form is valid.
- Slash autocomplete: hooks unchanged; new `/branch /worktree /diff /commit /commit-push-pr /pr /issue` commands surface in the existing `suggest_slash_commands` list (already wired by `git_slash_specs()` in `commands/slash.rs`).

---

## 6. Visual & copy

- Reuse existing token system (spacing/font/colors). No new theme variables.
- All UI copy in 简体中文; backend-rendered strings (status snapshot, diff text, slash response) stay verbatim because they are diagnostic / monospace blocks.
- Toast severity:
  - Info: "Branch switched to main"
  - Success: "Commit created"
  - Warn: "no workspace changes — skipped"
  - Error: any non-skipped failure

---

## 7. E2E test checklist (handed to QA after `fe_implement` lands)

1. Open Workbench in a non-git directory → drawer shows "Not a git repository" empty state, no error toast.
2. Open in a clean repo → status returns `null`, panel shows "Working tree clean".
3. Make an unstaged change → status panel + diff panel both update on next refresh.
4. Stage a change via `git add` outside the app → diff panel "Staged" section renders.
5. `/commit` with empty message → modal disables Commit; via slash returns typed `EmptyCommitMessage`.
6. `/commit` on a clean repo → benign skipped toast, no red error.
7. Switch to a branch with uncommitted local changes → confirm modal blocks until user accepts.
8. `gh` uninstalled → `/pr` and `/issue` open the draft modal with Copy enabled.
9. `gh` installed but `git push` rejected → `/commit-push-pr` shows the underlying gh stderr verbatim.
10. Detached HEAD → `git_current_branch` returns the typed-error string; UI shows "(detached)" badge.
11. Two simultaneous Workbench drawers in two project windows → no global state leakage (each holds its own `cwd`).

---

## 8. Open questions (for product / Staff sign-off before `fe_implement`)

1. Should the drawer offer **force push**? Default = no (out of scope v1).
2. Branch search filter — virtual list with client filter, or backend search? Default = client filter (lists are short).
3. Commit dialog — should we pre-fill from a "draft" stored per-session? Default = no.
4. Telemetry events to emit on commit / push / PR? Wait for product to define.

---

## 9. Definition of Done for `fe_implement`

- [ ] Typed invoke wrappers for every command in §4 live in `src/modules/git/api.ts`.
- [ ] All panels in §3 implemented with the four loading/empty/error/partial states.
- [ ] `gh_available()` checked at drawer mount, exposed via `useGhAvailable()` hook.
- [ ] All E2E checklist items in §7 pass on macOS local dev (no CI gating yet).
- [ ] No `any` in any new file; no direct `invoke('git_*')` calls outside `src/modules/git/api.ts`.
- [ ] One pass through `code-reviewer` (per migration plan §10.3) with zero remaining issues.
