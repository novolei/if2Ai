# cc-haha-main vs If2Ai 全盘架构评估与超越式整改报告

> 目标：把 `/Users/ryanliu/Documents/IfAI/cc-haha-main` 作为横向 benchmark，
> 对 `/Users/ryanliu/Documents/IfAI/if2Ai` 做一次可追溯、可迁移、可执行的 Staff 级架构审计。
>
> 审计日期：2026-04-21
> 审计者视角：Staff 系统架构师 + 桌面 AI Agent 产品/前端架构设计

---

## 1. 结论先行

**结论不是“cc-haha-main 全面更先进”，而是：它在 Desktop Host 分层、前端状态组织、进程边界与会话承载结构上明显优于当前 If2Ai；而 If2Ai 在 Rust 域建模、runtime contracts、memory / harness / control-plane ambition 上反而更强。**

因此最优路线不是“整仓照搬 cc-haha-main”，而是：

1. **复刻它的桌面宿主形态**
   - Tauri 只做 launcher / sidecar lifecycle / native capability bridge
   - 业务服务跑在独立 local server 或 canonical gateway
   - 前端只面对稳定的 API/WebSocket 协议，不直连数百个 Tauri command

2. **保留并继续升级 If2Ai 的 Rust 领域内核**
   - 保留 `application / control_plane / runtime / memory / harness` 这条方向
   - 不建议把 If2Ai 退回成 cc-haha 那种“CLI-first + desktop wrapper only”的窄壳

3. **超越 cc-haha-main 的正确方式**
   - 学它的壳层分层
   - 用 If2Ai 已有的 canonical contracts / runtime projection / memory governance / learning system 去做更强的内核
   - 最终形成“比 cc-haha 更薄的桌面壳 + 比 cc-haha 更强的 agent kernel”

---

## 2. 审计方法与证据来源

本报告只基于本地代码与文件结构，不基于印象判断。

### 2.1 审计范围

- Benchmark 仓库：`/Users/ryanliu/Documents/IfAI/cc-haha-main`
- 当前目标仓库：`/Users/ryanliu/Documents/IfAI/if2Ai`

### 2.2 对照维度

- 进程架构：Tauri 壳、sidecar、server、agent runtime 的边界
- 前端架构：App shell、router、store、api adapter、页面组织
- Agent 运行时：session orchestration、streaming、permission、transport
- 测试与可观测性：单测、服务测试、启动失败处理、健康检查
- 文档与演进纪律：代码注释、架构真相、与当前 Pack Charter 的一致性

### 2.3 本次采样到的关键量化事实

- `if2Ai` 前端主入口：
  - `src/App.tsx` = 2514 LOC
  - `src/lib/tauri.ts` = 2386 LOC
  - `src/components/ui/chat-ui.tsx` = 4736 LOC
- `if2Ai` 后端关键入口：
  - `src-tauri/src/commands/agent.rs` = 2718 LOC
  - `src-tauri/src/main.rs` = 1216 LOC
- `if2Ai` 当前前端测试文件数：**0**
- `if2Ai` 当前 Rust 测试文件数：**7**
- `if2Ai` 当前 `#[tauri::command]` 标注函数数：**210**

- `cc-haha-main` Desktop 壳层：
  - `desktop/src/App.tsx` = 5 LOC
  - `desktop/src/components/layout/AppShell.tsx` = 118 LOC
  - `desktop/src/api/client.ts` = 63 LOC
  - `desktop/src/lib/desktopRuntime.ts` = 54 LOC
- `cc-haha-main` Desktop store 文件数：**19**
- `cc-haha-main` Desktop API 模块数：**16**
- `cc-haha-main` desktop 测试文件数：**26**
- `cc-haha-main` server/core 测试文件数：**27**

这些数字本身不等于“质量”，但它们能直接说明职责是否被压平在入口层。

---

## 3. 关键发现

## 3.1 cc-haha-main 真正优于 If2Ai 的地方

### A. Tauri 被限制为 Desktop Host，而不是业务主承载层

`cc-haha-main` 的 Tauri 层几乎只负责三件事：

- 启动 server sidecar
- 管理 adapter sidecar 生命周期
- 把 server URL 暴露给前端

证据：

