# Memory Control Plane V1（Staff 架构审阅）

最后更新：2026-04-13

## 目标与结论

本文基于三方代码库深度审阅：

- 当前 If2Ai（`src-tauri` + `src`）
- UClaw（`uclaw-rs` + `UClawApp` + `uclaw-tauri`）
- Hermes Agent（`hermes-agent-main`）

目标不是复制某一实现，而是形成适配桌面 Tauri 场景的最优策略。  
结论：采用 **Memory Control Plane** 分层路线，优先补齐隔离和持久化，再增强召回质量与可解释闭环，最后引入可选混合检索。

---

## 一、三方实现完整性对比（架构维度）

| 维度       | If2Ai 当前                                    | UClaw                              | Hermes                                 |
| ---------- | --------------------------------------------- | ---------------------------------- | -------------------------------------- |
| 记忆定位   | 工具型 KV（store/recall/forget/purge/export） | 运行时捕获 + 策略治理 + 可操作闭环 | curated 文件记忆 + provider 插件体系   |
| 持久化     | 进程内 HashMap（重启丢失）                    | SQLite + Lance + 部分 JSON mirror  | MEMORY.md/USER.md + 外部 provider      |
| 召回能力   | 子串匹配，无稳定排序                          | FTS + 向量 + 图扩展 + 融合排序     | provider 决定；manager 侧偏文本拼接    |
| 作用域隔离 | 未强制 project/session 分区                   | scope 完整（session/project/user） | 支持 user/session 参数，接线依赖实现   |
| 可解释性   | 基本无 memory 证据链                          | why/rank/evidence 较完整           | 部分 provider 返回 score，系统级不统一 |
| 治理与审计 | 工具审计已起步，memory 专项不足               | policy audit + 事件模型            | 熔断和降级好，结构化审计较弱           |
| 前端闭环   | 记忆 UI 交互弱                                | Swift 侧闭环成熟                   | 主要面向运行时/网关，UI 闭环非核心     |

### 关键判断

1. If2Ai 的基础安全控制平面已成型，适合承接记忆控制平面。
2. UClaw 的产品闭环值得借鉴，但多真相源复杂度不宜早期引入。
3. Hermes 的抽象与降级策略值得借鉴，但存在文档/实现漂移风险，不能盲搬。

---

## 二、当前 If2Ai 的核心差距（按优先级）

### P0（必须先做）

1. 记忆没有 durable 存储，重启后丢失。
2. 记忆没有强制 scope（project/workdir/session）隔离。
3. recall 无稳定排序和可解释依据，结果不可审计。

### P1（紧随其后）

1. 缺少 capture -> review -> promote/forget 生命周期。
2. 缺少 memory 专项审计事件与前端证据透出。
3. 缺少策略治理（写入阈值、冲突键、降噪、拒绝原因）。

### P2（优化项）

1. 可选向量检索和混合召回。
2. policy dashboard 与运营化指标。
3. 高级去重、衰减、跨会话记忆迁移。

---

## 三、目标架构：Memory Control Plane

```text
UI (chat/tool cards/memory panel)
  -> Command Facade (agent.rs / tools.rs)
    -> Memory Control Plane
       - MemoryScopeResolver
       - MemoryPolicyEngine
       - MemoryRecallEngine
       - MemoryAuditEmitter
       - MemoryEventBridge
    -> Memory Store (SQLite as SSOT)
    -> Optional Index (Hybrid search behind feature flag)
```

### 3.1 MemoryScopeResolver

- 输入：`session_id`, `project_id`, `workdir`
- 输出：`MemoryExecutionScope`（不可变）
- 作用：所有 memory 读写必须绑定 scope，拒绝全局共享命名空间

### 3.2 MemoryPolicyEngine

- 写入策略：置信度阈值、冲突键、去重规则
- 读取策略：只读模式可见范围、敏感类别过滤
- 决策产物：`allow|deny|prompt` + `reason_code`

### 3.3 MemoryRecallEngine

- V1：`lexical + recency + importance`（稳定排序）
- V2：`hybrid`（关键词 + 可选向量）通过 feature flag 灰度
- 输出结构：`rank_score`, `reason_codes`, `evidence`

### 3.4 MemoryAuditEmitter

新增事件：

- `memory_captured`
- `memory_write_decision`
- `memory_persisted`
- `memory_recall_served`
- `memory_rejected`
- `memory_promoted`

统一字段：
`trace_id`, `session_id`, `project_id`, `effective_workdir`, `scope`, `policy_decision`, `reason_code`, `evidence_id`, `duration_ms`

### 3.5 MemoryEventBridge

把记忆事件桥接到前端消息流（tool card / memory chip）：

