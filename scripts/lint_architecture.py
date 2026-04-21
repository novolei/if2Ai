#!/usr/bin/env python3
"""lint-architecture — Mechanical enforcement of CHARTER invariants.

Implements three checks:

  1. Bounded-context membership: every directory under
     `src-tauri/src/modules/` and `src/modules/` must be either
     listed in CHARTER §1 destination_map or in the REGISTRY
     watchlist (transient).
  2. File size limits per CHARTER §5 (backend ≤ 500/800; frontend
     component ≤ 300/500; hooks ≤ 200/400; pack docs ≤ 60/100).
  3. Forbidden directory names: `utils/`, `helpers/`, `common/`,
     `misc/` (un-bounded buckets).

Every FAIL emits an `evidence` + `fix_prompt` line — the latter is
agent-readable: a one-shot instruction that, if followed, fixes the
violation. This is what makes lint output composable into the Ralph
Wiggum loop.

Exit codes:
  0  — no violations
  1  — violations found (printed to stdout; structured JSON to
       --json-out if requested)
  2  — script error
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass, asdict, field
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
CHARTER = REPO_ROOT / "docs" / "packs" / "CHARTER.md"
REGISTRY = REPO_ROOT / "docs" / "packs" / "REGISTRY.md"

BACKEND_MODULES = REPO_ROOT / "src-tauri" / "src" / "modules"
FRONTEND_MODULES = REPO_ROOT / "src" / "modules"

FORBIDDEN_DIR_NAMES = {"utils", "helpers", "common", "misc"}

# CHARTER §5 thresholds.
SIZE_LIMITS = {
    ".rs":  (500, 800),    # backend module file
    ".tsx": (300, 500),    # frontend component
    ".ts":  (200, 400),    # hooks / utility ts
    ".jsx": (300, 500),
    ".js":  (200, 400),
}

# Files exempt from size checks: known god-files actively tracked in
# docs/packs/REGISTRY.md (Tier 0 / Tier 1 / Tier 2 + frontend known
# debt). The lint's job is to catch NEW debt, not re-yell about already
# scheduled refactors. When a GFR pack ships, remove its file from here.
SIZE_EXEMPT_PATHS = {
    # Tier 0
    "src-tauri/src/commands/agent.rs",
    "src-tauri/src/main.rs",
    "src/components/ui/chat-ui.tsx",
    "src/App.tsx",
    "src/lib/tauri.ts",
    # Tier 1 backend (GFR-T1-A ~ I)
    # plugins/lib.rs + commands/lib.rs deleted as orphan dead code
    # 2026-04-21 (GFR-T1-A cancelled).
    # runtime/config.rs renamed to runtime/config/mod.rs in GFR-T1-B-1
    # then sliced through GFR-T1-B-2/3/4/5/tests; mod.rs now 711 LOC
    # (under 800 hard limit; still > 500 target — flagged as warning).
    # mcp_stdio sliced through GFR-T1-C-1/2/3; mod.rs now 270 LOC
    # (under 500 target) and removed from exempt list. Only tests.rs
    # remains exempt — it's the externalized #[cfg(test)] block (914 LOC,
    # 23 tests; could be split per-test-group in a follow-up if needed).
    "src-tauri/src/modules/runtime/mcp_stdio/tests.rs",
    # runtime/prompt.rs sliced through GFR-T1-G-1; mod.rs now 428 LOC
    # (under 500 target — no exempt needed; entry kept removed).
    "src-tauri/src/modules/runtime/conversation.rs",
    # sqlite_provider sliced through GFR-T1-D-1; mod.rs now 268 LOC (under 500
    # target). provider_impl.rs (594) + tests.rs (643) remain large but are
    # already-isolated single-responsibility files; OK to leave on warning shelf.
    "src-tauri/src/modules/memory/providers/sqlite_provider/provider_impl.rs",
    "src-tauri/src/modules/memory/providers/sqlite_provider/tests.rs",
    # ticker sliced through GFR-T1-F-1; mod.rs now 526 LOC (under 800 hard
    # limit, slightly over 500 target — kept exempt to silence warning).
    "src-tauri/src/modules/memory/ticker/mod.rs",
    "src-tauri/src/commands/tts.rs",
    # Tier 2 frontend (GFR-T2-A ~ C)
    "src/modules/onboarding/steps/ProviderSetupStep.tsx",
    "src/modules/settings/pages/SkillsSettingsPage.tsx",
    "src/components/ProjectRail.tsx",
    # Frontend known debt (candidates for future GFR-T2-D...)
    "src/modules/onboarding/steps/SystemCheckStep.tsx",
    "src/modules/onboarding/hooks/useOnboarding.ts",
    "src/modules/settings/pages/AgentVoicePicker.tsx",
    "src/modules/settings/pages/TtsProfilesPage.tsx",
    "src/modules/settings/pages/ModelSettingsPage.tsx",
    "src/modules/settings/pages/MemorySettingsPage.tsx",
    "src/modules/settings/pages/StrategyDiagnosticsPage.tsx",
}


@dataclass
class Violation:
    check: str
    severity: str          # "error" | "warning"
    file: str
    evidence: str
    fix_prompt: str
    extra: dict = field(default_factory=dict)


# ---------------------------------------------------------------------------
# Charter / Registry parsing


def parse_allowed_contexts() -> set[str]:
    """Extract first-level module names mentioned in CHARTER §1 + REGISTRY."""
    allowed: set[str] = set()
    for source in (CHARTER, REGISTRY):
        if not source.exists():
            continue
        text = source.read_text(encoding="utf-8")
        # match `src-tauri/src/modules/<name>/...` or `src/modules/<name>/...`
        for m in re.finditer(
            r"src(?:-tauri/src)?/modules/([A-Za-z0-9_\-]+)/?", text
        ):
            allowed.add(m.group(1))
        # match `application/<file>.rs` shorthand
        for m in re.finditer(r"`?application/([A-Za-z0-9_]+)\.rs`?", text):
            allowed.add("application")
    # Built-in always-allowed (existed before god-file regime; real
    # current bounded contexts in the codebase). New additions outside
    # this list MUST go to CHARTER §1 destination_map.
    allowed.update({
        # backend
        "application", "control_plane", "runtime", "harness", "learning",
        "memory", "tools", "session", "skills", "browser", "tts", "stt",
        "api", "plugins", "commands", "channel", "config", "projects",
        "provider", "scheduler", "security", "system_check",
        # frontend
        "boot", "chat", "preferences", "permission", "tool-projection",
        "markdown", "skills-report", "project-rail", "boot-shell",
        "shell-router", "window-bridge", "system-feedback", "settings",
        "onboarding", "memory-management", "execution-mode",
        "strategy-diagnostics", "agent-debug", "voice-settings",
        "browser-settings", "skills-hub", "app-shell", "browser-viewer",
    })
    return allowed


def watchlist_paths() -> set[str]:
    """Files explicitly listed in REGISTRY watchlist are temporarily exempt."""
    if not REGISTRY.exists():
        return set()
    text = REGISTRY.read_text(encoding="utf-8")
    paths: set[str] = set()
    for m in re.finditer(r"`([^`]+\.(?:rs|tsx?|jsx?))`", text):
        paths.add(m.group(1))
    return paths


# ---------------------------------------------------------------------------
# Checks


def check_bounded_contexts(allowed: set[str]) -> list[Violation]:
    out: list[Violation] = []
    for root in (BACKEND_MODULES, FRONTEND_MODULES):
        if not root.exists():
            continue
        for child in sorted(root.iterdir()):
            if not child.is_dir():
                continue
            name = child.name
            if name in FORBIDDEN_DIR_NAMES:
                rel = str(child.relative_to(REPO_ROOT))
                out.append(Violation(
                    check="forbidden_directory_name",
                    severity="error",
                    file=rel,
                    evidence=f"directory `{name}` is in FORBIDDEN_DIR_NAMES",
                    fix_prompt=(
                        f"Rename or split `{rel}/` into a bounded context. "
                        "CHARTER §1 forbids unbounded buckets (utils/helpers/"
                        "common/misc). Pick a context from CHARTER §1 "
                        "destination_map or propose a new one in REGISTRY."
                    ),
                ))
                continue
            if name not in allowed:
                rel = str(child.relative_to(REPO_ROOT))
                out.append(Violation(
                    check="unknown_bounded_context",
                    severity="error",
                    file=rel,
                    evidence=(
                        f"directory `{name}` is not in CHARTER §1 "
                        "destination_map nor in REGISTRY watchlist"
                    ),
                    fix_prompt=(
                        f"Either (a) add `{name}` to CHARTER §1 destination_map "
                        f"explaining which bounded context it belongs to, or "
                        f"(b) move {rel}/ contents into an existing context "
                        f"directory."
                    ),
                ))
    return out


def check_file_sizes(watchlist: set[str]) -> list[Violation]:
    out: list[Violation] = []
    for root in (BACKEND_MODULES, FRONTEND_MODULES):
        if not root.exists():
            continue
        for path in root.rglob("*"):
            if not path.is_file():
                continue
            suffix = path.suffix
            if suffix not in SIZE_LIMITS:
                continue
            rel = str(path.relative_to(REPO_ROOT))
            if rel in SIZE_EXEMPT_PATHS or rel in watchlist:
                continue
            try:
                loc = sum(1 for _ in path.open("r", encoding="utf-8", errors="replace"))
            except OSError:
                continue
            target, hard = SIZE_LIMITS[suffix]
            if loc > hard:
                out.append(Violation(
                    check="file_size_exceeds_hard_limit",
                    severity="error",
                    file=rel,
                    evidence=f"{loc} LOC > hard limit {hard}",
                    fix_prompt=(
                        f"This file exceeds CHARTER §5 hard limit. "
                        f"Promote to god-file watchlist and create a refactor "
                        f"pack: `./scripts/pack init GFR-NEW --type refactor "
                        f"--slug split-{path.stem.replace('_','-')} "
                        f"--files {rel}`. "
                        f"Then split into ≤ {target}-LOC files within {path.parent.relative_to(REPO_ROOT)}/."
                    ),
                    extra={"loc": loc, "target": target, "hard": hard},
                ))
            elif loc > target:
                out.append(Violation(
                    check="file_size_exceeds_target",
                    severity="warning",
                    file=rel,
                    evidence=f"{loc} LOC > target {target} (hard limit {hard})",
                    fix_prompt=(
                        f"Approaching hard limit. Plan a split before {hard} "
                        f"or add to REGISTRY watchlist with rationale."
                    ),
                    extra={"loc": loc, "target": target, "hard": hard},
                ))
    return out


# ---------------------------------------------------------------------------
# Output


def print_human(violations: list[Violation]) -> None:
    if not violations:
        print("lint-architecture: PASS")
        return
    by_sev: dict[str, list[Violation]] = {"error": [], "warning": []}
    for v in violations:
        by_sev[v.severity].append(v)
    print("=" * 72)
    print(f"lint-architecture: FAIL — {len(by_sev['error'])} error(s), "
          f"{len(by_sev['warning'])} warning(s)")
    print("=" * 72)
    for sev in ("error", "warning"):
        for v in by_sev[sev]:
            tag = "✗" if sev == "error" else "⚠"
            print(f"\n{tag} [{v.check}] {v.file}")
            print(f"  evidence:   {v.evidence}")
            print(f"  fix_prompt: {v.fix_prompt}")


def main() -> int:
    parser = argparse.ArgumentParser(prog="lint-architecture")
    parser.add_argument("--json-out", type=Path,
                        help="Write structured violations to this path.")
    parser.add_argument("--warnings-as-errors", action="store_true")
    args = parser.parse_args()

    allowed = parse_allowed_contexts()
    watchlist = watchlist_paths()

    violations: list[Violation] = []
    violations.extend(check_bounded_contexts(allowed))
    violations.extend(check_file_sizes(watchlist))

    print_human(violations)

    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(
            json.dumps([asdict(v) for v in violations], indent=2,
                       sort_keys=True) + "\n",
            encoding="utf-8",
        )

    has_error = any(v.severity == "error" for v in violations)
    has_warning = any(v.severity == "warning" for v in violations)
    if has_error or (args.warnings_as_errors and has_warning):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