- `cc-haha-main/desktop/src-tauri/src/lib.rs:19-57`
- `cc-haha-main/desktop/src-tauri/src/lib.rs:126-170`
- `cc-haha-main/desktop/src-tauri/src/lib.rs:187-235`

代表性代码：

```rust
#[derive(Default)]
struct ServerState(Mutex<ServerStatus>);

#[tauri::command]
fn get_server_url(state: State<'_, ServerState>) -> Result<String, String> {
    let guard = state.0.lock().map_err(|_| "desktop server state is unavailable".to_string())?;
    if let Some(runtime) = guard.runtime.as_ref() {
        return Ok(runtime.url.clone());
    }
    Err(guard.startup_error.clone().unwrap_or_else(|| "desktop server did not start".to_string()))
}
```

```rust
fn start_server_sidecar(app: &AppHandle) -> Result<ServerRuntime, String> {
    let host = "127.0.0.1";
    let port = reserve_local_port()?;
    let url = format!("http://{host}:{port}");
    let sidecar = app.shell().sidecar("claude-sidecar")?
        .args(["server", "--app-root", &app_root_arg, "--host", host, "--port", &port.to_string()]);
    ...
    wait_for_server(host, port)?;
    Ok(ServerRuntime { url, child })
}
```

这意味着：

- Desktop native shell 不背业务复杂度
- 前端不被 Tauri command 面暴露出来的内部实现牵着走
- 以后换 server 实现或替换 agent runtime，不需要重写整个桌面前端

对比 If2Ai：

- `src-tauri/src/main.rs:9-220` 中直接挂接了海量 command
- 当前 `main.rs` 已经承担了过多业务暴露面，而不是只做 composition root

### B. 前端主壳是薄层，状态与数据访问被拆到 store / api / router

`cc-haha-main` 的桌面前端入口极薄：

- `desktop/src/App.tsx:1-5` 只有 `<AppShell />`
- `desktop/src/components/layout/AppShell.tsx:15-118` 只负责 bootstrap、ready/error shell、Sidebar、TabBar、ContentRouter
- `desktop/src/components/layout/ContentRouter.tsx:7-26` 只做 active tab 到页面的映射

代表性代码：

```tsx
export function App() {
  return <AppShell />
}
```

```tsx
export function ContentRouter() {
  const activeTabId = useTabStore((s) => s.activeTabId)
  const activeTabType = useTabStore((s) => s.tabs.find((t) => t.sessionId === s.activeTabId)?.type)
  if (!activeTabId || !activeTabType) return <EmptySession />
  if (activeTabType === 'settings') return <Settings />
  if (activeTabType === 'scheduled') return <ScheduledTasks />
  return <ActiveSession />
}
```

对比 If2Ai：

- `src/App.tsx:1-220` 已经能看到它同时承担：
  - boot orchestration
  - onboarding / splash routing
  - project/session state
  - left rail width state
  - chat state
  - permission state
  - voice bridge
  - runtime projection wiring
  - settings / model / todo / title / stream abort 管理
- `src/App.tsx` 总体 2514 LOC，明显已超出容器组件应有职责

这类结构的直接后果不是“难看”，而是：

- 启动行为和聊天行为高度耦合
- 很难做稳定的页面级测试
- 改一个 boot 逻辑，容易误伤 session/chat 逻辑
- 任何新模块最终都倾向继续堆进 `App.tsx`

### C. 前端数据访问被显式封装为 API client，不直连 native invoke 海洋

`cc-haha-main` 前端不是到处 `invoke()`，而是：

- `desktop/src/api/client.ts:1-63` 提供统一 request 抽象
- 再在 `desktop/src/api/*.ts` 中分领域封装：`sessions.ts`、`settings.ts`、`providers.ts`、`skills.ts`、`teams.ts` 等
- store 依赖的是 typed api module，而不是窗口层原语

代表性代码：

```ts
async function request<T>(method: string, path: string, body?: unknown, options?: { timeout?: number }): Promise<T> {
  const url = `${baseUrl}${path}`
  ...
  const res = await fetch(url, { method, headers, body, signal: controller.signal })
  ...
  return res.json() as Promise<T>
}
```

对比 If2Ai：

