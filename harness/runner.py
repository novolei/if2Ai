"""
harness/runner.py — CLI entry point for harness gate execution.

Usage (called by executor or manually):
  python -m harness.runner run --slice 1.2.1 --workspace /path/to/project
  python -m harness.runner run --slice 1.2.1 --workspace . --suite harness/suites/agent_basic.yaml
  python -m harness.runner check-slice --file docs/exec-plans/active/phase-1-foundation.yaml

Exit codes:
  0  = all gates passed
  1  = at least one gate failed
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
    report = run_all_gates(
        slice_id=args.slice,
        workspace_root=workspace,
        package=args.package,
        test_filter=args.test_filter or None,
        suite_path=args.suite or None,
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

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
