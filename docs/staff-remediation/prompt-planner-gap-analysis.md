# Prompt Planner Gap Analysis: UClaw vs If2Ai

> 横向对照 UClaw 与 If2Ai 的 prompt planner 实现，识别架构差距与对齐机会。
>
> 生成日期: 2026-04-22
> 参考蓝图: [if2ai-staff-remediation-blueprint.md](./if2ai-staff-remediation-blueprint.md) §8.1.C

---

## 1. 执行摘要

**当前状态**：If2Ai 已在 MIG-004 中完成 prompt planner 的基础 traceability 升级（trace_id、block_hash、diagnostics），但与 UClaw 的成熟实现相比仍存在显著架构差距。

**核心发现**：
- If2Ai 的 prompt planner 是单文件 (~506 LOC) 的功能性实现
- UClaw 的 prompt planner 是模块化 (10+ 子模块) 的领域驱动设计
- 缺失关键契约：PromptBlockSource、priority、is_sensitive、PromptContribution、validation_issues
- 缺失关键机制：external contributions、strict validation mode、multiple build modes
- 缺失关键能力：coding continuation、compaction policy、persona/skill injection

**对齐优先级**：
- **P0 (立即对齐)**：PromptBlockSource、priority、is_sensitive、validation_issues
- **P1 (短期对齐)**：PromptContribution 机制、PromptBuildMode、strict validation
- **P2 (长期对齐)**：模块化重构、coding continuation、compaction policy

---

## 2. 架构对比

### 2.1 文件组织结构

#### UClaw (模块化)
```
uclaw-rs/src/prompt/
├── mod.rs                      # 模块导出
├── block.rs                    # PromptBlock + PromptBlockSource + PromptContribution
├── diagnostics.rs              # PromptPlanDiagnostics + PromptValidationIssue
├── build_request.rs            # PromptBuildRequest + PromptBuildMode + PromptBuildOptions
├── planner.rs                  # DefaultPromptPlanner + PromptPlanner trait
├── coding_prompt_augment.rs    # Coding mode 专用增强
├── project_context.rs          # ProjectContextSnapshot
└── ... (其他子模块)
```

**设计原则**：
- 每个文件承载单一职责
- 清晰的领域边界
- 易于测试和扩展

#### If2Ai (单文件)
```
if2Ai/src-tauri/src/modules/application/prompt_planner/
├── mod.rs                      # 所有核心逻辑 (~506 LOC)
├── sanitize.rs                 # 消息清理 (GFR-002a)
├── preflight.rs                # 预检估算 (GFR-002c)
└── governor.rs                 # 上下文治理 (GFR-002b)
```

**当前问题**：
- mod.rs 承载过多职责（block 定义 + plan 定义 + diagnostics + planner 逻辑）
- 缺少清晰的领域边界
- 扩展新能力需要修改核心文件

**Gap**: UClaw 的模块化结构更易维护和扩展。

---

## 3. 数据结构对比

### 3.1 PromptBlock

#### UClaw
```rust
pub struct PromptBlock {
    pub id: String,
    pub kind: PromptBlockKind,
    pub title: String,
    pub body: String,
    pub source: PromptBlockSource,      // ✅ 来源追踪
    pub priority: i32,                   // ✅ 排序优先级
    pub is_sensitive: bool,              // ✅ 敏感标记
}

pub struct PromptBlockSource {
    pub subsystem: String,               // 子系统名称
    pub reference: Option<String>,       // 可选引用路径
}
```

#### If2Ai
```rust
pub struct PromptBlock {
    pub id: String,
    pub kind: PromptBlockKind,
    pub title: &'static str,             // ❌ 静态字符串限制扩展性
    pub content: String,
    // ❌ 缺少 source
    // ❌ 缺少 priority
    // ❌ 缺少 is_sensitive
}
```

**Gap 分析**：
1. **PromptBlockSource 缺失** → 无法追踪 block 来源，harness 无法归因
2. **priority 缺失** → 无法实现基于优先级的 block 重排序（未来 token budget 优化需要）
3. **is_sensitive 缺失** → 依赖 kind 隐式判断敏感性，不够灵活
4. **title 类型限制** → `&'static str` 阻止动态 title（如 "memory_pinned-0"）

