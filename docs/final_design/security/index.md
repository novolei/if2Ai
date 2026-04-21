# Security 安全与权限系统

> If2Ai 的分层防御框架 —— 路径验证、原子写入、ThreatScanner、PII 检测与权限控制。

## 📚 文档目录

| 文件 | 面向 | 内容 |
|------|------|------|
| [01-usage-guide.md](./01-usage-guide.md) | 用户 | 权限框架、路径验证、安全配置、工具权限等级 |
| [02-implementation.md](./02-implementation.md) | 开发者 | 权限架构、原子写入、ThreatScanner 共享、审计日志 |

## 📍 核心概念速查

| 概念 | 说明 |
|------|------|
| **validate_safe_path** | 路径安全验证，防止 `../` 遍历和符号链接逃逸 |
| **atomic_write** | 原子文件写入，先写 `.tmp` → sync → rename，崩溃安全 |
| **MemoryAccessContext** | 内存访问上下文，基于分类的读写权限控制 |
| **validate_memory_entry** | 输入验证，防注入（XSS / 模板注入 / 空字节） |
| **ThreatScanner** | 共享安全扫描器（skills + memory 模块共用），60+ 规则 |
| **PII 检测** | 个人身份信息检测，60+ 正则模式，15 类威胁 |
| **PathError** | 路径验证错误类型 |
| **ValidationError** | 输入验证错误类型 |

## 🏗️ 源码位置

```
src-tauri/src/modules/security/
├── mod.rs           # 模块入口 + 公开导出
├── access.rs        # ⭐ 内存访问控制（分类权限）
├── atomic_write.rs  # ⭐ 原子文件写入（崩溃安全）
├── path.rs          # ⭐ 路径验证（遍历攻击防护）
└── validation.rs    # ⭐ 输入验证（注入攻击防护）
```

**跨模块共享**：
- `skills/guard/` — ThreatScanner 实例与 security 共享规则
- `memory/` — 使用 `validate_memory_entry` + `MemoryAccessContext`

## 🔗 相关链接

- [Skills 模块](../skills/) — ThreatScanner 安全扫描子系统
- [Voice 模块](../voice/) — 语音文件路径验证
- [Browser 模块](../browser/) — 浏览器操作安全边界
