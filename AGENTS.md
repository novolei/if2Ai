# If2Ai 项目导航 (AGENTS.md)

> 简洁导航地图。详细内容链接到对应文档，**本文件 ≤ 100 行**。
>
> 最后更新: 2026-04-21

## 📍 快速开始

如果你是...

- **Agent / executor** → 读 [CLAUDE.md](./CLAUDE.md)（Pack 流水线 + hard rules）→ 找 active Pack → 干活
- **新人贡献者** → 读 [docs/packs/CHARTER.md](./docs/packs/CHARTER.md) + [docs/packs/REGISTRY.md](./docs/packs/REGISTRY.md) 看当前活跃任务
- **想重构 god-file** → 看 [docs/packs/refactor/](./docs/packs/refactor/) 的 GFR-XXX 路线图
- **想加新功能** → 写一份 FEAT-XXX Pack，落到 `docs/packs/feature/`，模板见 CHARTER §6
- **架构 / 决策者** → 用 `system-architect` subagent 写 Pack；参考库 [docs/design-docs/](./docs/design-docs/)、[docs/product-specs/](./docs/product-specs/)（不再启动必读）
- **代码审查** → 用 `code-reviewer` subagent；规则在 CHARTER §3

## 🏗️ 项目核心

**If2Ai** 是一个 Tauri + Rust + React（TypeScript）的智能体桌面应用。

```
if2Ai/
├── src/                    # React + TypeScript 前端
├── src-tauri/              # Rust 后端（Tauri 2）
│   └── src/modules/        # 核心业务（按 bounded context 分模块）
├── docs/
│   ├── packs/              # ⭐ 唯一开发流程入口
│   │   ├── CHARTER.md      #    流水线规则
│   │   ├── REGISTRY.md     #    Pack 一览
│   │   ├── refactor/       #    GFR-XXX
│   │   ├── feature/        #    FEAT-XXX / CPD-XXX
│   │   └── snapshots/      #    refactor invariant baselines
│   ├── design-docs/        # 参考资料（Pack 中可点名引用）
│   ├── product-specs/      # 参考资料（同上）
│   └── _legacy/            # ⛔ 已冷藏：exec-plans / 老 implementation-packs
├── scripts/
│   └── pack                # ⭐ 唯一 Pack 工具（snapshot / verify / review / scan / init）
└── harness/                # 仅保留 suites/ + run --suite（其余子命令已废弃）
```

详见 [ARCHITECTURE.md](./ARCHITECTURE.md)（如存在）。

## 🔄 开发流水线（5 步）

```
① PACK    人写 1 个 Pack
② BUILD   Agent 实现
③ VERIFY  ./scripts/pack verify <PACK-ID>
④ REVIEW  ./scripts/pack review <PACK-ID>
⑤ COMMIT  1 PR / 1 commit / 更新 REGISTRY 状态
```

完整规范：[CLAUDE.md](./CLAUDE.md) + [docs/packs/CHARTER.md](./docs/packs/CHARTER.md)

## 📋 关键约束

- 一个文件一个有界职责，超 800 行入 god-file watchlist
- 跨模块用 `crate::modules::*`，不直接 `crate::xxx`
- 非测试代码无 `unwrap()` / `expect()` / `todo!()`
- 所有 `pub fn` 有 `///` doc
- 异步用 tokio，不用 `std::thread`

完整 lint 合约：[docs/references/coding-style-and-lint-contract.md](./docs/references/coding-style-and-lint-contract.md)

## ⛔ 已退役（请勿再使用）

- `docs/exec-plans/` → 迁至 `docs/_legacy/exec-plans/`，agent 禁读
- `docs/implementation-packs/` (old README/TEMPLATE/ACTIVE) → 迁至 `docs/_legacy/implementation-packs/`
- `docs/refactor/` (v1) → 迁至 `docs/_legacy/refactor-v1/`
- `harness run --slice / --diff-gate / --review / --promote / --check-slice` → 已废弃
- `.cursor/skills/if2ai-slice-runner/` → 已删除
- 17 步 SOP → 由 5 步 Pack 流水线取代

## 💡 常见问题

**Q: 想加新功能从哪开始？**
A: 让 `system-architect` subagent 帮你写一份 FEAT-XXX Pack，落到 `docs/packs/feature/`。然后让 executor agent 跑这个 Pack。

**Q: harness 还能用吗？**
A: 只剩 `./scripts/pack suite <yaml>`（= `harness run --suite`）作为可选集成测试。其余子命令已退役。

**Q: 看不到 design-docs 是怎么知道设计的？**
A: 设计意图压缩在 Pack 的 Goal + Spec 段。如确需深读 design-docs，Pack 自己会在 `## Reads` 段点名某文件某章节。
