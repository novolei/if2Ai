# Active Implementation Pack

> 当前建议作为 Cursor / Codex 唯一主执行入口的任务包。
>
> 最后更新: 2026-04-21

## Active

- [MIG-001 Canonical Chat Execution Spine](./migration-core/MIG-001-canonical-chat-execution-spine.md)

## Rules

1. 默认只执行上面列出的 active pack。
2. 若 active pack 改变，先更新本文件。
3. 若并发多个 pack，必须保证 write scope 不重叠。