- `src/lib/tauri.ts:1-220` 虽然名义上是 thin bridge，但依然集中保留了大量 DTO + invoke/listen helper
- 该文件整体 2386 LOC，仍然是一个事实上的 mega transport facade
- `src/App.tsx:3-34` 直接 import 大量 Tauri IPC helper，说明前端容器层仍然知道太多后端命令细节

### D. 会话运行链路被组织成 “desktop shell -> local server -> conversation service -> CLI session”

`cc-haha-main` 中最值得借鉴的不是某个 UI，而是这个会话承载链：

- Desktop 启动本地 server
- 前端通过 HTTP/WebSocket 访问 server
- `ConversationService` 负责每个 session 的 CLI 子进程生命周期
- 权限、streaming、resume、partial messages 都在服务层消化

证据：

- `cc-haha-main/desktop/src/lib/desktopRuntime.ts:8-27`
- `cc-haha-main/src/server/services/conversationService.ts:1-212`
- `cc-haha-main/src/server/__tests__/conversation-service.test.ts:57-186`

代表性代码：

```ts
export async function initializeDesktopServerUrl() {
  const { invoke } = await import('@tauri-apps/api/core')
  const serverUrl = await invoke<string>('get_server_url')
  setBaseUrl(serverUrl)
  await waitForHealth(serverUrl)
  return serverUrl
}
```

```ts
export class ConversationService {
  private sessions = new Map<string, SessionProcess>()

  async startSession(sessionId: string, workDir: string, sdkUrl: string, options?: SessionStartOptions): Promise<void> {
    ...
    proc = Bun.spawn(args, {
      cwd: workDir,
      env: childEnv,
      stdin: 'pipe',
      stdout: 'ignore',
      stderr: 'pipe',
    })
    ...
  }
}
```

这条链路的好处是：

- session 生命周期可独立测试
- CLI/agent runtime 的不稳定性不会直接污染桌面壳层
- 对 partial stream / reconnect / permission replay 的治理点更集中

### E. Desktop 测试与 server/service 测试明显更成体系

`cc-haha-main` 已有：

- desktop UI/store test: 26 个
- server/core test: 27 个

而 If2Ai 当前：

- 前端测试文件：0
- Rust 测试文件：7

这意味着当前 If2Ai 即使架构方向更“雄心大”，在桌面壳与用户态交互上仍缺少回归护栏。

---

## 3.2 If2Ai 已经比 cc-haha-main 更先进、不能丢的地方

### A. If2Ai 已经在构建 canonical runtime projection

这是 If2Ai 一个非常正确、而且比 cc-haha-main 更有潜力的方向。

证据：

- `src/runtime-projection/runtime-projection-store.ts:1-133`
- `src/runtime-projection/runtime-projection-bridge.ts:1-252`

代表性代码：

```ts
export function createRuntimeProjectionStore(...) {
  let snapshot: RuntimeProjectionSnapshot = emptyProjectionSnapshot()
  const queue = createRuntimeEventQueue({ flushMode: options.flushMode })
  queue.subscribe((batch) => {
    const next = reduceRuntimeEventBatch(snapshot, batch)
    if (next === snapshot) return
    snapshot = next
    notifyAll()
  })
}
```

```ts
export function wireRuntimeProjectionListeners(...) {
  track(listenToAgentTokenStream(...), 'agent-token')
  track(listenToPermissionRequests(...), 'permission-request')
  track(listenMemoryEvent(...), 'memory_event')
  ...
}
```

这套东西一旦真正成为 UI 真相层，会比 cc-haha-main 当前的 per-store 手工整合更强。

### B. If2Ai 正在把 agent orchestration 从 god-file 中抽成 application seam

证据：

- `src-tauri/src/modules/application/turn_service.rs:1-227`

代表性代码：

```rust
pub async fn prepare_chat_inputs(
    &self,
    request: PrepareChatInputsRequest,
) -> Result<PreparedChatInputs, TurnServiceError> {
    let provider = resolve_chat_runtime_provider(&request.workdir).await?;
    let intelligence = classify(...);
    let coordinator = MemoryCoordinator::with_default_policy(...);
    let prepared_context = coordinator.prepare_context(...).await;
    let prompt = build_prompt_plan(...).await?;
    Ok(PreparedChatInputs { ... })
}
```

cc-haha-main 更像是“桌面壳与 server 层做得顺”；If2Ai 则已经开始向“真正的 domain/application 层分离”迈进。