- 展示写入是否成功
- 展示为什么写入/拒绝
- 展示召回为什么命中

---

## 四、最优策略（Staff 推荐）

### 原则

1. **SSOT 优先**：先建立单一真相源（SQLite），避免早期双写。
2. **隔离优先于智能**：先解决 scope 与边界，再做向量化。
3. **可解释优先于复杂召回**：先有 reason/evidence，再谈混合算法。
4. **灰度可回滚**：所有新能力都要 feature flag + shadow 模式。

### 实施节奏

1. M1（基础）：持久化 + scope 隔离 + memory 审计事件
2. M2（增强）：稳定召回排序 + 可解释字段 + UI 证据闭环
3. M3（扩展）：可选 hybrid 检索 + policy dashboard

---

## 五、Hermes/UClaw 借鉴与禁搬 Checklist

> 用法：每个 slice 开发前过一遍，PR review 时逐项勾选。

### 5.1 Hermes 可借鉴项 Checklist

- [ ] 采用 provider 接口抽象，保持后端可替换（避免硬耦合单一实现）
- [ ] 保留 manager/broker 编排层，对 provider 失败做隔离
- [ ] prefetch 结果只注入请求副本，不污染会话持久消息
- [ ] 引入熔断/冷却机制，避免 memory 后端雪崩影响主对话
- [ ] 保留 flush-before-end 思路，减少陈旧覆盖风险
- [ ] 明确 memory 与 transcript 的职责边界（长期事实 vs 临时上下文）
- [ ] 写入路径维持幂等语义（去重键/冲突键）

### 5.2 Hermes 禁搬项 Checklist

- [ ] 不直接引入平台相关锁实现（如仅 Unix 可用方案）
- [ ] 不接受文档、测试、实现三套语义分叉
- [ ] 不在主路径吞掉所有 memory 错误而无审计
- [ ] 不让 `session_id` 在编排层缺失导致潜在串桶
- [ ] 不做无结构 prefetch 文本拼接作为长期方案
- [ ] 不在多后端并存时缺少单一真相源定义
- [ ] 不依赖线程拼接一致性，优先受控 async 队列

### 5.3 UClaw 可借鉴项 Checklist

- [ ] 引入 `memory_captured` 事件并带 evidence/why 字段
- [ ] 建立 session/project/user scope 策略并可配置
- [ ] 引入 `reason_codes` / `rank_score` 的召回解释
- [ ] 引入 write policy（阈值、冲突处理、降噪）
- [ ] 建立 memory policy audit（可查询、可追踪）
- [ ] 提供用户可操作闭环（promote / pin / delete）
- [ ] 让开发者工具可观测 why-rank 与命中证据

### 5.4 UClaw 禁搬项 Checklist

- [ ] 不在早期同时维护 SQLite + JSON + 向量多真相源
- [ ] 不让多客户端契约漂移（类型/路由/字段不一致）
- [ ] 不先做重型混合检索再补隔离与持久化
- [ ] 不引入“占位图谱”但对外宣称真实关系图
- [ ] 不把策略散落多入口而缺统一控制平面
- [ ] 不让 session 记忆无策略地写入长期索引
- [ ] 不在缺回滚开关时上线高风险 recall 逻辑

---

## 六、架构策略更新（对现有 6A 的延续）

在 `phase-6a-control-plane-hardening` 基础上新增 `phase-6b-memory-control-plane`：

1. 在命令层强制 memory 走 `MemoryScopeResolver`。
2. 在 runtime/config 增加 memory feature flags：
   - `memoryControlPlaneV1Enabled`
   - `memoryRecallMode` (`lexical|hybrid`)
   - `memoryPolicyEnforceMode` (`shadow|enforce`)
3. 在 chat stream 增加 memory 事件载荷：
   - `policy_decision`
   - `reason_codes`
   - `evidence_id`
   - `effective_scope`

---

## 七、验收指标（建议）

- P0：
  - 跨项目/会话记忆泄漏事件数 = 0
  - 重启后记忆恢复率 = 100%
  - 无证据写入宣称率 = 0
- P1：
  - 召回解释覆盖率 > 95%
  - 策略拒绝可解释率 = 100%
  - 记忆相关问题 MTTR < 10 分钟

---

## 八、最终建议

最优路径不是“复制 UClaw 或 Hermes”，而是：

- 用 If2Ai 已有控制平面承载 memory 治理；
- 借 Hermes 的抽象和降级；
- 借 UClaw 的产品闭环和可解释性；
- 严格执行“单一真相源 + 分阶段灰度”。

该路径在工程复杂度、上线风险、用户体验三者之间最均衡。