**对齐建议 (P0)**：
```rust
pub struct PromptBlock {
    pub id: String,
    pub kind: PromptBlockKind,
    pub title: String,                   // 改为 String
    pub content: String,
    pub source: PromptBlockSource,       // 新增
    pub priority: i32,                   // 新增
    pub is_sensitive: bool,              // 新增
}

pub struct PromptBlockSource {
    pub subsystem: String,
    pub reference: Option<String>,
}
```

---

### 3.2 PromptPlanDiagnostics

#### UClaw
```rust
pub struct PromptPlanDiagnostics {
    pub trace_id: String,
    pub block_kinds: Vec<PromptBlockKind>,
    pub block_count: usize,
    pub redacted_preview: Vec<String>,
    pub validation_issues: Vec<PromptValidationIssue>,  // ✅ 验证问题列表
}

pub struct PromptValidationIssue {
    pub code: String,
    pub message: String,
}
```

#### If2Ai
```rust
pub struct PromptPlanDiagnostics {
    pub trace_id: String,
    pub block_kinds: Vec<PromptBlockKind>,
    pub block_count: usize,
    pub redacted_preview: Vec<String>,
    // ❌ 缺少 validation_issues
}
```

**Gap 分析**：
- **validation_issues 缺失** → 无法记录 prompt 构建过程中的警告/错误
- 当前 If2Ai 的 diagnostics 只是"快照"，不是"诊断报告"

**对齐建议 (P0)**：
```rust
pub struct PromptPlanDiagnostics {
    pub trace_id: String,
    pub block_kinds: Vec<PromptBlockKind>,
    pub block_count: usize,
    pub redacted_preview: Vec<String>,
    pub validation_issues: Vec<PromptValidationIssue>,  // 新增
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PromptValidationIssue {
    pub code: String,
    pub message: String,
}
```

---

### 3.3 PromptBuildRequest

#### UClaw
```rust
pub struct PromptBuildRequest {
    pub conversation_id: String,
    pub run_id: Option<String>,
    pub user_input: String,
    pub mode: PromptBuildMode,           // ✅ 多模式支持
    pub persona_id: Option<String>,      // ✅ Persona 注入
    pub active_skill_ids: Vec<String>,   // ✅ Skills 注入
    pub tool_names: Vec<String>,
    pub context: ProjectContextSnapshot,
    pub options: PromptBuildOptions,     // ✅ 构建选项
}

pub enum PromptBuildMode {
    Chat,
    Coding,
    Automation,
    Compose,
}

pub struct PromptBuildOptions {
    pub include_diagnostics: bool,
    pub strict_block_validation: bool,   // ✅ 严格验证模式
    pub coding_compaction_estimated_tokens: Option<usize>,
    pub coding_compaction_message_count: Option<usize>,
    pub coding_session_snapshot: Option<String>,
}
```

#### If2Ai
```rust
pub struct BuildPromptPlanRequest {
    pub session_id: String,
    pub user_message: String,
    pub workdir: PathBuf,
    pub current_date: String,
    pub os_name: String,
    pub os_family: String,
    pub registered_tool_names: Vec<String>,
    pub active_strategy_overlay: Option<String>,
    pub memory_injection: Option<MemoryInjectionArtifacts>,
    pub caller: &'static str,
    // ❌ 缺少 mode
    // ❌ 缺少 persona_id
    // ❌ 缺少 active_skill_ids
    // ❌ 缺少 options
}
```

**Gap 分析**：
1. **PromptBuildMode 缺失** → 所有请求走同一条路径，无法区分 Chat/Coding/Automation
2. **persona_id 缺失** → 无法注入不同 persona（未来多 persona 支持受阻）
3. **active_skill_ids 缺失** → skills 信息未进入 prompt plan（与 UClaw 的 skill block 不对齐）
4. **PromptBuildOptions 缺失** → 无法控制构建行为（如 strict validation）

