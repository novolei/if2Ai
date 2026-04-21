#!/usr/bin/env python3
"""rfp_helper — Snapshot / verify / scan / init for Refactor Pack Loop.

Captures lightweight, machine-checkable invariants for a structural
refactor pack:

  * pub_symbols   — declared `pub fn|struct|enum|trait|const|type` (Rust)
                    and `export <kind>` (TS/TSX).
  * test_names    — `#[test]` / `#[tokio::test]` fn names (Rust) and
                    `it(...)` / `test(...)` literals (TS).
  * event_strings — `app.emit("...")` / `emit("...")` / `listen("...")` /
                    `invoke("...")` literals (rough but stable).
  * loc           — line count per file.

The script is intentionally minimal: regex over text. We do NOT parse
ASTs because the contract is "before == after"; any false positive
that's stable across snapshots is acceptable.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections import defaultdict
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import Iterable

REPO_ROOT = Path(__file__).resolve().parent.parent
PACKS_ROOT = REPO_ROOT / "docs" / "packs"
PACKS_REFACTOR = PACKS_ROOT / "refactor"
PACKS_FEATURE = PACKS_ROOT / "feature"
SNAPSHOTS_DIR = PACKS_ROOT / "snapshots"
REGISTRY_FILE = PACKS_ROOT / "REGISTRY.md"

RUST_PUB_RE = re.compile(
    r"^\s*pub(?:\([^)]*\))?\s+"
    r"(fn|struct|enum|trait|const|static|type|mod)\s+"
    r"([A-Za-z_][A-Za-z0-9_]*)",
    re.MULTILINE,
)
RUST_TEST_RE = re.compile(
    r"#\[(?:tokio::)?test(?:\([^)]*\))?\]\s*\n\s*(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)",
)
TS_EXPORT_RE = re.compile(
    r"^\s*export\s+"
    r"(?:default\s+)?"
    r"(?:async\s+)?"
    r"(function|const|let|class|interface|type|enum)\s+"
    r"([A-Za-z_$][A-Za-z0-9_$]*)",
    re.MULTILINE,
)
TS_REEXPORT_RE = re.compile(r"^\s*export\s*\{\s*([^}]+)\}", re.MULTILINE)
EVENT_RE = re.compile(
    r"\b(?:app\.emit|emit|emit_to|listen|invoke|invokeOnce)\s*[\(:]\s*"
    r"['\"]([a-zA-Z0-9_:.\-]+)['\"]"
)


@dataclass
class FileSnapshot:
    path: str
    loc: int
    pub_symbols: list[str] = field(default_factory=list)
    test_names: list[str] = field(default_factory=list)
    event_strings: list[str] = field(default_factory=list)
    exists: bool = True


@dataclass
class PackSnapshot:
    pack_id: str
    phase: str
    files: list[FileSnapshot]


def die(msg: str, code: int = 2) -> None:
    print(f"pack: {msg}", file=sys.stderr)
    sys.exit(code)


def find_pack(pack_id: str) -> Path:
    """Search both refactor/ and feature/ subdirs."""
    matches: list[Path] = []
    for sub in (PACKS_REFACTOR, PACKS_FEATURE):
        if sub.exists():
            matches.extend(sub.glob(f"{pack_id}-*.md"))
            # also support nested layout (e.g. feature/chat-prompt-dispatch/CPD-001-*.md)
            matches.extend(sub.glob(f"*/{pack_id}-*.md"))
    matches = sorted(set(matches))
    if not matches:
        die(f"no pack file matches {pack_id}-*.md under {PACKS_ROOT}")
    if len(matches) > 1:
        die(f"multiple pack files match {pack_id}: {matches}")
    return matches[0]


def is_refactor_pack(pack_path: Path) -> bool:
    return PACKS_REFACTOR in pack_path.parents


def parse_files_from_pack(pack_path: Path) -> list[Path]:
    """Extract scope file list under '## Files (scope)' bullet block."""
    text = pack_path.read_text(encoding="utf-8")
    lines = text.splitlines()
    files: list[Path] = []
    in_block = False
    for line in lines:
        stripped = line.strip()
        if stripped.lower().startswith("## files"):
            in_block = True
            continue
        if in_block and stripped.startswith("## "):
            break
        if in_block and stripped.startswith("- "):
            payload = stripped[2:].split("#", 1)[0].strip()
            payload = payload.strip("`")
            if payload:
                files.append(REPO_ROOT / payload)
    if not files:
        die(f"pack {pack_path.name} has no '## Files (scope)' bullet list")
    return files


def snapshot_file(path: Path) -> FileSnapshot:
    rel = str(path.relative_to(REPO_ROOT))
    if not path.exists():
        return FileSnapshot(path=rel, loc=0, exists=False)
    text = path.read_text(encoding="utf-8", errors="replace")
    loc = text.count("\n") + (1 if text and not text.endswith("\n") else 0)
    pub_symbols: list[str] = []
    test_names: list[str] = []
    event_strings: list[str] = []
    suffix = path.suffix
    if suffix == ".rs":
        for kind, name in RUST_PUB_RE.findall(text):
            pub_symbols.append(f"{kind} {name}")
        test_names.extend(RUST_TEST_RE.findall(text))
    elif suffix in {".ts", ".tsx", ".js", ".jsx", ".mjs"}:
        for kind, name in TS_EXPORT_RE.findall(text):
            pub_symbols.append(f"{kind} {name}")
        for group in TS_REEXPORT_RE.findall(text):
            for raw in group.split(","):
                ident = raw.split(" as ")[0].strip()
                if ident:
                    pub_symbols.append(f"reexport {ident}")
    event_strings.extend(EVENT_RE.findall(text))
    return FileSnapshot(
        path=rel,
        loc=loc,
        pub_symbols=sorted(set(pub_symbols)),
        test_names=sorted(set(test_names)),
        event_strings=sorted(set(event_strings)),
    )


def write_snapshot(pack_id: str, phase: str, files: list[Path]) -> Path:
    snap = PackSnapshot(
        pack_id=pack_id,
        phase=phase,
        files=[snapshot_file(p) for p in files],
    )
    out_dir = SNAPSHOTS_DIR / pack_id
    out_dir.mkdir(parents=True, exist_ok=True)
    out_path = out_dir / f"{phase}.json"
    payload = {
        "pack_id": snap.pack_id,
        "phase": snap.phase,
        "files": [asdict(f) for f in snap.files],
    }
    out_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return out_path


def cmd_snapshot(args: argparse.Namespace) -> int:
    pack_path = find_pack(args.pack_id)
    files = parse_files_from_pack(pack_path)
    out = write_snapshot(args.pack_id, args.phase, files)
    print(f"pack: snapshot {args.phase} -> {out.relative_to(REPO_ROOT)}")
    print(f"     {len(files)} file(s) recorded")
    return 0


def aggregate_symbols(snapshot: dict) -> tuple[set[str], set[str], set[str], dict[str, int]]:
    pub: set[str] = set()
    tests: set[str] = set()
    events: set[str] = set()
    loc: dict[str, int] = {}
    for f in snapshot["files"]:
        pub.update(f.get("pub_symbols", []))
        tests.update(f.get("test_names", []))
        events.update(f.get("event_strings", []))
        loc[f["path"]] = f.get("loc", 0)
    return pub, tests, events, loc


def _print_feedback_block(pack_id: str, failures: list[str], kind: str) -> None:
    """Print a ready-to-paste block for the executor agent to retry."""
    print()
    print("=" * 72)
    print(f"AGENT FEEDBACK — paste the block below to executor for retry")
    print("=" * 72)
    print(f"```")
    print(f"VERIFY FAIL on {pack_id} ({kind}). Fix the violations below and")
    print(f"re-run `./scripts/pack run {pack_id}`. Do NOT widen the pack scope.")
    print(f"")
    for f in failures:
        print(f)
        print("")
    print(f"After fixing, do not modify the pack file or the charter.")
    print(f"```")


def cmd_verify(args: argparse.Namespace) -> int:
    pack_path = find_pack(args.pack_id)
    if not is_refactor_pack(pack_path):
        # Feature pack: just run cargo build + test + clippy.
        print(f"pack: feature pack — running cargo build/test/clippy for {args.pack_id}")
        feature_failures: list[str] = []
        for cmd in (
            ["cargo", "build", "--manifest-path", "src-tauri/Cargo.toml"],
            ["cargo", "test",  "--manifest-path", "src-tauri/Cargo.toml"],
            ["cargo", "clippy", "--manifest-path", "src-tauri/Cargo.toml",
             "--all-targets", "--", "-D", "warnings"],
        ):
            r = subprocess.run(cmd, cwd=REPO_ROOT)
            if r.returncode != 0:
                feature_failures.append(
                    f"✗ [{cmd[1]} failed]\n"
                    f"  evidence:   exit code {r.returncode}\n"
                    f"  fix_prompt: Re-read the {cmd[1]} output above. Address"
                    f" each error inline. Do not silence with #[allow(...)] or"
                    f" rustc cfg unless the pack explicitly permits it."
                )
                _print_feedback_block(args.pack_id, feature_failures, kind="cargo")
                return 1
        print(f"pack: VERIFY PASS — {args.pack_id} (feature)")
        print("     next: ./scripts/pack review " + args.pack_id)
        return 0

    files = parse_files_from_pack(pack_path)
    before_path = SNAPSHOTS_DIR / args.pack_id / "before.json"
    if not before_path.exists():
        die(f"missing before snapshot: {before_path.relative_to(REPO_ROOT)}")
    after_path = write_snapshot(args.pack_id, "after", files)

    before = json.loads(before_path.read_text(encoding="utf-8"))
    after = json.loads(after_path.read_text(encoding="utf-8"))
    b_pub, b_tests, b_events, b_loc = aggregate_symbols(before)
    a_pub, a_tests, a_events, a_loc = aggregate_symbols(after)

    failures: list[str] = []

    # I1 — pub symbols set-equal across union of files in scope.
    pub_added = sorted(a_pub - b_pub)
    pub_removed = sorted(b_pub - a_pub)
    if pub_added or pub_removed:
        failures.append(
            "✗ [I1 pub symbols differ]\n"
            f"  evidence:   added={pub_added}\n"
            f"              removed={pub_removed}\n"
            "  fix_prompt: Restore the removed pub symbols (re-export them"
            " via `pub use ...` if you moved them) AND/OR remove the added"
            " ones if not in the pack's Verify Whitelist. Refactor invariant"
            " I1 forbids changing the pub-symbol set."
        )

    # I2 — test names set-superset (allow new tests, never lose tests).
    tests_lost = sorted(b_tests - a_tests)
    if tests_lost:
        failures.append(
            f"✗ [I2 tests removed]\n"
            f"  evidence:   {tests_lost}\n"
            "  fix_prompt: Restore the deleted tests verbatim. CHARTER §3.2"
            " forbids removing or renaming tests during a refactor pack."
        )

    # I3 — event strings set-equal.
    events_added = sorted(a_events - b_events)
    events_removed = sorted(b_events - a_events)
    if events_added or events_removed:
        failures.append(
            "✗ [I3 event strings differ]\n"
            f"  evidence:   added={events_added}\n"
            f"              removed={events_removed}\n"
            "  fix_prompt: Event/IPC name strings must remain byte-identical."
            " Restore the original literal at every emit/listen/invoke call"
            " site. CHARTER invariant I3."
        )

    # LOC sanity: sum of LOC across scope must be approximately conserved.
    b_total = sum(b_loc.values())
    a_total = sum(a_loc.values())
    delta = a_total - b_total
    drift_pct = (abs(delta) / b_total * 100) if b_total else 0.0
    if drift_pct > 5.0:
        failures.append(
            f"✗ [LOC drift {delta:+d} ({drift_pct:.1f}% of {b_total}) > 5%]\n"
            "  evidence:   sum of LOC across scope changed too much\n"
            "  fix_prompt: A refactor pack should be near LOC-conservative."
            " Inspect the diff for accidental semantic changes (added/removed"
            " logic). Either roll back the extra changes, or split them into"
            " a separate FEAT-XXX pack."
        )

    if failures:
        print("=" * 60)
        print(f"VERIFY FAIL — {args.pack_id}")
        print("=" * 60)
        for f in failures:
            print(f)
            print("-" * 60)
        return 1

    print(f"pack: VERIFY PASS — {args.pack_id}")
    print(f"     pub_symbols : {len(b_pub)} (unchanged)")
    print(f"     test_names  : {len(a_tests)} (>= {len(b_tests)})")
    print(f"     event_strings: {len(b_events)} (unchanged)")
    print(f"     LOC delta   : {delta:+d} ({drift_pct:.1f}%)")

    if args.skip_lint:
        print("     lint        : skipped (--skip-lint)")
        return 0

    rust_in_scope = any(p.suffix == ".rs" for p in files)
    if rust_in_scope:
        print("pack: running cargo fmt --check ...")
        r = subprocess.run(
            ["cargo", "fmt", "--manifest-path", "src-tauri/Cargo.toml", "--all", "--", "--check"],
            cwd=REPO_ROOT,
        )
        if r.returncode != 0:
            print("VERIFY FAIL — cargo fmt --check failed")
            return 1
        print("pack: running cargo clippy -D warnings ...")
        r = subprocess.run(
            [
                "cargo", "clippy",
                "--manifest-path", "src-tauri/Cargo.toml",
                "--all-targets", "--", "-D", "warnings",
            ],
            cwd=REPO_ROOT,
        )
        if r.returncode != 0:
            print("VERIFY FAIL — clippy reported warnings")
            return 1

    return 0


def cmd_run(args: argparse.Namespace) -> int:
    """End-to-end half-auto loop: lint → snapshot → verify → review prompt.

    Half-auto: this runs every gate that does not require an agent CLI.
    On FAIL, prints an agent-feedback block ready to paste to executor.
    On PASS, prints the exact code-reviewer subagent invocation and the
    suggested commit message.
    """
    pack_path = find_pack(args.pack_id)
    pack_kind = "refactor" if is_refactor_pack(pack_path) else "feature"
    print(f"pack run: {args.pack_id} ({pack_kind})")
    print(f"          pack file: {pack_path.relative_to(REPO_ROOT)}")
    print()

    # 1. Architecture lint — non-blocking warnings, blocking errors.
    lint_script = REPO_ROOT / "scripts" / "lint_architecture.py"
    if lint_script.exists():
        print("─── step 1/4: lint-architecture ─────────────────────────────")
        r = subprocess.run(["python3", str(lint_script)], cwd=REPO_ROOT)
        if r.returncode != 0:
            print()
            print("✗ lint-architecture FAIL — fix the errors above before retrying.")
            return 1
    else:
        print("⚠ lint-architecture script missing; skipping.")

    print()

    # 2. Refactor: snapshot before (idempotent if already exists).
    if pack_kind == "refactor":
        before_path = SNAPSHOTS_DIR / args.pack_id / "before.json"
        if not before_path.exists():
            print("─── step 2/4: snapshot before ───────────────────────────────")
            snap_args = argparse.Namespace(pack_id=args.pack_id, phase="before")
            cmd_snapshot(snap_args)
            print()

    # 3. Verify (refactor: snapshot diff + lint; feature: cargo build/test).
    print("─── step 3/4: verify ────────────────────────────────────────")
    verify_args = argparse.Namespace(
        pack_id=args.pack_id,
        skip_lint=args.skip_lint,
    )
    rc = cmd_verify(verify_args)
    if rc != 0:
        return rc

    # 4. Review hand-off.
    print()
    print("─── step 4/4: code review ──────────────────────────────────")
    print(f"All automated gates PASS. Now invoke the code-reviewer subagent:")
    print()
    print(f"    Use the code-reviewer subagent to review pack {args.pack_id}")
    print()
    print("If reviewer outputs REVIEW_PASS, commit with this message:")
    print()
    print("─" * 72)
    pack_type = "refactor" if pack_kind == "refactor" else "feat"
    print(f"{pack_type}({args.pack_id}): <one-line summary>")
    print()
    print(f"Pack: {args.pack_id}")
    print(f"Verify: PASS")
    print(f"Review: PASS")
    print("─" * 72)
    print()
    print(f"Then update docs/packs/REGISTRY.md to mark {args.pack_id} as done.")
    return 0


def cmd_scan(_: argparse.Namespace) -> int:
    """Refresh REGISTRY.md Tier 3 LOC numbers from disk (idempotent)."""
    if not REGISTRY_FILE.exists():
        die(f"missing {REGISTRY_FILE}")
    text = REGISTRY_FILE.read_text(encoding="utf-8")
    pattern = re.compile(r"\| `([^`]+\.(?:rs|tsx?|jsx?))` \| +(\d+) +\|")
    updated_lines: list[str] = []
    changed = 0
    for line in text.splitlines():
        m = pattern.search(line)
        if not m:
            updated_lines.append(line)
            continue
        rel = m.group(1)
        f = REPO_ROOT / rel
        if not f.exists():
            updated_lines.append(line)
            continue
        loc = sum(1 for _ in f.open("r", encoding="utf-8", errors="replace"))
        old_loc = int(m.group(2))
        if loc != old_loc:
            line = line.replace(f"| {old_loc} ", f"| {loc} ")
            changed += 1
        updated_lines.append(line)
    REGISTRY_FILE.write_text("\n".join(updated_lines) + "\n", encoding="utf-8")
    print(f"pack: scan complete — {changed} LOC entries updated")
    return 0


def cmd_init(args: argparse.Namespace) -> int:
    pack_id = args.pack_id
    slug = args.slug.strip().lower().replace(" ", "-")
    if args.type not in {"refactor", "feature"}:
        die("--type must be 'refactor' or 'feature'")
    target_dir = PACKS_REFACTOR if args.type == "refactor" else PACKS_FEATURE
    target_dir.mkdir(parents=True, exist_ok=True)
    out = target_dir / f"{pack_id}-{slug}.md"
    if out.exists():
        die(f"pack already exists: {out.relative_to(REPO_ROOT)}")
    files_block = "\n".join(f"- {p}" for p in args.files)
    if args.type == "refactor":
        body = f"""# {pack_id}: <one-line goal>

