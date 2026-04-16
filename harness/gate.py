"""
Harness Gate — 三层约束门控

负责在 executor 每次完成代码后，按顺序运行三层检测。
全部通过才允许进入 git commit。

Layer 1 — compile_gate : cargo check      (< 10s 快速失败)
Layer 2 — test_gate    : cargo test       (< 60s 单元覆盖)
Layer 3 — behavior_gate: harness evaluate (< 120s Agent 行为要求)
"""

from __future__ import annotations

import json
import subprocess
import sys
import time
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Optional


# ─────────────────────────────────────────────────────────────────────────────
# Data classes
# ─────────────────────────────────────────────────────────────────────────────

class GateStatus(str, Enum):
    PASS = "pass"
    FAIL = "fail"
    SKIP = "skip"


@dataclass
class GateResult:
    gate: str
    status: GateStatus
    duration_s: float
    output: str = ""
    error: str = ""

    @property
    def passed(self) -> bool:
        return self.status == GateStatus.PASS

    def to_dict(self) -> dict:
        return {
            "gate": self.gate,
            "status": self.status.value,
            "duration_s": round(self.duration_s, 2),
            "output": self.output[-2000:] if len(self.output) > 2000 else self.output,
            "error": self.error[-1000:] if len(self.error) > 1000 else self.error,
        }


@dataclass
class HarnessReport:
    slice_id: str
    timestamp: str
    results: list[GateResult] = field(default_factory=list)

    @property
    def passed(self) -> bool:
        return all(r.passed or r.status == GateStatus.SKIP for r in self.results)

    @property
    def first_failure(self) -> Optional[GateResult]:
        return next((r for r in self.results if not r.passed), None)

    def summary(self) -> str:
        lines = [f"Harness Report — slice: {self.slice_id}"]
        for r in self.results:
            icon = "✅" if r.passed else "❌"
            lines.append(f"  {icon} {r.gate:20s} [{r.status.value}] ({r.duration_s:.1f}s)")
        overall = "PASS" if self.passed else f"FAIL at {self.first_failure.gate}"
        lines.append(f"\n  Overall: {overall}")
        return "\n".join(lines)

    def to_json(self) -> str:
        return json.dumps({
            "slice_id": self.slice_id,
            "timestamp": self.timestamp,
            "passed": self.passed,
            "results": [r.to_dict() for r in self.results],
        }, indent=2)


# ─────────────────────────────────────────────────────────────────────────────
# Internal helpers
# ─────────────────────────────────────────────────────────────────────────────