**对齐建议 (P1)**：
```rust
pub enum PromptBuildMode {
    Chat,
    Coding,
    // 未来扩展: Automation, Compose
}

pub struct PromptBuildOptions {
    pub include_diagnostics: bool,
    pub strict_block_validation: bool,
}

pub struct BuildPromptPlanRequest {
    pub session_id: String,
    pub user_message: String,
    pub mode: PromptBuildMode,           // 新增
    pub persona_id: Option<String>,      // 新增
    pub active_skill_ids: Vec<String>,   // 新增
    pub workdir: PathBuf,
    pub current_date: String,
    pub os_name: String,
    pub os_family: String,
    pub registered_tool_names: Vec<String>,
    pub memory_injection: Option<MemoryInjectionArtifacts>,
    pub options: PromptBuildOptions,     // 新增
    pub caller: &'static str,
}
```

---

## 4. 核心机制对比

### 4.1 External Contributions

#### UClaw
```rust
pub struct PromptContribution {
    pub kind: PromptBlockKind,
    pub title: String,
    pub body: String,
    pub source: PromptBlockSource,
}

pub trait PromptPlanner {
    fn build_plan(
        &self,
        request: PromptBuildRequest,
        external_contributions: Vec<PromptContribution>,  // ✅ 外部贡献机制
    ) -> Result<PromptPlan>;
}

impl DefaultPromptPlanner {
    fn merge_external_contributions(
        &self,
        core: Vec<PromptBlock>,
        external: Vec<PromptContribution>,
        strict_mode: bool,
    ) -> Result<(Vec<PromptBlock>, Vec<PromptValidationIssue>)> {
        // 验证 + 合并逻辑
        // 禁止外部覆盖 Persona/Policy/UserIntent
        // 按 kind 分组插入到对应 core block 之后
    }
}
```

**设计价值**：
- Memory、MCP、Skills 等子系统可以独立贡献 prompt blocks
- 核心 planner 不需要知道所有子系统的细节
- 严格验证模式防止外部污染敏感 blocks

#### If2Ai
```rust
pub async fn build_prompt_plan(
    request: BuildPromptPlanRequest,
) -> Result<PromptPlanResult, PromptPlannerError> {
    // ❌ 所有 blocks 都在 planner 内部硬编码构建
    // ❌ 无法接受外部贡献
    // ❌ 扩展新 block 类型需要修改 planner 核心逻辑
}
```

**Gap 分析**：
- **PromptContribution 机制缺失** → 子系统无法独立贡献 blocks
- 当前 memory_injection 是通过 `Option<MemoryInjectionArtifacts>` 传入，但这是特例，不是通用机制
- 未来 MCP、Skills、Learning 等子系统都需要类似能力

**对齐建议 (P1)**：
```rust
pub struct PromptContribution {
    pub kind: PromptBlockKind,
    pub title: String,
    pub body: String,
    pub source: PromptBlockSource,
}

pub async fn build_prompt_plan(
    request: BuildPromptPlanRequest,
    external_contributions: Vec<PromptContribution>,  // 新增参数
) -> Result<PromptPlanResult, PromptPlannerError> {
    // 1. 构建 core blocks
    // 2. 合并 external contributions
    // 3. 验证 + 排序
    // 4. 生成 diagnostics
}
```

---

### 4.2 Strict Validation Mode

#### UClaw
```rust
impl DefaultPromptPlanner {
    fn merge_external_contributions(
        &self,
        core: Vec<PromptBlock>,
        external: Vec<PromptContribution>,
        strict_mode: bool,  // ✅ 严格模式开关
    ) -> Result<(Vec<PromptBlock>, Vec<PromptValidationIssue>)> {
        let mut issues = Vec::new();
        
        for ext in external {
            // 禁止外部覆盖敏感 blocks
            if matches!(ext.kind, Persona | Policy | UserIntent) {
                let issue = PromptValidationIssue { ... };
                if strict_mode {
                    return Err(anyhow!(issue.message));  // ✅ 严格模式直接失败
                }
                issues.push(issue);  // ✅ 宽松模式记录警告
            }
        }
        
        Ok((blocks, issues))
    }
}
```