## Status
- State: active

## Source
- <god-file path> (lines A–B)

## Destination
- <new module path>

## Files (scope)
{files_block}

## Contract
- Move only. Keep all pub fn signatures.
- Leave `pub use crate::modules::<dest>::*;` shim in source file.
- Charter invariants I1–I7 apply.

## Out of Scope
- Anything not literally in the lines above.

## Verify
```
./scripts/pack snapshot {pack_id} --phase before
# ... agent executes the move ...
./scripts/pack verify {pack_id}
./scripts/pack review {pack_id}
```
"""
    else:
        body = f"""# {pack_id}: <one-line goal>

## Status
- State: active

## Goal
<2–3 lines: what behavior must exist>

## Spec (verifiable — each item paired with a test name)
- <behavior 1> → test `tests::<module>::<test_name>`
- <behavior 2> → test `tests::<module>::<test_name>`

## Files (scope — write list)
{files_block}

## Reads (read-only inputs allowed beyond Files)
- <existing file path>

## Contract (review must check)
- <invariant 1>

## Out of Scope
- ❌ Do not refactor adjacent code

## Verify
- cargo build --manifest-path src-tauri/Cargo.toml
- cargo test  --manifest-path src-tauri/Cargo.toml -- <module>::
- cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
- ./scripts/pack review {pack_id}