### C. If2Ai 的 memory / harness / learning ambition 明显更强

从 `src-tauri/src/modules/mod.rs:15-37` 可以看到 If2Ai 当前包含：

- `application`
- `control_plane`
- `harness`
- `learning`
- `memory`
- `runtime`
- `skills`
- `browser`
- `tts` / `stt`

这说明 If2Ai 不是单纯 chat desktop，而是在做 Agent OS / Agent Runtime。

因此战略上不能为了学 cc-haha-main，而把自己退回成“纯桌面包壳 + CLI 代理”。

---

## 4. If2Ai 当前最需要修的不足

## 4.1 Desktop Host 与 Agent Kernel 没有真正隔离

当前 If2Ai 里：

- Tauri main 暴露了极大的业务 command 面
- 前端大量直接依赖 Tauri IPC helper
- App 容器知道太多 session / project / onboarding / streaming / permission / settings 细节

这导致三个问题：

1. **native 壳层无法保持稳定**
2. **前端 transport 变更会牵动业务真相层**
3. **agent runtime 很难独立演进为可测试的 canonical backend**

证据：

- `src-tauri/src/main.rs:9-220`
- `src/lib/tauri.ts:1-220`
- `src/App.tsx:1-220`

## 4.2 `commands/agent.rs` 仍然过大，TurnService 还不是真正的 canonical orchestrator

虽然 If2Ai 已经有 `TurnService`，但 `commands/agent.rs` 仍然 2718 行，而且继续承担：

- session restore
- harness hooks
- permission bridge
- stream lifecycle
- compaction
- tool loop wiring
- runtime session conversion
- turn-end bookkeeping

证据：

- `src-tauri/src/commands/agent.rs:1-120`
- `src-tauri/src/commands/agent.rs:2615-2718`

这说明当前状态仍然是：

- `TurnService` 是 seam
- 但不是 single orchestrator truth

而用户要的“Agent 运行更完整、更稳定”，真正关键是让**一条 canonical turn spine**成立。

## 4.3 `modules/mod.rs` 的聚合式 glob re-export 会继续放大边界模糊

证据：

- `src-tauri/src/modules/mod.rs:1-67`

问题点：

- `#![allow(ambiguous_glob_reexports)]`
- 广泛 `pub use xxx::*`

这类聚合层在项目早期方便，但在中后期会导致：

- 依赖关系不清晰
- import 来源模糊
- 编译错误与 API 面扩大
- 难以明确模块边界

对一个目标是“bounded context + canonical workflow truth”的项目，这不是长期可接受形态。

## 4.4 前端容器与 transport 仍然是 god-file 级别

最典型的三个文件：

- `src/App.tsx`
- `src/lib/tauri.ts`
- `src/components/ui/chat-ui.tsx`

当前问题不是仅仅“文件大”，而是**大且承担真相**：

- `App.tsx` 承担 boot + session + project + UI orchestration
- `tauri.ts` 承担 transport contract 兼 DTO 仓库
- `chat-ui.tsx` 承担 transcript/renderer/composer/structure logic

这会拖慢所有后续 iteration，包括你想做的“复刻 cc-haha 并继续超越”。

## 4.5 测试面严重失衡，前端几乎没有回归护栏

当前 If2Ai 的一个硬风险不是 Rust 领域层，而是“用户直接可见层的行为稳定性”。

现状：

- 前端测试文件数为 0
- 没有形成 `store -> page -> shell -> transport mock` 这条测试链

与 cc-haha-main 对比，差距非常直观：

- `desktop/src/stores/*.test.ts`
- `desktop/src/components/**/*.test.tsx`
- `src/server/__tests__/*.test.ts`

这也是为什么后者更容易在桌面产品层“显得稳定”。

## 4.6 文档与代码注释存在“旧流程真相残留”

当前 Pack Charter 已明确：

- 禁止继续以 `docs/exec-plans/*` 作为 agent 执行真相
- `docs/_legacy` 禁读

但代码与文档里仍然残留大量旧引用。

证据：

- `src-tauri/src/modules/application/turn_service.rs:21-24`
- `src-tauri/src/modules/application/mod.rs`
- `src-tauri/src/commands/activation.rs`
- `docs/staff-remediation/README.md`

这不是表面洁癖问题，而是会造成：

