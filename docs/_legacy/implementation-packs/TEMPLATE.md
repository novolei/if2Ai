# PACK-ID Title

> Implementation Pack 模板。复制本文件创建新的 pack，并删掉所有模板提示。

## Status

- State: `draft | active | blocked | completed`
- Owner: `@executor`
- Gap Module:
- Last Updated:

---

## Goal

用一句话定义这次改动的硬目标。

例：

- 把 `chat_prompt_dispatch` 主链从 command-heavy 改为 turn-service-owned execution spine。

---

## Why Now

说明为什么当前必须先做这件事，而不是别的。

建议回答：

1. 它阻断了哪条 workflow。
2. 它为什么是更大 gap 的前置条件。
3. 用户为什么能感知到它没做好。

---

## Allowed Files

只列本次允许修改的文件或目录。

- `path/a`
- `path/b`

---

## Forbidden Files

列出这次明确不能顺手扩散的范围。

- `src/modules/settings/**`
- `docs/exec-plans/**`

---

## Source Of Truth

列出本次允许引用的 If2Ai 真相文档。

- `docs/staff-remediation/gap-modules/...`
- `docs/staff-remediation/if2ai-workflow-truth.md`

---

## UClaw References

只列最相关的 UClaw 参考，不要过量。

- `/Users/.../CURRENT_WORKFLOWS.md`
- `/Users/.../planner.rs`

---

## Required Changes

按 3-6 条列出这次必须完成的代码变化。

1. ...
2. ...
3. ...

---

## Acceptance

必须可执行、可拒绝。

- `cargo check --manifest-path src-tauri/Cargo.toml`
- 新增或更新测试：
  - `cargo test --manifest-path src-tauri/Cargo.toml ...`
- 代码状态：
  - 例如“X 不再直接在 Y 中发生”
  - 例如“workflow truth 可从 partial 提升到 canonical-ready”

---

## Out Of Scope

明确写这次不做的事，防止 AI 扩散。

- 不做 ...
- 不做 ...

---

## Execution Notes

可选，写给执行型 AI 的简短说明。

- 先复述 Goal、Allowed Files、Acceptance 再开工。
- 不要读取其他规划文档，除非本 pack 明确引用。

