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
        return all(r.passed for r in self.results)

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
    """
    cmd = ["cargo", "test", "-p", package, "--", "--test-output=immediate"]
    if test_filter:
        cmd.append(test_filter)
    return _gate("test_gate", cmd, cwd=workspace_root, timeout=180)


def behavior_gate(
    workspace_root: Path,
    suite_path: Optional[str] = None,
) -> GateResult:
    """
    Layer 3: Python harness evaluate — Agent behavior matches design-docs.
    Only runs if test_gate passed. Skipped if no suite_path is provided.
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

    return _gate(
        "behavior_gate",
        [sys.executable, "-m", "harness.runner", "run", "--suite", str(suite)],
        cwd=workspace_root,
        timeout=300,
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

    # Layer 2
    r2 = test_gate(workspace_root, package, test_filter)
    report.results.append(r2)
    if not r2.passed:
        return report

    # Layer 3
    if not skip_behavior:
        r3 = behavior_gate(workspace_root, suite_path)
        report.results.append(r3)

    return report
