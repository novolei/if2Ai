#!/usr/bin/env python3
"""Locate the next pending slice in the current If2Ai exec-plan.

Workflow:
  1. Parse docs/exec-plans/index.md, find the row marked `**[当前]**` (NOT
     wrapped in ~~strikethrough~~) and extract its backtick-wrapped YAML path.
  2. Load that YAML, scan `slices` for the first entry whose `status` is
     `pending`.
  3. Emit a compact, agent-friendly summary (Markdown by default, JSON with
     --json) containing: phase YAML path, slice id, design_ref,
     meta.design_docs (phase bibliography), depends_on, must_implement,
     impl_targets, review_checklist, plus a human_checkpoint warning if the
     slice id matches `meta.human_checkpoints[*].after_slice`.

Exit codes:
  0  found a pending slice (or PHASE_COMPLETE — see stdout)
  2  index.md / YAML structural error
  3  no `**[当前]**` row found, or multiple non-strikethrough rows found

Usage:
  .venv/bin/python .cursor/skills/if2ai-slice-runner/scripts/find_current_slice.py
  .venv/bin/python .cursor/skills/if2ai-slice-runner/scripts/find_current_slice.py --json
  .venv/bin/python .cursor/skills/if2ai-slice-runner/scripts/find_current_slice.py --workspace /path/to/repo
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

try:
    import yaml
except ImportError:
    sys.stderr.write(
        "error: pyyaml not installed. Activate .venv first:\n"
        "  source .venv/bin/activate && pip install pyyaml\n"
    )
    sys.exit(2)


CURRENT_MARK = "**[当前]**"
INDEX_REL = Path("docs/exec-plans/index.md")
BACKTICK_PATH_RE = re.compile(r"`([^`]+\.yaml)`")


def find_current_phase_yaml(workspace: Path) -> Path:
    index_path = workspace / INDEX_REL
    if not index_path.is_file():
        sys.stderr.write(f"error: not found: {index_path}\n")
        sys.exit(2)

    matches: list[tuple[int, str]] = []
    for lineno, raw in enumerate(index_path.read_text(encoding="utf-8").splitlines(), start=1):
        # Skip strikethrough rows: ~~...**[当前]**...~~
        # A line is "active current" iff it contains the literal mark and the
        # mark itself is not wrapped between ~~ on the same line.
        if CURRENT_MARK not in raw:
            continue
        # Strip ~~...~~ spans, then re-test
        stripped = re.sub(r"~~.*?~~", "", raw)
        if CURRENT_MARK not in stripped:
            continue
        matches.append((lineno, raw))

    if not matches:
        sys.stderr.write(
            f"error: no active `{CURRENT_MARK}` row found in {index_path}\n"
        )
        sys.exit(3)
    if len(matches) > 1:
        sys.stderr.write(
            f"error: {len(matches)} active `{CURRENT_MARK}` rows found "
            f"(lines {[m[0] for m in matches]}); expected exactly one\n"
        )
        sys.exit(3)

    lineno, row = matches[0]
    paths = BACKTICK_PATH_RE.findall(row)
    if not paths:
        sys.stderr.write(
            f"error: no `*.yaml` path found in current row at line {lineno}\n"
        )
        sys.exit(2)
    if len(paths) > 1:
        sys.stderr.write(
            f"warning: multiple yaml paths in current row, using first: {paths[0]}\n"
        )
    yaml_path = workspace / paths[0]
    if not yaml_path.is_file():
        sys.stderr.write(f"error: phase yaml not found: {yaml_path}\n")
        sys.exit(2)
    return yaml_path


def load_phase(yaml_path: Path) -> dict[str, Any]:
    try:
        data = yaml.safe_load(yaml_path.read_text(encoding="utf-8"))
    except yaml.YAMLError as e:
        sys.stderr.write(f"error: failed to parse {yaml_path}: {e}\n")
        sys.exit(2)
    if not isinstance(data, dict):
        sys.stderr.write(f"error: top-level of {yaml_path} is not a mapping\n")
        sys.exit(2)
    return data


def first_pending_slice(phase: dict[str, Any]) -> dict[str, Any] | None:
    slices = phase.get("slices")
    if not isinstance(slices, list):
        sys.stderr.write("error: phase yaml has no `slices` list\n")
        sys.exit(2)
    for slc in slices:
        if isinstance(slc, dict) and str(slc.get("status", "")).strip() == "pending":
            return slc
    return None


def previous_slice_is_checkpoint(phase: dict[str, Any], pending_id: str) -> str | None:
    """Return the human_checkpoint reason if the previous slice triggers one."""
    meta = phase.get("meta") or {}
    checkpoints = meta.get("human_checkpoints") or []
    if not isinstance(checkpoints, list):
        return None

    slices = phase.get("slices") or []
    pending_idx = next(
        (i for i, s in enumerate(slices) if isinstance(s, dict) and str(s.get("id")) == pending_id),
        None,
    )
    if pending_idx is None or pending_idx == 0:
        return None
    prev_id = str(slices[pending_idx - 1].get("id", ""))

    for cp in checkpoints:
        if isinstance(cp, dict) and str(cp.get("after_slice", "")) == prev_id:
            reason = str(cp.get("reason", "")).strip() or "(no reason given)"
            return f"after slice {prev_id}: {reason}"
    return None


def render_markdown(
    phase_path: Path,
    phase: dict[str, Any],
    slc: dict[str, Any] | None,
    checkpoint: str | None,
    workspace: Path,
) -> str:
    rel = phase_path.relative_to(workspace)
    meta = phase.get("meta") or {}
    phase_name = meta.get("name") or meta.get("phase") or "(unknown)"

    if slc is None:
        return (
            f"# PHASE_COMPLETE\n\n"
            f"All slices in `{rel}` are done or skipped.\n"
            f"Phase: {phase_name}\n\n"
            f"Next: human_checkpoint review or run "
            f"`python -m harness.runner promote --workspace .` (only if authorized).\n"
        )

    lines: list[str] = []
    lines.append(f"# Current Slice: {slc.get('id')}")
    lines.append("")
    lines.append(f"- **Phase YAML**: `{rel}`")
    lines.append(f"- **Phase**: {phase_name}")
    lines.append(f"- **Slice name**: {slc.get('name', '(unnamed)')}")
    lines.append(f"- **Priority**: {slc.get('priority', '(none)')}")
    lines.append(f"- **Estimated days**: {slc.get('estimated_days', '(none)')}")
    design_ref = slc.get("design_ref") or meta.get("design_ref") or "(missing)"
    lines.append(f"- **Design ref**: `{design_ref}`")

    deps = slc.get("depends_on")
    if isinstance(deps, list) and deps:
        lines.append(f"- **depends_on**: {', '.join(str(d) for d in deps)}")

    bib = meta.get("design_docs")
    if isinstance(bib, list) and bib:
        lines.append("")
        lines.append("## Phase bibliography (`meta.design_docs`)")
        lines.append(
            "以下为本 Phase 相关设计文档索引；**实现本 slice 时仍以当前 slice 的 "
            "`design_ref` 为权威**，若接口/术语不明再按需打开下列文件中的对应篇。"
        )
        for p in bib:
            lines.append(f"- `{str(p).strip()}`")

    if checkpoint:
        lines.append("")
        lines.append(
            f"> ⚠️  HUMAN_CHECKPOINT before this slice: {checkpoint}\n"
            f"> Per CLAUDE.md, stop and emit `HUMAN_CHECKPOINT_REACHED` "
            f"unless explicitly authorized to continue."
        )

    def _bullets(title: str, key: str) -> None:
        items = slc.get(key)
        if not items:
            return
        lines.append("")
        lines.append(f"## {title}")
        if isinstance(items, str):
            lines.append(items.strip())
            return
        for it in items:
            lines.append(f"- {str(it).strip()}")

    cs = slc.get("current_state")
    if cs:
        lines.append("")
        lines.append("## current_state")
        lines.append(str(cs).strip())

    _bullets("must_implement", "must_implement")
    _bullets("impl_targets", "impl_targets")
    _bullets("acceptance", "acceptance")
    _bullets("review_checklist", "review_checklist")

    lines.append("")
    lines.append("---")
    lines.append("Next steps (per CLAUDE.md):")
    lines.append(f"1. Read design_ref: `{design_ref}` (primary contract for this slice)")
    lines.append(
        "2. If design_ref references other harness docs, or terms are unclear — "
        "open only the needed paths from Phase bibliography above (do not bulk-read all)."
    )
    lines.append("3. Read each impl_targets file to understand current state")
    lines.append("4. Produce a non-empty TO-DO LIST gap analysis BEFORE coding")
    lines.append(
        "5. Implement → cargo fmt + clippy + test → harness gate → review → diff-gate → commit"
    )
    return "\n".join(lines) + "\n"


def render_json(
    phase_path: Path,
    phase: dict[str, Any],
    slc: dict[str, Any] | None,
    checkpoint: str | None,
    workspace: Path,
) -> str:
    meta = phase.get("meta") or {}
    payload: dict[str, Any] = {
        "phase_yaml": str(phase_path.relative_to(workspace)),
        "phase_name": meta.get("name"),
        "phase_status": meta.get("status"),
        "design_docs": meta.get("design_docs") if isinstance(meta.get("design_docs"), list) else [],
        "human_checkpoint_warning": checkpoint,
    }
    if slc is None:
        payload["status"] = "PHASE_COMPLETE"
    else:
        payload["status"] = "PENDING_SLICE_FOUND"
        payload["slice"] = {
            "id": slc.get("id"),
            "name": slc.get("name"),
            "priority": slc.get("priority"),
            "estimated_days": slc.get("estimated_days"),
            "design_ref": slc.get("design_ref") or meta.get("design_ref"),
            "depends_on": slc.get("depends_on") if isinstance(slc.get("depends_on"), list) else [],
            "current_state": slc.get("current_state"),
            "must_implement": slc.get("must_implement", []),
            "impl_targets": slc.get("impl_targets", []),
            "acceptance": slc.get("acceptance", []),
            "review_checklist": slc.get("review_checklist", []),
        }
    return json.dumps(payload, ensure_ascii=False, indent=2) + "\n"


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    p.add_argument(
        "--workspace",
        type=Path,
        default=Path.cwd(),
        help="Repo root (defaults to CWD).",
    )
    p.add_argument("--json", action="store_true", help="Emit JSON instead of Markdown.")
    args = p.parse_args(argv)

    workspace = args.workspace.resolve()
    phase_path = find_current_phase_yaml(workspace)
    phase = load_phase(phase_path)
    slc = first_pending_slice(phase)
    checkpoint = previous_slice_is_checkpoint(phase, str(slc.get("id"))) if slc else None

    renderer = render_json if args.json else render_markdown
    sys.stdout.write(renderer(phase_path, phase, slc, checkpoint, workspace))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