**设计价值**：
- 生产环境可以用 strict mode 防止 prompt 污染
- 开发/测试环境可以用宽松模式收集警告
- 所有验证问题都记录在 diagnostics 中

#### If2Ai
```rust
// ❌ 无 strict validation 机制
// ❌ 无 validation issues 收集
// ❌ 构建失败只能通过 Result::Err 传播，无法区分"致命错误"和"警告"
```

**对齐建议 (P1)**：
- 在 PromptBuildOptions 中添加 `strict_block_validation: bool`
- 在 merge 逻辑中收集 validation issues
- strict mode 下遇到问题直接返回 Err
- 宽松模式下记录到 diagnostics.validation_issues

---

### 4.3 Priority-Based Block Ordering

#### UClaw
```rust
pub struct PromptBlock {
    pub priority: i32,  // ✅ 显式优先级
    // ...
}

impl DefaultPromptPlanner {
    fn build_core_blocks(&self, request: &PromptBuildRequest) -> Vec<PromptBlock> {
        let mut blocks = Vec::new();
        blocks.push(PromptBlock { priority: 100, kind: Persona, ... });
        blocks.push(PromptBlock { priority: 90, kind: Policy, ... });
        blocks.push(PromptBlock { priority: 80, kind: Environment, ... });
        blocks.push(PromptBlock { priority: 70, kind: Context, ... });
        blocks.push(PromptBlock { priority: 60, kind: CodingContext, ... });
        blocks.push(PromptBlock { priority: 50, kind: Continuation, ... });
        blocks.push(PromptBlock { priority: 40, kind: Skill, ... });
        blocks.push(PromptBlock { priority: 0, kind: UserIntent, ... });
        blocks
    }
    
    // 未来可以按 priority 重排序（token budget 优化时）
}
```

**设计价值**：
- 显式优先级使 block 顺序可预测
- 未来 token budget 紧张时可以按 priority 裁剪低优先级 blocks
- External contributions 可以指定插入位置

#### If2Ai
```rust
// ❌ Block 顺序完全由代码执行顺序决定
// ❌ 无法动态调整顺序
// ❌ 未来 token budget 优化无法基于优先级裁剪
```

**对齐建议 (P0)**：
- 为每个 block 分配显式 priority
- 保持当前顺序不变（通过 priority 值体现）
- 为未来的 budget-aware reordering 做准备

---

## 5. 功能对比

### 5.1 Coding Mode 专用能力

#### UClaw
```rust
pub enum PromptBuildMode {
    Chat,
    Coding,      // ✅ 专用 Coding 模式
    Automation,
    Compose,
}

impl DefaultPromptPlanner {
    fn build_coding_augment_blocks(&self, request: &PromptBuildRequest) -> Vec<PromptBlock> {
        // ✅ Coding 模式专用增强
        // - workspace context
        // - tool surface
        // - instruction files
    }
    
    fn build_coding_continuation_block(&self, request: &PromptBuildRequest) -> Option<PromptBlock> {
        // ✅ Coding session compaction
        // - 根据 token budget 决定是否生成 continuation block
        // - 包含 current_work、pending_work、key_files
    }
}
```

**设计价值**：
- Coding 模式有专用的 context augmentation
- 支持长会话的 compaction + continuation
- 与 Chat 模式清晰分离

#### If2Ai
```rust
// ❌ 无 mode 区分
// ❌ 无 Coding 专用增强
// ❌ 无 continuation/compaction 机制
```

**对齐建议 (P2)**：
- 短期：添加 PromptBuildMode 枚举，但所有模式走相同逻辑
- 长期：为 Coding 模式实现专用增强（参考 UClaw 的 coding_prompt_augment.rs）

---

### 5.2 Persona 和 Skills 注入

