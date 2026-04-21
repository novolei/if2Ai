# CLAUDE.md — If2Ai Agent 行为规范

> 项目唯一的 agent 执行规范。
> 取代旧版 17 步 SOP（已归档至 `docs/_legacy/exec-plans/` + `docs/_legacy/refactor-v1/`）。
>
> 最后更新: 2026-04-21

---

## 0. 角色与流程

你是 **If2Ai 的代码 agent**。所有工作通过 **Pack** 驱动：

> Pack = 一份 ≤ 60 行的 markdown 文件，同时承载设计意图、可验证规格、文件作用域、不变量、范围外清单。
> 它替代了过去的 design-doc → product-spec → exec-plan-yaml → runbook 漏斗。

**统一流水线（5 步，必须在一个会话内完成）**：

```
① PACK   人写 1 个 Pack（agent 不写 Pack，只读）
② BUILD  Agent 只读 1 个 Pack + Pack 中点名的现有代码，按 Spec 实现
③④ RUN  ./scripts/pack run <PACK-ID>   ⭐ 一条命令完成 lint + verify + review prompt
         FAIL 时自动打印"agent feedback block"（含 evidence + fix_prompt 字段），
         直接贴回 executor 即可重试
⑤ COMMIT 1 PR、1 commit、更新 docs/packs/REGISTRY.md 状态为 done
```

verify 或 review 失败 → 看 fix_prompt 修代码 → 重跑 `pack run`。失败 3 次仍不过 → STOP 并报告原因。

**关键设计**：每个 FAIL 都带 `fix_prompt` 字段，描述"该怎么修"。Agent 拿到反馈后照做，不需要"理解错误"——这是 close-loop 的核心。

---

## 1. 启动路由（看用户消息选 Pack）

| 用户说什么                                       | 你做什么                                            |
| ------------------------------------------------ | --------------------------------------------------- |
| "执行 GFR-XXX" / "重构 god-file" / "拆 god-file" | 读 `docs/packs/refactor/GFR-XXX-*.md`               |
| "执行 FEAT-XXX" / "实现 CPD-XXX" / "新功能 X"    | 读 `docs/packs/feature/<...>/<PACK-ID>-*.md`        |
| "改一下这个 bug" / 单文件小修 ad-hoc             | 直接做，不需要 Pack                                 |
| 模糊 / 找不到对应 Pack                           | 问用户，**不要默认**；如确需新 Pack，请用户先写一份 |

---

## 2. 启动后必读（≤ 3 个文件）

1. `docs/packs/CHARTER.md` — 不变量、Pack 类型、hard rules
2. 当前 Pack 文件本身
3. （仅 refactor）`docs/packs/snapshots/<PACK-ID>/before.json`

**禁止漫读**：
- ❌ `docs/design-docs/` / `docs/product-specs/` / `docs/staff-remediation/` — 仅当 Pack 显式点名某文件某章节时才按路径读
- ❌ `docs/_legacy/**` — 整个目录禁读，包括 exec-plans / 老 implementation-packs / refactor-v1
- ❌ `harness/runner.py` 的 `--slice` / `--diff-gate` / `--review` / `--promote` / `--check-slice` — 已废弃

---

## 3. 编码硬规则

| 规则                                                 | 检查方式               |
| ---------------------------------------------------- | ---------------------- |
| 无 `unwrap()` / `expect()`（测试除外）               | code-reviewer + clippy |
| 无 `todo!()` / `unimplemented!()`                    | code-reviewer          |
| 无硬编码 API key / secret                            | code-reviewer          |
| 跨模块用 `crate::modules::*`，不直接 `crate::xxx`    | clippy + code-reviewer |
| 所有 `pub fn` 有 `///` 注释                          | code-reviewer          |
| 异步用 tokio，不用 `std::thread`                     | code-reviewer          |
| `Result` 错误传播用 `?`，不嵌 `match`                | code-reviewer          |
| 默认 `pub(crate)`，`pub` 仅在边界真实存在时          | code-reviewer          |
| 一个文件一个有界职责，超 800 行入 god-file watchlist | `./scripts/pack scan`  |
| 不引入 Pack 没声明的新 dependency                    | code-reviewer          |

完整 lint 命令：

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --all
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

---

## 4. Commit 规范

```
<type>(<PACK-ID>): <简短描述>

<可选详细说明>

Pack: <PACK-ID>
Verify: PASS
Review: PASS
```

`type` ∈ `feat | fix | refactor | test | chore | perf | docs`。一个 commit 只能涵盖一个 Pack。

---

## 5. 阻塞处理

| 情况                    | 行动                                                           |
| ----------------------- | -------------------------------------------------------------- |
| verify FAIL 超 3 次     | STOP，在 Pack 内追加 `## Blocked` 段说明，等人工               |
| Pack 描述与代码现状不符 | STOP，问用户，不要自行扩大 Pack 范围                           |
| 依赖的 Pack 未完成      | 检查 REGISTRY，若是 pending 先执行；若 active 在他人手里则等待 |
| 编译错误超出 Pack 范围  | 只修与本 Pack 相关的；其余记录到 `## Blocked`                  |

---

## 6. 不要做的事

- ❌ 不要写 / 改 Pack 文件本身（人写、人改；agent 只读）
- ❌ 不要读 `docs/_legacy/**`
- ❌ 不要跑 `harness run --slice` / `--diff-gate` / `--promote` / `--check-slice` / `--review`
- ❌ 不要在一个 PR 里塞两个 Pack
- ❌ 不要在 Pack 没列出的文件里做修改（即使"看起来很相关"）
- ❌ 不要顺手"优化"/"清理"代码，除非 Pack 明文要求

---

## 7. 快速参考

```bash
./scripts/pack run GFR-001                         # ⭐ 主入口：lint + verify + review 全跑
./scripts/pack snapshot GFR-001 --phase before     # refactor: 单独拍快照（pack run 已自动跑）
./scripts/pack verify GFR-001                      # 单独验证
./scripts/pack scan                                 # 刷新 REGISTRY LOC
./scripts/pack init FEAT-042 --type feature \
    --slug some-thing --files path/to/a.rs         # 新建 stub
./scripts/pack suite harness/suites/<name>.yaml    # 可选：跑 behavior suite
python3 scripts/lint_architecture.py               # 单独跑架构 lint
```

---

## 8. 老版本去哪了

- 老 17 步 SOP / Phase / YAML / harness diff-gate / promote → `docs/_legacy/`
- 老 design-docs / product-specs → 保留原位作参考库，**不再启动必读**
- 老 `if2ai-slice-runner` Cursor skill → 已删除
- 老 CHARTER (refactor-only) → `docs/_legacy/refactor-v1/CHARTER.md`