- 新旧流程双真相
- agent 注释与实际工作流冲突
- 后续协作时误导 executor

---

## 5. 不建议照搬 cc-haha-main 的部分

为了避免“benchmark 崇拜”，必须明确这几条。

### A. 不建议照搬它的 CLI/core monolith

`cc-haha-main` 并不是整体上所有层都更优。

它自己的 core 里也有大文件：

- `src/main.tsx` = 4704 LOC
- `src/cli/print.ts` = 5594 LOC
- `src/utils/sessionStorage.ts` = 5105 LOC
- `src/utils/hooks.ts` = 5040 LOC

所以不应该得出“只要变成 Bun + TS server 就自然更优”的结论。

### B. 不建议把 If2Ai 的 Rust kernel 降级成单纯 CLI wrapper

If2Ai 现在最大的长期价值，在于它已经开始具备：

- Rust runtime / memory / learning / harness 治理潜力
- 更强的本地 agent kernel 可塑性

这部分是应该增强，不是回退。

### C. 不建议把前端状态全做成“普通 store 拼装”而失去 canonical projection

cc-haha-main 的 Zustand 分层值得学习，但 If2Ai 已经有更强的 runtime projection 路线。

正确做法应该是：

- UI 层学它的 store/page/shell 分拆
- 真相层保留 If2Ai 的 canonical event projection

---

## 6. 建议的目标架构：在 If2Ai 内“超越式复刻” cc-haha-main

## 6.1 目标形态

建议最终收敛为四层：

### 第 1 层：Desktop Host

职责：

- 启动/停止 local gateway
- 管理 sidecars
- 承接系统权限、窗口、tray、native menu
- 暴露极少量 native-only command

技术形态：

- `src-tauri/src/desktop_host/*`

### 第 2 层：Canonical Local Gateway

职责：

- 提供 HTTP/WebSocket 或稳定的 internal RPC surface
- 聚合 session / settings / providers / skills / tasks / browser / harness
- 让前端与 runtime 解耦

建议：

- Rust 实现优先，不建议新引入 TS server 作为长期核心
- 可以是：
  - Tauri 内嵌 Axum/Hyper 本地 server
  - 或 Tauri 内部 gateway service + typed bridge，但前端调用面必须被“server-like API”统一起来

### 第 3 层：Agent Kernel

职责：

- canonical turn spine
- provider resolution
- tool execution
- permission policy
- memory injection / recall / after-turn writeback
- stream emission
- harness trace/report

对应现有 If2Ai 优势区：

- `application`
- `control_plane`
- `runtime`
- `memory`
- `learning`
- `harness`

### 第 4 层：Frontend Surface

职责：

- AppShell / Router / Pages / Stores / Feature Components
- 只读 projection、只写 API/store action
- 不知道 Tauri command 细节

---

## 7. 迁移蓝图：从当前 If2Ai 到目标形态

## 7.1 P0：先修“壳层边界”

优先级：最高

目标：

- 把“前端直连 Tauri command 海洋”改造成“前端只依赖 domain API facade”
- 为后续替换 transport 做准备

建议动作：

1. 新建 `src/api/client.ts`
   - 统一封装所有 frontend request/listen 行为
   - 当前底层仍可调用 Tauri invoke/listen
   - 但上层不再 import `@/lib/tauri`

2. 新建 `src/api/{sessions,projects,settings,providers,skills,agent}.ts`
   - 按领域拆出 typed module

3. 收缩 `src/lib/tauri.ts`
   - 只保留最底层 native bridge
   - DTO 全迁移到 `src/transport/*`

4. 把 `App.tsx` 变成 `AppShell.tsx + bootstrap store + router`

### P0 借鉴模板

- `cc-haha-main/desktop/src/api/client.ts`
- `cc-haha-main/desktop/src/components/layout/AppShell.tsx`
- `cc-haha-main/desktop/src/components/layout/ContentRouter.tsx`

## 7.2 P1：建立 If2Ai 的 Local Gateway

优先级：最高

目标：

- 让 Tauri main 不再成为海量业务 command 注册地
- 为桌面壳稳定性、服务测试、WebSocket 流式传输统一入口

建议动作：

1. 新建 `src-tauri/src/gateway/`
   - `server.rs`
   - `routes/`
   - `ws/`
   - `dto/`

