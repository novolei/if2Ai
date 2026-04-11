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

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