## Done
- Verify all PASS
- REGISTRY status changed to done
"""
    out.write_text(body, encoding="utf-8")
    print(f"pack: stub written to {out.relative_to(REPO_ROOT)}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(prog="pack")
    sub = parser.add_subparsers(dest="cmd", required=True)

    p_snap = sub.add_parser("snapshot")
    p_snap.add_argument("pack_id")
    p_snap.add_argument("--phase", choices=["before", "after"], required=True)
    p_snap.set_defaults(func=cmd_snapshot)

    p_ver = sub.add_parser("verify")
    p_ver.add_argument("pack_id")
    p_ver.add_argument("--skip-lint", action="store_true",
                       help="Skip cargo fmt/clippy (snapshot diff only).")
    p_ver.set_defaults(func=cmd_verify)

    p_run = sub.add_parser("run")
    p_run.add_argument("pack_id")
    p_run.add_argument("--skip-lint", action="store_true",
                       help="Skip cargo fmt/clippy (snapshot diff only).")
    p_run.set_defaults(func=cmd_run)

    p_scan = sub.add_parser("scan")
    p_scan.set_defaults(func=cmd_scan)

    p_init = sub.add_parser("init")
    p_init.add_argument("pack_id")
    p_init.add_argument("--type", required=True, choices=["refactor", "feature"])
    p_init.add_argument("--slug", required=True)
    p_init.add_argument("--files", nargs="+", required=True)
    p_init.set_defaults(func=cmd_init)

    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
