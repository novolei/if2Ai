# 学习与自我改进（Learning）

> If2Ai 独有的学习与自我改进系统：让 AI 代理理解自身能力并持续进化

## 📚 子文档目录

| 文档 | 面向 | 内容 |
|------|------|------|
| [01-usage-guide.md](./01-usage-guide.md) | 用户 | 自我模型、反思引擎、策略管理使用指南 |
| [02-implementation.md](./02-implementation.md) | 开发者 | 架构实现、数据流、差距分析 |

## 📍 核心概念速查表

| 概念 | 说明 | 源码位置 |
|------|------|----------|
| SelfModel | AI 对自身能力的认知模型 | `learning/self_model.rs` |
| ReflectionEngine | 反思引擎，分析会话并提取模式 | `learning/reflection.rs` |
| TrajectoryManager | ShareGPT JSONL 轨迹记录 | `learning/trajectory.rs` |
| StrategyRegistry | 策略注册表，管理策略生命周期 | `learning/strategy_registry.rs` |
| PromotionGate | 推广门控，决定策略是否上线 | `learning/promotion_gate.rs` |
| CandidateEvaluator | 候选策略评估器 | `learning/candidate_evaluator.rs` |
| TrustTracker | 信任追踪器 | `learning/trust_tracker.rs` |
| FailureClustering | 失败聚类分析 | `learning/failure_clustering.rs` |
| ActiveOverlay | 活跃策略叠加层 | `learning/active_overlay.rs` |

## 🔗 相关链接

- [记忆模块](../memory/) — 学习系统读取记忆但不直接修改
- [桌面架构](../desktop/02-architecture.md) — LearningModule 在 AppState 中的位置
- [If2Ai 项目概述](../../ARCHITECTURE.md)