#### UClaw
```rust
pub struct PromptBuildRequest {
    pub persona_id: Option<String>,      // ✅ Persona 注入
    pub active_skill_ids: Vec<String>,   // ✅ Skills 注入
    // ...
}

impl DefaultPromptPlanner {
    fn build_core_blocks(&self, request: &PromptBuildRequest) -> Vec<PromptBlock> {
        blocks.push(PromptBlock {
            kind: PromptBlockKind::Persona,
            body: request.persona_id.clone().unwrap_or_else(|| "default".to_string()),
            // ...
        });
        
        if !request.active_skill_ids.is_empty() {
            blocks.push(PromptBlock {
                kind: PromptBlockKind::Skill,
                body: request.active_skill_ids.join(","),
                // ...
            });
        }
    }
}
```

#### If2Ai
```rust
// ❌ 无 persona_id 字段
// ❌ 无 active_skill_ids 字段
// ❌ Skills 信息未进入 prompt plan
```

**对齐建议 (P1)**：
- 添加 persona_id 和 active_skill_ids 到 BuildPromptPlanRequest
- 生成对应的 Persona 和 Skill blocks
- 为未来多 persona 支持做准备

---

## 6. 测试覆盖对比

### UClaw
- ✅ 11 个单元测试覆盖核心场景
- ✅ 测试 external contributions 合并逻辑
- ✅ 测试 strict validation 模式
- ✅ 测试 block 顺序保证（UserIntent 始终最后）
- ✅ 测试 coding continuation 触发条件
- ✅ 测试 diagnostics redaction

### If2Ai
- ✅ 3 个单元测试覆盖基础场景
- ✅ 测试 block hash 稳定性
- ✅ 测试 trace_id 生成
- ❌ 无 external contributions 测试（因为功能不存在）
- ❌ 无 validation 测试
- ❌ 无 priority 测试

**对齐建议**：
- 随着新功能添加，同步增加测试覆盖

---

## 7. 对齐路线图

### Phase 1: P0 数据结构对齐 (1-2 天)

**目标**：让 If2Ai 的 PromptBlock 和 PromptPlanDiagnostics 与 UClaw 对齐

**任务**：
1. 为 PromptBlock 添加 source、priority、is_sensitive 字段
2. 将 title 从 `&'static str` 改为 `String`
3. 创建 PromptBlockSource 结构体
4. 为 PromptPlanDiagnostics 添加 validation_issues 字段
5. 创建 PromptValidationIssue 结构体
6. 更新所有 block 构建代码，填充新字段
7. 更新测试

**验收**：
- `cargo check` 通过
- 所有现有测试通过
- 新字段有合理的默认值

---

### Phase 2: P1 机制对齐 (3-5 天)

**目标**：引入 PromptContribution 机制和 PromptBuildMode

**任务**：
1. 创建 PromptContribution 结构体
2. 创建 PromptBuildMode 枚举
3. 创建 PromptBuildOptions 结构体
4. 更新 BuildPromptPlanRequest，添加 mode、persona_id、active_skill_ids、options
5. 更新 build_prompt_plan 签名，添加 external_contributions 参数
6. 实现 merge_external_contributions 逻辑
7. 实现 strict validation 模式
8. 更新 TurnService 调用点
9. 添加测试覆盖

**验收**：
- External contributions 可以正常合并
- Strict validation 模式可以正常工作
- 所有测试通过

---

### Phase 3: P2 模块化重构 (1-2 周)

**目标**：将 prompt_planner/mod.rs 拆分为多个子模块

**任务**：
1. 创建 prompt_planner/block.rs（PromptBlock + PromptBlockSource + PromptContribution）
2. 创建 prompt_planner/diagnostics.rs（PromptPlanDiagnostics + PromptValidationIssue）
3. 创建 prompt_planner/build_request.rs（BuildPromptPlanRequest + PromptBuildMode + PromptBuildOptions）
4. 创建 prompt_planner/planner.rs（build_prompt_plan 核心逻辑）
5. 更新 mod.rs 为纯导出模块
6. 验证所有导入路径正确

**验收**：
- 模块结构清晰
- 每个文件 < 400 LOC
- 所有测试通过

