# ADR-009 Trajectory Learning Timing (P3) — 详细实施 backlog

## 概述

**目标**: 实现轨迹学习，仅作为导出用途（不运行 RL 训练）
**ADR**: [ADR-009](./ADR-009-Trajectory-Learning-Timing-P3.md)
**优先级**: P3
**预估工时**: 3-4 天

---

## 子任务清单

### TASK-009-01: Trajectory 数据结构

**目标**: 定义 ShareGPT 格式的轨迹结构

**具体任务**:
- [ ] 创建 `src-tauri/src/modules/learning/trajectory.rs`
- [ ] 定义核心结构:
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct Trajectory {
      pub id: String,
      pub conversations: Vec<ConversationEntry>,
      pub model_id: String,
      pub system: String,
      pub temperature: f32,
      pub turn_metadata: TurnMetadata,
  }

  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct ConversationEntry {
      pub from: String,  // "human" or "gpt"
      pub value: String,
  }

  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct TurnMetadata {
      pub session_id: String,
      pub timestamp: String,
      pub token_count: u64,
      pub tools_used: Vec<String>,
  }
  ```
- [ ] 实现 `Trajectory::from_session(session: &Session, system_prompt: &str, model_id: &str) -> Self`
- [ ] 实现 `to_jsonl(&self) -> String`
- [ ] 编写测试

**验收标准**:
- [ ] 结构符合 ShareGPT 格式
- [ ] 可从 Session 转换
- [ ] JSONL 序列化正确

---

### TASK-009-02: TrajectoryManager 实现

**目标**: 实现轨迹管理器

**具体任务**:
- [ ] 定义 `TrajectoryManager`:
  ```rust
  pub struct TrajectoryManager {
      base_path: PathBuf,
      max_file_size: usize,  // 100MB
  }
  ```
- [ ] 实现 `new(base_path: PathBuf) -> Result<Self, TrajectoryError>`
- [ ] 实现 `record(&self, session: &Session, system_prompt: &str, model_id: &str) -> Result<String>`
  - 创建 Trajectory
  - 追加到当日文件
- [ ] 实现 `export_all(&self, output_path: &Path) -> Result<u64>`
- [ ] 编写测试

**验收标准**:
- [ ] 轨迹正确记录到文件
- [ ] 导出合并所有文件
- [ ] 文件轮转正确

---

### TASK-009-03: 文件轮转

**目标**: 实现自动文件轮转

**具体任务**:
- [ ] 实现 `current_file(&self) -> PathBuf`
  - 格式: `trajectory_YYYY-MM-DD.jsonl`
- [ ] 实现 `should_rotate(&self) -> bool`
  - 检查当前文件大小 > max_file_size
- [ ] 实现 `rotate(&self) -> Result<()>`
  - 将当前文件重命名为 archive
- [ ] 编写测试

**验收标准**:
- [ ] 按日期正确轮转
- [ ] 大小超限时正确轮转
- [ ] 归档文件命名正确

---

### TASK-009-04: 隐私控制

**目标**: 实现隐私保护

**具体任务**:
- [ ] 定义 `TrajectoryPrivacy`:
  ```rust
  #[derive(Debug, Clone)]
  pub struct TrajectoryPrivacy {
      pub include_system_prompt: bool,  // default: false
      pub include_tool_calls: bool,     // default: true
      pub min_session_length: usize,    // default: 3
      pub anonymize_user_content: bool, // default: true
  }
  ```
- [ ] 在 `record()` 中应用隐私过滤
- [ ] 编写测试验证隐私控制

**验收标准**:
- [ ] system_prompt 可选包含
- [ ] 短会话被过滤
- [ ] 用户内容可脱敏

---

### TASK-009-05: 压缩器 (可选)

**目标**: 实现轨迹压缩

**具体任务**:
- [ ] 定义 `TrajectoryCompressor`:
  ```rust
  pub struct TrajectoryCompressor {
      min_success_rate: f32,
      max_length: usize,
  }
  ```
- [ ] 实现 `compress(&self, trajectories: Vec<Trajectory>) -> Vec<Trajectory>`
  - 过滤低质量轨迹
  - 截断超长轨迹
- [ ] 编写测试

**验收标准**:
- [ ] 低质量轨迹被过滤
- [ ] 超长轨迹被截断

---

### TASK-009-06: Tauri 命令导出

**目标**: 提供 IPC 导出接口

**具体任务**:
- [ ] 在 `src-tauri/src/commands/` 添加导出命令
- [ ] 实现 `#[tauri::command] export_trajectories(output_path: String) -> Result<u64, String>`
- [ ] 实现 `#[tauri::command] get_trajectory_count() -> Result<u64, String>`
- [ ] 前端调用示例

**验收标准**:
- [ ] 命令正确暴露
- [ ] 返回正确的轨迹数量
- [ ] 前端可调用

---

## 优先级排序

| 优先级 | Task | 理由 |
|--------|------|------|
| P0 | TASK-009-01 | 轨迹结构 |
| P0 | TASK-009-02 | 轨迹管理器 |
| P1 | TASK-009-03 | 文件轮转 |
| P1 | TASK-009-04 | 隐私控制 |
| P2 | TASK-009-05 | 压缩器 |
| P2 | TASK-009-06 | 导出命令 |

---

## 验收总览

- [ ] Trajectory 结构符合 ShareGPT
- [ ] Session 可转换为 Trajectory
- [ ] 轨迹记录到 JSONL 文件
- [ ] 文件轮转正确
- [ ] 隐私控制有效
- [ ] 导出命令可用
