"""
harness/runner.py — CLI entry point for harness gate execution.

Usage (called by executor or manually):
  python -m harness.runner run --slice 1.2.1 --workspace /path/to/project
  python -m harness.runner run --slice 1.2.1 --workspace . --suite harness/suites/agent_basic.yaml
  python -m harness.runner check-slice --file docs/exec-plans/active/phase-1-foundation.yaml
  python -m harness.runner review --slice 1.2 --workspace .  # automated static code review
  python -m harness.runner promote --workspace .          # semi-auto: promote next draft phase
  python -m harness.runner promote --workspace . --dry-run  # preview only

Exit codes:
  0  = all gates passed / review passed
  1  = at least one gate failed / review failed
  2  = usage / config error
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


def cmd_run(args: argparse.Namespace) -> int:
    from harness.gate import run_all_gates

    workspace = Path(args.workspace).resolve()

    # Extract symbol checks from slice YAML if available
    symbol_checks: list = []
    slice_yaml_path = _find_slice_yaml(workspace, args.slice)
    if slice_yaml_path:
        try:
            import yaml  # type: ignore
            with open(slice_yaml_path) as f:
                plan = yaml.safe_load(f)
            slices = plan.get("slices", []) if isinstance(plan, dict) else []
            for s in slices:
                if str(s.get("id", "")) == str(args.slice):
                    for acc in s.get("acceptance", []):
                        if acc.get("type") == "symbol":
                            symbol_checks.append({
                                "file": acc["file"],
                                "grep": acc["grep"],
                                "label": acc.get("label", acc["grep"]),
                            })
                    break
        except Exception:
            pass  # Non-fatal: symbol checks are best-effort

    report = run_all_gates(
        slice_id=args.slice,
        workspace_root=workspace,
        package=args.package,
        test_filter=args.test_filter or None,
        suite_path=args.suite or None,
        symbol_checks=symbol_checks or None,
        skip_behavior=args.skip_behavior,
    )

    # Always emit the summary to stderr so it's visible in CI logs
    print(report.summary(), file=sys.stderr)

    # Emit JSON report to stdout for machine parsing
    print(report.to_json())

    # Optionally write report to file
    if args.report_out:
        out_path = Path(args.report_out)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(report.to_json())

    return 0 if report.passed else 1


def _find_slice_yaml(workspace: Path, slice_id: str) -> "Path | None":
    """Locate the exec-plan YAML file containing the given slice id."""
    active_dir = workspace / "docs" / "exec-plans" / "active"
    if not active_dir.exists():
        return None
    for yaml_file in active_dir.glob("*.yaml"):
        try:
            content = yaml_file.read_text()
            if f'id: "{slice_id}"' in content or f"id: '{slice_id}'" in content:
                return yaml_file
        except Exception:
            pass
    return None


def cmd_check_slice(args: argparse.Namespace) -> int:
    """Validate a slice YAML file for required fields."""
    import yaml  # type: ignore

    slice_file = Path(args.file)
    if not slice_file.exists():
        print(f"ERROR: file not found: {slice_file}", file=sys.stderr)
        return 2

    with slice_file.open() as f:
        data = yaml.safe_load(f)

    slices = data.get("slices", [])
    if not slices:
        print("ERROR: no 'slices' key found in file", file=sys.stderr)
        return 2

    errors = []
    for s in slices:
        sid = s.get("id", "<no id>")
        if not s.get("title"):
            errors.append(f"slice {sid}: missing 'title'")
        if not s.get("design_ref"):
            errors.append(f"slice {sid}: missing 'design_ref' (executor cannot load context without it)")
        if not s.get("impl_targets"):
            errors.append(f"slice {sid}: missing 'impl_targets' (executor needs target file paths)")
        if not s.get("acceptance"):
            errors.append(f"slice {sid}: missing 'acceptance' criteria")
        if not s.get("review_checklist"):
            errors.append(f"slice {sid}: missing 'review_checklist'")

    if errors:
        for e in errors:
            print(f"  ❌ {e}", file=sys.stderr)
        return 1

    print(f"✅ {len(slices)} slices validated OK in {slice_file}", file=sys.stderr)
    return 0


def cmd_diff_gate(args: argparse.Namespace) -> int:
    """Verify that the current git diff contains actual source code changes.

    A slice cannot be marked done if only documentation/YAML files were changed.
    This gate fails if there are no .rs or .ts/.tsx code file changes since the
    last commit (or since a specified base ref).

    Exit codes:
      0 = code changes found (gate passes)
      1 = no code changes found (gate fails — slice must not be marked done)
      2 = usage error
    """
    import subprocess

    workspace = Path(args.workspace).resolve()
    base_ref = args.base or "HEAD~1"
    min_lines = args.min_lines

    # Get changed files between base_ref and working tree (staged + unstaged)
    result = subprocess.run(
        ["git", "diff", "--name-only", base_ref, "HEAD"],
        cwd=workspace, capture_output=True, text=True,
    )
    if result.returncode != 0:
        # Try just staged changes if HEAD~1 fails (first commit case)
        result = subprocess.run(
            ["git", "diff", "--name-only", "--cached"],
            cwd=workspace, capture_output=True, text=True,
        )

    changed_files = [f.strip() for f in result.stdout.splitlines() if f.strip()]

    # Also include unstaged changes
    result2 = subprocess.run(
        ["git", "diff", "--name-only"],
        cwd=workspace, capture_output=True, text=True,
    )
    changed_files += [f.strip() for f in result2.stdout.splitlines() if f.strip()]
    changed_files = list(set(changed_files))

    code_extensions = {".rs", ".ts", ".tsx", ".js", ".jsx", ".py"}
    # Exclude harness infra, docs, and integration-test harness files.
    # Use path-prefix checks, not substring (avoids false exclusions on "test" in paths).
    exclude_prefixes = ("harness/", "docs/", "spec/")
    exclude_suffixes = (".test.", ".spec.", "_test.", "_spec.")

    code_files = [
        f for f in changed_files
        if any(f.endswith(ext) for ext in code_extensions)
        and not f.startswith(exclude_prefixes)
        and not any(f.endswith(suf) for suf in exclude_suffixes)
    ]

    # Count lines changed in code files
    lines_changed = 0
    if code_files:
        stat_result = subprocess.run(
            ["git", "diff", "--stat", base_ref, "HEAD", "--"] + code_files,
            cwd=workspace, capture_output=True, text=True,
        )
        import re
        m = re.search(r"(\d+) insertion", stat_result.stdout)
        if m:
            lines_changed = int(m.group(1))

    print(f"\n  Diff Gate — base: {base_ref}")
    print(f"  Changed files   : {len(changed_files)} total")
    print(f"  Code files      : {len(code_files)}")
    if code_files:
        for f in code_files[:10]:
            print(f"    + {f}")
        if len(code_files) > 10:
            print(f"    ... and {len(code_files)-10} more")
    print(f"  Lines inserted  : {lines_changed} (minimum required: {min_lines})")

    if not code_files:
        print(f"\n  ❌ DIFF_GATE FAIL: No source code files changed.")
        print(f"     Only docs/YAML changes found — slice must NOT be marked done.")
        print(f"     Implement the code in impl_targets first.\n")
        return 1

    if lines_changed < min_lines:
        print(f"\n  ❌ DIFF_GATE FAIL: Too few lines changed ({lines_changed} < {min_lines}).")
        print(f"     This looks like a docs-only update. Implement actual code.\n")
        return 1

    print(f"\n  ✅ DIFF_GATE PASS: {len(code_files)} code files, {lines_changed} lines inserted.\n")
    return 0


def cmd_review(args: argparse.Namespace) -> int:
    """Automated static code review — replaces sub-agent review step.

    Runs a series of static checks against the workspace and emits
    per-check PASS/FAIL lines, ending with REVIEW_PASS or REVIEW_FAIL.
    Claude Code can call this directly instead of spawning a sub-agent.

    Checks performed:
      1. cargo fmt --check          — formatting
      2. cargo clippy -D warnings   — lint
      3. no unwrap() outside tests  — safety
      4. no todo!()/unimplemented!()— completeness
      5. no hardcoded secrets       — security
      6. pub fn has doc comments    — documentation
      7. review_checklist items     — slice-specific (reported as manual reminder)
    """
    import subprocess
    import re
    import yaml  # type: ignore

    workspace = Path(args.workspace).resolve()
    package = args.package
    failed: list[str] = []
    passed: list[str] = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        if ok:
            passed.append(name)
            print(f"  PASS: {name}")
        else:
            failed.append(name)
            print(f"  FAIL: {name}" + (f"\n        {detail}" if detail else ""))

    print(f"\n{'─'*60}")
    print(f"  Harness Review — slice: {args.slice}  package: {package}")
    print(f"{'─'*60}\n")

    # ── 1. cargo fmt --check ────────────────────────────────────────────────
    r = subprocess.run(
        ["cargo", "fmt", "--all", "--", "--check"],
        cwd=workspace, capture_output=True, text=True,
    )
    check("cargo fmt --check", r.returncode == 0,
          "Run `cargo fmt --all` to fix formatting")

    # ── 2. cargo clippy ─────────────────────────────────────────────────────
    r = subprocess.run(
        ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
        cwd=workspace, capture_output=True, text=True,
    )
    check("cargo clippy -D warnings", r.returncode == 0,
          r.stderr[-600:] if r.returncode != 0 else "")

    # ── 3-6. static grep checks over src-tauri/src ──────────────────────────
    src_dir = workspace / "src-tauri" / "src"
    rust_files = list(src_dir.rglob("*.rs")) if src_dir.exists() else []

    unwrap_violations: list[str] = []
    todo_violations: list[str] = []
    secret_violations: list[str] = []
    undoc_violations: list[str] = []

    secret_pat  = re.compile(r'(sk-[A-Za-z0-9]{20,}|AIza[A-Za-z0-9_-]{35}|"[A-Za-z0-9+/]{40,}")')
    pub_fn_pat  = re.compile(r'^\s*pub\s+(async\s+)?fn\s+\w+')
    doc_pat     = re.compile(r'^\s*///')

    for fpath in rust_files:
        is_test_file = "test" in fpath.name or "tests" in str(fpath)
        lines = fpath.read_text(errors="replace").splitlines()
        in_test_block = False
        for i, line in enumerate(lines):
            if "#[cfg(test)]" in line or "#[test]" in line:
                in_test_block = True
            # reset test block heuristic at module boundary
            if line.strip().startswith("mod ") and "test" not in line:
                in_test_block = False

            rel = fpath.relative_to(workspace)
            if not (is_test_file or in_test_block):
                if re.search(r'\.(unwrap|expect)\s*\(', line):
                    unwrap_violations.append(f"{rel}:{i+1}")
                if re.search(r'\b(todo!|unimplemented!)\s*\(', line):
                    todo_violations.append(f"{rel}:{i+1}")
            if secret_pat.search(line):
                secret_violations.append(f"{rel}:{i+1}")
            # Check if this line is a pub fn and has no preceding doc comment.
            # Walk backwards to collect all consecutive doc-comment / attribute lines.
            if pub_fn_pat.match(line):
                has_doc = False
                for j in range(i - 1, -1, -1):
                    prev = lines[j].strip()
                    if doc_pat.match(lines[j]):
                        has_doc = True
                        break
                    # Stop at first non-doc, non-attribute line
                    if prev and not prev.startswith("//") and not prev.startswith("#["):
                        break
                if not has_doc:
                    undoc_violations.append(f"{rel}:{i+1}  {line.strip()[:60]}")

    check("no unwrap()/expect() outside tests",
          len(unwrap_violations) == 0,
          "Found in: " + ", ".join(unwrap_violations[:5]) + ("…" if len(unwrap_violations) > 5 else ""))

    check("no todo!()/unimplemented!()",
          len(todo_violations) == 0,
          "Found in: " + ", ".join(todo_violations[:5]))

    check("no hardcoded secrets",
          len(secret_violations) == 0,
          "Suspicious: " + ", ".join(secret_violations[:3]))

    check("pub fn has /// doc comment",
          len(undoc_violations) == 0,
          "Missing docs: " + "; ".join(undoc_violations[:3]) + ("…" if len(undoc_violations) > 3 else ""))

    # ── 7. slice review_checklist reminder ──────────────────────────────────
    if args.slice:
        active_dir = workspace / "docs" / "exec-plans" / "active"
        checklist_items: list[str] = []
        for yf in active_dir.glob("phase-*.yaml"):
            with yf.open() as f:
                data = yaml.safe_load(f)
            for s in data.get("slices", []):
                if str(s.get("id")) == str(args.slice):
                    checklist_items = s.get("review_checklist", [])
                    break
        if checklist_items:
            print(f"\n  ── Slice review_checklist (manual verify) ──")
            for item in checklist_items:
                print(f"  [ ] {item}")

    # ── summary ─────────────────────────────────────────────────────────────
    print(f"\n{'─'*60}")
    total = len(passed) + len(failed)
    if failed:
        print(f"  REVIEW_FAIL  ({len(passed)}/{total} checks passed)")
        print(f"  Failed: {', '.join(failed)}")
    else:
        print(f"  REVIEW_PASS  ({len(passed)}/{total} checks passed)")
    print(f"{'─'*60}\n")

    return 0 if not failed else 1


def cmd_promote(args: argparse.Namespace) -> int:
    """Promote the next draft phase to active, and mark the current phase as completed.

    Updates two files atomically:
      1. docs/exec-plans/index.md   — [当前] → [已完成], next [草稿] → [当前]
      2. next phase YAML            — phase_status: draft → active

    Usage:
        python -m harness.runner promote --workspace .
        python -m harness.runner promote --workspace . --dry-run
    """
    import re
    import yaml  # type: ignore

    workspace = Path(args.workspace).resolve()
    index_path = workspace / "docs" / "exec-plans" / "index.md"
    active_dir = workspace / "docs" / "exec-plans" / "active"

    if not index_path.exists():
        print(f"ERROR: {index_path} not found", file=sys.stderr)
        return 2

    index_text = index_path.read_text()

    # ── find current and next phase rows in index.md ────────────────────────
    # Matches lines like: | **[当前]** | Phase 1 | ... | `path/to/file.yaml` | ... |
    current_pat = re.compile(r"\|\s*\*\*\[当前\]\*\*\s*\|.*?`([^`]+\.yaml)`", re.IGNORECASE)
    draft_pat   = re.compile(r"\|\s*\[草稿\]\s*\|.*?`([^`]+\.yaml)`", re.IGNORECASE)

    current_match = current_pat.search(index_text)
    draft_match   = draft_pat.search(index_text)

    if not current_match:
        print("ERROR: no [当前] phase found in index.md", file=sys.stderr)
        return 2
    if not draft_match:
        print("INFO: no [草稿] phase found — nothing to promote.", file=sys.stderr)
        return 0

    current_yaml_rel = current_match.group(1)
    next_yaml_rel    = draft_match.group(1)
    next_yaml_path   = workspace / next_yaml_rel

    if not next_yaml_path.exists():
        print(f"ERROR: next phase YAML not found: {next_yaml_path}", file=sys.stderr)
        return 2

    # ── preview ─────────────────────────────────────────────────────────────
    print(f"  Current phase YAML : {current_yaml_rel}  →  [已完成]")
    print(f"  Next phase YAML    : {next_yaml_rel}      →  [当前]  (phase_status: active)")

    if args.dry_run:
        print("\n  [dry-run] No files changed.")
        return 0

    # ── update index.md ─────────────────────────────────────────────────────
    # 1. first draft row → [当前]
    new_index = draft_pat.sub(
        lambda m: m.group(0).replace("[草稿]", "**[当前]**", 1),
        index_text,
        count=1,
    )
    # 2. current [当前] row → [已完成]
    new_index = current_pat.sub(
        lambda m: m.group(0).replace("**[当前]**", "[已完成]", 1),
        new_index,
        count=1,
    )

    if not args.dry_run:
        index_path.write_text(new_index)
        print(f"  ✅ Updated {index_path.relative_to(workspace)}")

    # ── update next phase YAML via proper YAML parsing ──────────────────────
    import yaml  # type: ignore
    with next_yaml_path.open() as f:
        yaml_data = yaml.safe_load(f)

    updated = False
    if isinstance(yaml_data, dict) and yaml_data.get("phase_status") == "draft":
        yaml_data["phase_status"] = "active"
        updated = True

    if updated:
        with next_yaml_path.open("w") as f:
            yaml.safe_dump(yaml_data, f, allow_unicode=True, sort_keys=False)
        print(f"  ✅ Updated {next_yaml_rel}  (phase_status: active)")
    else:
        print(f"  ⚠️  phase_status: draft not found in {next_yaml_rel} — skipped YAML update")

    print("\n  Promotion complete. Run `python -m harness.runner status` to verify.")
    return 0


def cmd_status(args: argparse.Namespace) -> int:
    """Print a live progress dashboard across all active exec-plan YAMLs."""
    import yaml  # type: ignore
    import subprocess
    import datetime

    workspace = Path(args.workspace).resolve()
    active_dir = workspace / "docs" / "exec-plans" / "active"

    if not active_dir.exists():
        print("ERROR: docs/exec-plans/active/ not found", file=sys.stderr)
        return 2

    # ── collect all YAML files sorted by phase number ──────────────────────
    yaml_files = sorted(active_dir.glob("phase-*.yaml"))
    if not yaml_files:
        print("No phase YAML files found in docs/exec-plans/active/", file=sys.stderr)
        return 2

    STATUS_ICON = {
        "done":        "✅",
        "in-progress": "🔄",
        "pending":     "⏳",
        "skip":        "⏭️ ",
        "blocked":     "🚫",
    }
    PHASE_STATUS_ICON = {
        "active":    "▶ ",
        "draft":     "📋",
        "completed": "✅",
    }

    now = datetime.datetime.now().strftime("%Y-%m-%d %H:%M")
    print(f"\n{'─'*60}")
    print(f"  If2Ai Executor Progress  [{now}]")
    print(f"{'─'*60}")

    any_active = False
    for yf in yaml_files:
        with yf.open() as f:
            data = yaml.safe_load(f)

        meta = data.get("meta", {})
        phase_num = meta.get("phase", "?")
        phase_title = meta.get("title", yf.stem)
        phase_status = meta.get("phase_status", "active")  # Phase 1 has no phase_status field → treat as active
        dashboard = data.get("dashboard", {})
        slices = data.get("slices", [])

        done = [s for s in slices if s.get("status") == "done"]
        in_prog = [s for s in slices if s.get("status") == "in-progress"]
        blocked_list = dashboard.get("blocked", [])
        current_slice = dashboard.get("current_slice", "—")
        notes = dashboard.get("notes", "")

        p_icon = PHASE_STATUS_ICON.get(phase_status, "▶ ")

        print(f"\n  {p_icon} Phase {phase_num}: {phase_title}")
        print(f"     Progress : {len(done)}/{len(slices)} slices done", end="")

        if phase_status in ("draft",):
            print(f"  [DRAFT — not yet active]")
        else:
            any_active = True
            print()
            print(f"     Current  : slice {current_slice}")

        # slice table
        for s in slices:
            sid = s.get("id", "?")
            title = s.get("title", "")[:52]
            status = s.get("status", "pending")
            icon = STATUS_ICON.get(status, "❓")
            marker = " ← working" if str(sid) == str(current_slice) and phase_status != "draft" else ""
            print(f"       {icon}  {sid:6s}  {title}{marker}")

        if blocked_list:
            print(f"     🚫 BLOCKED: {', '.join(str(b) for b in blocked_list)}")

        if notes and phase_status != "draft":
            # truncate long notes
            note_display = notes[:80] + ("…" if len(notes) > 80 else "")
            print(f"     📝 {note_display}")

    # ── recent git commits ──────────────────────────────────────────────────
    if args.git:
        print(f"\n{'─'*60}")
        print("  Recent executor commits:")
        result = subprocess.run(
            ["git", "log", "--oneline", "-8"],
            cwd=workspace,
            capture_output=True,
            text=True,
        )
        if result.returncode == 0:
            for line in result.stdout.strip().splitlines():
                print(f"    {line}")
        else:
            print("    (git log unavailable)")

    # ── harness reports ─────────────────────────────────────────────────────
    reports_dir = workspace / ".harness-reports"
    if reports_dir.exists():
        reports = sorted(reports_dir.glob("slice-*.json"))
        if reports:
            print(f"\n{'─'*60}")
            print(f"  Harness reports ({len(reports)} slices):")
            for rp in reports[-5:]:  # show last 5
                try:
                    rdata = json.loads(rp.read_text())
                    status_str = "PASS ✅" if rdata.get("passed") else "FAIL ❌"
                    ts = rdata.get("timestamp", "")[:10]
                    print(f"    [{ts}] slice {rdata.get('slice_id','?'):8s} {status_str}")
                except Exception:
                    pass

    print(f"\n{'─'*60}\n")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="python -m harness.runner",
        description="If2Ai Harness — gate runner and slice validator",
    )
    sub = parser.add_subparsers(dest="command", required=True)

    # ── run ──────────────────────────────────────────────────────────────────
    run_p = sub.add_parser("run", help="Run all gates for a slice")
    run_p.add_argument("--slice", required=True, help="Slice ID (e.g. 1.2.1)")
    run_p.add_argument(
        "--workspace", default=".", help="Workspace root (default: .)"
    )
    run_p.add_argument(
        "--package", default="if2ai-backend", help="Cargo package name"
    )
    run_p.add_argument(
        "--test-filter", default="", help="Optional cargo test filter pattern"
    )
    run_p.add_argument(
        "--suite", default="", help="Path to harness behavior suite YAML"
    )
    run_p.add_argument(
        "--skip-behavior", action="store_true", help="Skip behavior gate (Layer 3)"
    )
    run_p.add_argument(
        "--report-out", default="", help="Write JSON report to this file path"
    )
    run_p.set_defaults(func=cmd_run)

    # ── review ───────────────────────────────────────────────────────────────
    rev_p = sub.add_parser("review", help="Automated static code review (replaces sub-agent)")
    rev_p.add_argument("--slice", required=True, help="Slice ID for checklist lookup (e.g. 1.2)")
    rev_p.add_argument("--workspace", default=".", help="Workspace root (default: .)")
    rev_p.add_argument("--package", default="if2ai-backend", help="Cargo package name")
    rev_p.set_defaults(func=cmd_review)

    # ── diff-gate ─────────────────────────────────────────────────────────────
    dg_p = sub.add_parser(
        "diff-gate",
        help="Verify git diff contains actual source code changes (prevents docs-only slice completion)",
    )
    dg_p.add_argument("--workspace", default=".", help="Workspace root (default: .)")
    dg_p.add_argument(
        "--base", default=None,
        help="Base git ref to diff against (default: HEAD~1)",
    )
    dg_p.add_argument(
        "--min-lines", type=int, default=5,
        help="Minimum inserted lines of code required (default: 5)",
    )
    dg_p.set_defaults(func=cmd_diff_gate)

    # ── check-slice ───────────────────────────────────────────────────────────
    chk_p = sub.add_parser("check-slice", help="Validate slice YAML structure")
    chk_p.add_argument("--file", required=True, help="Path to slice YAML file")
    chk_p.set_defaults(func=cmd_check_slice)

    # ── status ────────────────────────────────────────────────────────────────
    st_p = sub.add_parser("status", help="Print live progress dashboard across all phases")
    st_p.add_argument(
        "--workspace", default=".", help="Workspace root (default: .)"
    )
    st_p.add_argument(
        "--git", action="store_true", help="Include recent git commits"
    )
    st_p.set_defaults(func=cmd_status)

    # ── promote ───────────────────────────────────────────────────────────────
    pr_p = sub.add_parser(
        "promote",
        help="Promote next draft phase to active (updates index.md + phase YAML)",
    )
    pr_p.add_argument(
        "--workspace", default=".", help="Workspace root (default: .)"
    )
    pr_p.add_argument(
        "--dry-run", action="store_true", help="Preview changes without writing files"
    )
    pr_p.set_defaults(func=cmd_promote)

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