2. 将高频业务面抽成 gateway API：
   - sessions
   - conversations
   - settings
   - providers
   - skills
   - tasks
   - browser

3. `src-tauri/src/main.rs` 降级为：
   - start gateway
   - wire native plugins
   - expose `get_gateway_url` / health / restart native-only hooks

### P1 借鉴模板

- `cc-haha-main/desktop/src-tauri/src/lib.rs`
- `cc-haha-main/desktop/src/lib/desktopRuntime.ts`
- `cc-haha-main/src/server/router.ts`
- `cc-haha-main/src/server/api/*.ts`

## 7.3 P2：把 `commands/agent.rs` 真正收口成 canonical turn orchestrator

优先级：最高

目标：

- 不再让 `commands/agent.rs` 继续同时做 IPC adapter 和 turn orchestration
- 建立真正的一条 canonical chat execution spine

建议动作：

1. 新建 `src-tauri/src/modules/application/chat_orchestrator.rs`
2. 让 orchestration 覆盖：
   - session execution context resolve
   - prepare chat inputs
   - runtime turn execution
   - permission loop
   - stream emission
   - after-turn dispatch
   - harness report envelope

3. `commands/agent.rs` 只保留：
   - request decode
   - state lookup
   - orchestrator call
   - response encode

### P2 借鉴方式

这里**不要**照抄 cc-haha 的 `ConversationService` 实现细节；
应该借它“会话生命周期集中管理”的思想，把 If2Ai 已有 `TurnService` 升级成更完整的 Rust orchestrator。

## 7.4 P3：前端 UI 架构做一次真正的壳层拆分

优先级：高

目标：

- `App.tsx` 不再是应用真相中心
- `chat-ui.tsx` 不再继续增长

建议动作：

1. 引入 store 分层
   - `bootStore`
   - `projectStore`
   - `sessionStore`
   - `chatStore`
   - `settingsStore`
   - `uiStore`

2. 页面/壳层结构改为：
   - `src/app/AppShell.tsx`
   - `src/app/ContentRouter.tsx`
   - `src/pages/ChatPage.tsx`
   - `src/pages/SettingsPage.tsx`
   - `src/pages/SkillsPage.tsx`
   - `src/pages/AutomationPage.tsx`

3. `chat-ui.tsx` 拆为：
   - transcript renderer
   - composer
   - blocks renderer
   - tool/result cards
   - permissions surfaces
   - telemetry drawer

### P3 借鉴模板

- `cc-haha-main/desktop/src/components/layout/AppShell.tsx`
- `cc-haha-main/desktop/src/stores/chatStore.ts`
- `cc-haha-main/desktop/src/stores/sessionStore.ts`

## 7.5 P4：补齐测试基线，否则“复刻”会引入更大风险

优先级：最高

必须至少补齐：

- frontend shell test
- router test
- session store test
- chat store test
- settings store test
- gateway API integration test
- orchestrator regression test
- streaming / permission / resume test

可直接借鉴的组织方式：

- `cc-haha-main/desktop/src/stores/*.test.ts`
- `cc-haha-main/desktop/src/components/**/*.test.tsx`
- `cc-haha-main/src/server/__tests__/conversation-service.test.ts`

---

## 8. 可直接开做的 Pack / 模块化任务建议

以下不是泛泛 roadmap，而是建议你下一轮直接落 Pack 的顺序。

## 8.1 建议执行顺序与优先级

### `P0` 先做，建立可运行主链

1. `MIG-001`：Canonical Chat Execution Spine
2. `MIG-010`：Local Gateway Bootstrap
3. `MIG-012`：Frontend API Facade And Transport Cutover
4. `MIG-013`：App Shell Router And Bootstrap Store
5. `MIG-014`：Session And Chat Store Foundation
6. `MIG-015`：Gateway Conversations And Streaming Surface

为什么先做这一组：

- 这 6 个 Pack 共同决定“主聊天产品链”是不是稳定可持续的
- 不先把主链与边界做出来，后面的 execution mode、projection、harness 都只能继续挂在旁路
- 这是复刻 `cc-haha-main` 优势区最直接的一组

### `P1` 第二波，补语义与内核闭环

7. `MIG-002`：Execution Mode Routing And Policy Enforcement
8. `MIG-004`：Prompt Planning Traceability
9. `MIG-005`：Real Memory Lifecycle
10. `MIG-011`：Desktop Host Thin Shell