def _run(cmd: list[str], cwd: Path, timeout: int) -> tuple[int, str, str]:
    """Run a subprocess, return (returncode, stdout, stderr)."""
    t0 = time.monotonic()
    try:
        proc = subprocess.run(
            cmd,
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        return proc.returncode, proc.stdout, proc.stderr
    except subprocess.TimeoutExpired:
        elapsed = time.monotonic() - t0
        return -1, "", f"Timeout after {elapsed:.0f}s (limit={timeout}s)"
    except FileNotFoundError as e:
        return -1, "", f"Command not found: {e}"


def _gate(name: str, cmd: list[str], cwd: Path, timeout: int) -> GateResult:
    t0 = time.monotonic()
    code, out, err = _run(cmd, cwd, timeout)
    elapsed = time.monotonic() - t0
    status = GateStatus.PASS if code == 0 else GateStatus.FAIL
    return GateResult(gate=name, status=status, duration_s=elapsed, output=out, error=err)


# ─────────────────────────────────────────────────────────────────────────────
# Gate implementations
# ─────────────────────────────────────────────────────────────────────────────

def _symbol_check_gate(workspace_root: Path, checks: list[dict]) -> GateResult:
    """
    Inner helper: verify that required symbols (functions/structs/traits) exist
    in specific source files using grep.

    Each check entry:
      file: "src-tauri/src/modules/runtime/conversation.rs"
      grep: "pub async fn run_conversation"
      label: "run_conversation() — agent-loop.md §2.2"   # optional, for reporting
    """
    t0 = time.monotonic()
    failures: list[str] = []
    passes: list[str] = []

    for check in checks:
        target_file = workspace_root / check["file"]
        pattern = check["grep"]
        label = check.get("label", pattern)

        if not target_file.exists():
            failures.append(f"  MISSING FILE  {check['file']} — {label}")
            continue

        try:
            content = target_file.read_text(encoding="utf-8", errors="replace")
            # Use word-boundary regex: pattern must appear as a whole token (not inside another identifier).
            # Uses \b word boundary to avoid false positives from partial token matches.
            import re
            word_boundary = re.compile(rf"\b{re.escape(pattern)}\b")
            if word_boundary.search(content):
                passes.append(f"  ✅  {label}")
            else:
                failures.append(f"  ❌  NOT FOUND: '{pattern}' in {check['file']}\n       → {label}")
        except Exception as e:
            failures.append(f"  ❌  READ ERROR: {check['file']}: {e}")

    elapsed = time.monotonic() - t0
    output = "\n".join(passes + failures)
    if failures:
        return GateResult(
            gate="symbol_check",
            status=GateStatus.FAIL,
            duration_s=elapsed,
            output=output,
            error=f"{len(failures)} required symbol(s) not found — implement them per design_ref",
        )
    return GateResult(
        gate="symbol_check",
        status=GateStatus.PASS,
        duration_s=elapsed,
        output=output,
    )


def symbol_gate(
    workspace_root: Path,
    checks: list[dict],
) -> GateResult:
    """
    Standalone symbol existence gate (called directly, not via suite YAML).

    Checks that required public functions/structs/traits exist in source files.
    Used as a quick sanity check that impl_targets were actually modified.
    """
    return _symbol_check_gate(workspace_root, checks)


def compile_gate(workspace_root: Path, package: str = "if2ai-backend") -> GateResult:
    """
    Layer 1: cargo check — syntax and type correctness.
    Fastest gate, runs first.
    """
    return _gate(
        "compile_gate",
        ["cargo", "check", "-p", package, "--message-format=short"],
        cwd=workspace_root,
        timeout=120,
    )


def test_gate(
    workspace_root: Path,
    package: str = "if2ai-backend",
    test_filter: Optional[str] = None,
) -> GateResult:
    """
    Layer 2: cargo test — all unit tests must pass.
    Only runs if compile_gate passed.

    Uses single-threaded test execution to avoid flaky concurrent test failures.
    """
    import os
    cmd = ["cargo", "test", "-p", package]
    if test_filter:
        cmd.extend(["--", test_filter])
    env = dict(os.environ, RUST_TEST_THREADS="1")
    return _gate_with_env("test_gate", cmd, cwd=workspace_root, timeout=180, env=env)


def _gate_with_env(
    name: str, cmd: list[str], cwd: Path, timeout: int, env: dict
) -> GateResult:
    """Run a subprocess with custom env vars, return GateResult."""
    t0 = time.monotonic()
    try:
        proc = subprocess.run(
            cmd,
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=timeout,
            env=env,
        )
        elapsed = time.monotonic() - t0
        status = GateStatus.PASS if proc.returncode == 0 else GateStatus.FAIL
        return GateResult(
            gate=name, status=status, duration_s=elapsed, output=proc.stdout, error=proc.stderr,
        )
    except subprocess.TimeoutExpired:
        elapsed = time.monotonic() - t0
        return GateResult(
            gate=name, status=GateStatus.FAIL, duration_s=elapsed,
            error=f"Timeout after {elapsed:.0f}s (limit={timeout}s)",
        )


def behavior_gate(
    workspace_root: Path,
    suite_path: Optional[str] = None,
    package: str = "if2ai-backend",
) -> GateResult:
    """
    Layer 3: Harness behavior suite — validates Agent behavior matches design-docs.

    Reads the suite YAML and dispatches based on the `runner` field:
      - "cargo_test": runs `cargo test -- <test_fn>` for each test case
      - other: skipped with a warning

    Skipped if no suite_path is provided or suite file does not exist.
    """
    if not suite_path:
        return GateResult(
            gate="behavior_gate",
            status=GateStatus.SKIP,
            duration_s=0.0,
            output="No harness suite specified for this slice — skipped",
        )

    suite = workspace_root / suite_path
    if not suite.exists():
        return GateResult(
            gate="behavior_gate",
            status=GateStatus.SKIP,
            duration_s=0.0,
            output=f"Suite not found: {suite_path} — skipped",
        )

    try:
        import yaml  # type: ignore
        with suite.open() as f:
            suite_data = yaml.safe_load(f)
    except Exception as e:
        return GateResult(
            gate="behavior_gate",
            status=GateStatus.FAIL,
            duration_s=0.0,
            error=f"Failed to parse suite YAML: {e}",
        )

    runner_type = suite_data.get("runner", "cargo_test")
    test_cases = suite_data.get("test_cases", [])

    if runner_type == "symbol_check":
        return _symbol_check_gate(workspace_root, test_cases)

    if runner_type != "cargo_test":
        return GateResult(
            gate="behavior_gate",
            status=GateStatus.SKIP,
            duration_s=0.0,
            output=f"Suite runner '{runner_type}' not yet supported — skipped",
        )

    if not test_cases:
        return GateResult(
            gate="behavior_gate",
            status=GateStatus.SKIP,
            duration_s=0.0,
            output="Suite has no test_cases — skipped",
        )

    # Collect all test function names from the suite
    test_fns = [tc["test_fn"] for tc in test_cases if tc.get("test_fn")]
    if not test_fns:
        return GateResult(
            gate="behavior_gate",
            status=GateStatus.SKIP,
            duration_s=0.0,
            output="No test_fn entries found in suite test_cases — skipped",
        )

    # Run all specified test functions in one cargo test invocation
    # Cargo test filter supports multiple patterns separated by space but
    # only one filter arg is accepted; run them individually and collect.
    t0 = time.monotonic()
    all_output: list[str] = []
    for fn in test_fns:
        cmd = [
            "cargo", "test", "-p", package,
            "--", fn,
        ]
        code, out, err = _run(cmd, cwd=workspace_root, timeout=120)
        all_output.append(f"[{fn}]\n{out}{err}")
        if code != 0:
            elapsed = time.monotonic() - t0
            return GateResult(
                gate="behavior_gate",
                status=GateStatus.FAIL,
                duration_s=elapsed,
                output="\n".join(all_output),
                error=f"Test '{fn}' failed (exit {code})",
            )

    elapsed = time.monotonic() - t0
    return GateResult(
        gate="behavior_gate",
        status=GateStatus.PASS,
        duration_s=elapsed,
        output=f"All {len(test_fns)} behavior tests passed\n" + "\n".join(all_output),
    )


# ─────────────────────────────────────────────────────────────────────────────
# Main entry point
# ─────────────────────────────────────────────────────────────────────────────

def run_all_gates(
    *,
    slice_id: str,
    workspace_root: Path,
    package: str = "if2ai-backend",
    test_filter: Optional[str] = None,
    suite_path: Optional[str] = None,
    symbol_checks: Optional[list] = None,
    skip_behavior: bool = False,
) -> HarnessReport:
    """
    Run all three gates in sequence. Short-circuits on first failure.
    Returns a HarnessReport with all results.

    Usage:
        report = run_all_gates(
            slice_id="1.2.1",
            workspace_root=Path("/path/to/project"),
        )
        if not report.passed:
            print(report.summary())
            sys.exit(1)
    """
    from datetime import datetime, timezone

    report = HarnessReport(
        slice_id=slice_id,
        timestamp=datetime.now(timezone.utc).isoformat(),
    )

    # Layer 1
    r1 = compile_gate(workspace_root, package)
    report.results.append(r1)
    if not r1.passed:
        return report

    # Layer 1b — symbol existence check (verifies impl_targets were actually modified)
    if symbol_checks:
        r1b = symbol_gate(workspace_root, symbol_checks)
        report.results.append(r1b)
        if not r1b.passed:
            return report

    # Layer 2
    r2 = test_gate(workspace_root, package, test_filter)
    report.results.append(r2)
    if not r2.passed:
        return report

    # Layer 3
    if not skip_behavior:
        r3 = behavior_gate(workspace_root, suite_path, package)
        report.results.append(r3)

    return report