---

### Phase 4: P2 Coding Mode 增强 (可选，长期)

**目标**：为 Coding 模式实现专用增强

**任务**：
1. 实现 coding continuation block 生成逻辑
2. 实现 compaction policy
3. 实现 workspace context augmentation
4. 添加 Coding 模式专用测试

**验收**：
- Coding 模式有明显区别于 Chat 模式的 prompt 结构
- Continuation block 在长会话中正确触发

---

## 8. 风险与注意事项

### 8.1 Breaking Changes

**风险**：PromptBlock 结构变更会影响所有消费者

**缓解**：
- 分阶段迁移，先添加新字段（带默认值），再逐步填充
- 保持 PromptPlan.join_into_text() 输出不变
- 添加兼容性测试

### 8.2 性能影响

**风险**：priority 排序、validation 检查可能增加开销

**缓解**：
- 当前 block 数量很少（< 10），性能影响可忽略
- 如果未来 block 数量增加，可以优化排序算法

### 8.3 测试覆盖

**风险**：新功能可能引入回归

**缓解**：
- 每个 Phase 都要求测试覆盖
- 保持现有测试通过
- 添加集成测试验证端到端流程

---

## 9. 总结

### 9.1 核心差距

| 维度 | UClaw | If2Ai | Gap 等级 |
|------|-------|-------|----------|
| 模块化 | 10+ 子模块 | 单文件 | P2 |
| PromptBlockSource | ✅ | ❌ | P0 |
| priority | ✅ | ❌ | P0 |
| is_sensitive | ✅ | ❌ | P0 |
| validation_issues | ✅ | ❌ | P0 |
| PromptContribution | ✅ | ❌ | P1 |
| PromptBuildMode | ✅ | ❌ | P1 |
| strict validation | ✅ | ❌ | P1 |
| persona_id | ✅ | ❌ | P1 |
| active_skill_ids | ✅ | ❌ | P1 |
| coding continuation | ✅ | ❌ | P2 |
| compaction policy | ✅ | ❌ | P2 |

### 9.2 对齐价值

**立即价值 (P0)**：
- PromptBlockSource → harness 可以追踪 block 来源，归因问题
- priority → 为未来 token budget 优化做准备
- is_sensitive → 更灵活的敏感内容控制
- validation_issues → 更好的诊断能力

**短期价值 (P1)**：
- PromptContribution → 子系统解耦，扩展性提升
- PromptBuildMode → 为 Coding/Automation 模式差异化做准备
- strict validation → 生产环境 prompt 质量保证
- persona/skills → 多 persona 支持基础

**长期价值 (P2)**：
- 模块化 → 代码可维护性提升
- coding continuation → 长会话支持
- compaction policy → 更好的 token 管理

### 9.3 建议行动

**立即执行**：
- Phase 1 (P0 数据结构对齐) — 1-2 天工作量，收益明显

**短期规划**：
- Phase 2 (P1 机制对齐) — 3-5 天工作量，为子系统解耦做准备

**长期规划**：
- Phase 3 (P2 模块化重构) — 1-2 周工作量，提升代码质量
- Phase 4 (P2 Coding Mode 增强) — 可选，根据产品需求决定

---

## 10. 参考资料

- [UClaw prompt planner.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/prompt/planner.rs)
- [UClaw prompt block.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/prompt/block.rs)
- [UClaw prompt diagnostics.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/prompt/diagnostics.rs)
- [UClaw prompt build_request.rs](/Users/ryanliu/Documents/iClaw/UClaw/uclaw-rs/src/prompt/build_request.rs)
- [If2Ai prompt_planner/mod.rs](/Users/ryanliu/Documents/IfAI/if2Ai/src-tauri/src/modules/application/prompt_planner/mod.rs)
- [If2Ai Staff Remediation Blueprint](./if2ai-staff-remediation-blueprint.md) §8.1.C
- [MIG-004 Pack](/Users/ryanliu/Documents/IfAI/if2Ai/docs/packs/feature/migration-core/MIG-004-prompt-planning-traceability.md)
