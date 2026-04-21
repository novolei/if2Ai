# Refactor Charter

> If2Ai god-file 重构的**唯一全局真相**。Agent 每次执行 GFR 都必须先读这份文件。
>
> 范围：纯结构重构（move + re-export shim），**零业务变更**。
> 不在范围：功能开发、API 修改、性能优化、依赖升级。
>
> 命名：所有 pack 用 `GFR-XXX`（God-File Refactor）前缀。
>
> 最后更新: 2026-04-21

---

## 1. Destination Map（11 个 bounded context）

任何拆出的代码必须落入下面 11 个 context 之一。**禁止** 新建 `utils/` `helpers/` `common/` `misc/` 目录。

| context              | 后端路径（**真实存在**）                                                                   | 前端路径                                                       |
| -------------------- | ------------------------------------------------------------------------------------------ | -------------------------------------------------------------- |
| conversation         | `src-tauri/src/modules/application/turn_service.rs` 系列                                   | `src/modules/chat/transcript/`                                 |
| prompt               | `src-tauri/src/modules/application/prompt_planner.rs` 系列                                 | —                                                              |
| provider             | `src-tauri/src/modules/application/provider_service.rs` + `application/real_api_client.rs` | —                                                              |
| tooling              | `src-tauri/src/modules/tools/` 已存在 + `application/tool_executor.rs`（待建）             | `src/modules/tool-projection/`                                 |
| memory               | `src-tauri/src/modules/application/memory_*.rs` + `modules/memory/`                        | `src/modules/memory/`                                          |
| permission           | `src-tauri/src/modules/application/permission_service.rs`（待建）                          | `src/modules/permission/`                                      |
| request_intelligence | `src-tauri/src/modules/application/request_intelligence_service.rs`                        | —                                                              |
| activation           | `src-tauri/src/modules/application/activation_service.rs` + `license_lifecycle_service.rs` | `src/boot/`                                                    |
| governance           | `src-tauri/src/modules/harness/`                                                           | `src/modules/governance/`（待建）                              |
| learning             | `src-tauri/src/modules/learning/`                                                          | `src/modules/strategy-diagnostics/`                            |
| shell_surface        | —                                                                                          | `src/boot/`、`src/shell/`、`src/modules/shell-router/`（待建） |

### 1.1 命名别名（与 GFR roadmap 对照）

GFR roadmap 中写的 `services/X` 在真实代码里是 `application/X`。**两者等价**。pack 里一律按 *真实路径* 写 destination：

| GFR roadmap              | 真实落点                                                                                     |
| ------------------------ | -------------------------------------------------------------------------------------------- |
| `services/provider/`     | `application/real_api_client.rs` + 已有 `application/provider_service.rs`                    |
| `services/prompt/`       | 已有 `application/prompt_planner.rs` 系列                                                    |
| `services/permission/`   | `application/permission_service.rs`（GFR-003 新建）                                          |
| `services/stream/`       | `application/stream_emitter_service.rs`（GFR-004 新建，与 `runtime/stream_emitter.rs` 配对） |
| `services/conversation/` | 已有 `application/turn_service.rs` 系列                                                      |
| `services/trajectory/`   | `application/trajectory_service.rs`（GFR-006 新建）                                          |

> 一次性把 `application/` 改名为 `services/` 不在重构范围（会使 LOC drift 失真且收益为 0）。

---

## 2. 不变量（Agent 必须遵守，否则 `verify` 失败）

| ID     | 不变量                                                                                                | 由谁验证                     |
| ------ | ----------------------------------------------------------------------------------------------------- | ---------------------------- |
| **I1** | 被搬代码的 `pub` 符号集合（fn / struct / enum / trait / const / type）**不增不减不改签名**            | `scripts/rfp verify`         |
| **I2** | 测试名称集合不减；测试 pass/fail 状态相对于 `before.json` 不退化                                      | `cargo test` + `verify` 比对 |
| **I3** | 事件名字符串字面量（`app.emit("...")` / `listen("...")`）集合不变                                     | `verify`（grep 比对）        |
| **I4** | 持久化 key 名（localStorage / sqlite column / config field）不变                                      | 人工 + grep 守门             |
| **I5** | 被拆 god-file 中保留 `pub use crate::modules::<dest>::*;` shim；调用方 import 路径**本 GFR 内不修改** | 人工 review diff             |
| **I6** | 被搬代码的函数体一行不改（除非是 `use` 路径修正）                                                     | 人工 review diff             |
| **I7** | `cargo fmt --check` + `cargo clippy -D warnings` 通过                                                 | `verify` 调用                |

> 任何不变量被打破，PR 必须分裂为「move PR」+「semantic change PR」两个独立 commit。

---

## 3. 文件大小目标

| 类型                     | 目标                                    | 上限   |
| ------------------------ | --------------------------------------- | ------ |
| 后端 module 文件         | ≤ 500 行                                | 800 行 |
| 前端组件 `.tsx`          | ≤ 300 行                                | 500 行 |
| 前端 hook 文件           | ≤ 200 行                                | 400 行 |
| God-file 拆完后剩余 shim | ≤ 200 行（仅 `pub use` + thin command） | 800 行 |

超过上限 = 必须再拆。

---

## 4. GFR 执行 5 步循环

```
① CONTRACT  — 人写 GFR-XXX.md（≤ 60 行）。stub 已生成，启动前需要补 Execute Plan + Verify Whitelist
② SNAPSHOT  — ./scripts/rfp snapshot GFR-XXX --phase before
③ EXECUTE   — agent 按 pack 搬代码 + 留 shim
④ VERIFY    — ./scripts/rfp verify GFR-XXX  （须 PASS）
⑤ COMMIT    — 1 PR + 更新 REGISTRY.md
```

**整个循环必须在一个 agent 会话内完成**。失败 3 次自动 STOP 并报告。

### 4.1 Stub → Active 升格

`packs/GFR-XXX-*.md` 在 roadmap 落地时全部以 *stub* 形式存在（只有 Goal / Source 候选 / Destination / Out-of-Scope）。

**前一个 GFR 状态变 done 后**，由人（或 agent 在人的指令下）做 3 件事把下一个 GFR 升格为 active：

1. 重读 source god-file，**重新校准 Source 行号**（前一个 GFR 已让行号漂移）。
2. 补写 `## Execute Plan`：列出搬哪几段、target 文件 use 头、shim 方式。
3. 补写 `## Verify Whitelist`：列出 `verify` 报告中允许出现的 added 项（pub 符号 / 事件 / 测试），其余一律视为违规。

未完成上述 3 步的 stub，agent **禁止执行**。

---

## 5. Hard Rules（Agent 启动必读）

- ❌ 禁止改任何函数体（仅允许调整 `use` 路径与 `pub(crate)` 可见性以编译通过）
- ❌ 禁止删测试、改测试名、改 assertion
- ❌ 禁止重命名 pub 符号
- ❌ 禁止顺手修复"看起来不优雅"的代码
- ❌ 禁止引入新依赖
- ❌ 禁止在同一 PR 内做两个 GFR
- ❌ 禁止执行处于 `pending` 状态的 stub
- ✅ 允许：移动代码、新建文件、加 `pub use` shim、调整 `use` 导入、`pub` ↔ `pub(crate)` 收紧
- ✅ 允许：在原 god-file 顶部加 `// Moved to <path> in GFR-XXX` 注释

---

## 6. 与现有 SOP 的边界

- GFR Loop **专门做 god-file 结构重构**。
- 业务功能演进继续走 `docs/implementation-packs/` + 老 exec-plan 体系。
- 两条轨道**不允许在同一 PR 内交叉**。