为什么排第二波：

- 这些工作能显著提高 agent runtime 的一致性、可追踪性和产品体感
- 但它们更依赖主聊天链已经不再是 command-heavy 临时结构
- `MIG-011` 虽然是 host 层工作，但在 gateway/bootstrap 成立后做更稳，不容易变成一次大搬家

### `P2` 第三波，统一前端真相与工具执行契约

11. `MIG-003`：Runtime Event Projection Truth
12. `MIG-006`：Frontend Shell Truth
13. `MIG-007`：Worker Tool Execution Contract

为什么排第三波：

- 这组工作的收益很大，但前提是 conversations surface、store 基础层和 shell 容器已经稳定
- 否则 projection/shell contract 很容易再次落回到双真相与过度耦合

### `P3` 最后做，收治理和商业闭环

14. `MIG-008`：Harness Replay And Eval On Canonical Run Report
15. `MIG-009`：Activation License Lifecycle

为什么最后做：

- harness 与 activation 都重要，但它们不应该领先于产品主链和运行时主链
- 如果主链不稳，这两块越先做，越容易放大“治理很强、产品闭环很弱”的失衡

---

## 9. 最终判断：应该怎么“完整复刻并超越”

如果目标是“完整复刻 `/Users/ryanliu/Documents/IfAI/cc-haha-main` 中的相关内容，并进一步优化并超越它”，我给出的 Staff 结论是：

### 应该完整复刻的

- Desktop Host 极薄化
- local server / gateway 作为前后端稳定边界
- api module + store + router 的前端组织方式
- session lifecycle 的集中治理思路
- healthcheck / startup error / ready shell
- 以测试护栏支撑桌面行为稳定性

### 应该选择性复刻的

- Conversation/session manager 的生命周期设计
- WebSocket streaming 与 permission resolution 的集中处理方式
- settings / tabs / sessions / chat 的 store 化组织

### 不该复刻的

- 它自己 core/CLI 里的大文件和 monolith
- 将 If2Ai Rust kernel 退化为“桌面 wrapper”
- 放弃 If2Ai 已有的 runtime contracts / projection / memory / harness / learning 优势

### If2Ai 超越它的正确目标

- `cc-haha-main` 级别的 Desktop product stability
- 再叠加 If2Ai 自己的：
  - canonical event projection
  - memory governance
  - harness-grade evaluation
  - learning / strategy evolution
  - richer local runtime

一句话总结：

**要学的是 cc-haha-main 的“桌面宿主架构”，不是它整个技术体；If2Ai 应该把自己重构成“cc-haha 风格的稳定壳层 + 更强的 Rust agent kernel”。**

---

## 10. 本报告引用的关键文件

### If2Ai

- `src/App.tsx`
- `src/lib/tauri.ts`
- `src/runtime-projection/runtime-projection-store.ts`
- `src/runtime-projection/runtime-projection-bridge.ts`
- `src-tauri/src/main.rs`
- `src-tauri/src/commands/agent.rs`
- `src-tauri/src/modules/application/turn_service.rs`
- `src-tauri/src/modules/mod.rs`

### cc-haha-main

- `desktop/src/App.tsx`
- `desktop/src/components/layout/AppShell.tsx`
- `desktop/src/components/layout/ContentRouter.tsx`
- `desktop/src/api/client.ts`
- `desktop/src/lib/desktopRuntime.ts`
- `desktop/src/stores/sessionStore.ts`
- `desktop/src/stores/chatStore.ts`
- `desktop/src-tauri/src/lib.rs`
- `src/server/services/conversationService.ts`
- `src/server/__tests__/conversation-service.test.ts`

---

## 11. 建议的下一步

建议下一轮直接进入两条并行线：

1. 写正式 migration-core Pack
   - 先把 `Desktop Host / Local Gateway / Frontend API Facade / AppShell` 四件事落成 Pack 序列

2. 先做一个最小可见的垂直切片
   - `get_server_url -> frontend api client -> settings/session list -> AppShell/Router/store`
   - 以最小 slice 验证“新壳层”成立

如果这条垂直切片跑通，后面再把 chat streaming、permission、memory after-turn、harness 抽过去，风险会小很多。
