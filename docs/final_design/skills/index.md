# Skills 技能系统

> If2Ai 的技能发现、安装、执行与安全扫描框架 —— 15 类 60+ 威胁模式保驾护航。

## 📚 文档目录

| 文件 | 面向 | 内容 |
|------|------|------|
| [01-usage-guide.md](./01-usage-guide.md) | 用户 | 技能发现、安装、执行、编辑、安全审查 |
| [02-implementation.md](./02-implementation.md) | 开发者 | SkillManager 架构、ThreatScanner、加载机制、执行沙箱 |

## 📍 核心概念速查

| 概念 | 说明 |
|------|------|
| **Skill** | 一个可被 AI 代理调用的能力单元，由 `SKILL.md` + 辅助文件组成 |
| **SkillsGuard** | 安全扫描器，15 类 60+ 正则模式静态分析 |
| **TrustLevel** | 信任等级：`Builtin` / `Trusted` / `Community` / `AgentCreated` |
| **SkillHub** | 多源技能市场适配器（GitHub / ClawHub / skills.sh） |
| **SkillContext** | 技能操作上下文，持有 `skills_dir` + `SkillsGuard` 实例 |
| **ThreatCategory** | 威胁分类：Exfiltration、Injection、Destructive 等 15 类 |
| **InstallPolicy** | 安装策略：根据扫描结果 + 信任等级决定是否允许安装 |
| **HubPaths** | 技能市场状态目录：lock.json、audit.log、quarantine 等 |

## 🏗️ 源码位置

```
src-tauri/src/modules/skills/
├── mod.rs                  # 模块入口
├── commands.rs             # 斜杠命令集成（/skill-name 调用）
├── config.rs               # 技能配置
├── external_dirs.rs        # 外部技能目录支持
├── remote_passthrough.rs   # 远程后端环境变量透传
├── guard/                  # ⭐ 安全扫描子系统
│   ├── mod.rs              #    SkillsGuard 主逻辑
│   ├── threat_patterns.rs  #    60+ 正则威胁模式
│   ├── policy.rs           #    安装策略 + 信任等级
│   ├── invisible_unicode.rs#    不可见 Unicode 检测
│   └── structural_limits.rs#    结构性限制检查
├── hub/                    # ⭐ 技能市场子系统
│   ├── mod.rs              #    Hub 入口
│   ├── github.rs           #    GitHub 源适配器
│   ├── clawhub.rs          #    ClawHub 源适配器
│   ├── skills_sh.rs        #    skills.sh 源适配器
│   ├── marketplace.rs      #    市场搜索
│   ├── source.rs           #    SkillSource trait
│   ├── state.rs            #    状态管理（lock/quarantine/audit）
│   └── types.rs            #    Hub 类型定义
├── manager/                # ⭐ 技能管理子系统
│   ├── mod.rs              #    SkillContext + SkillError
│   ├── actions.rs          #    CRUD 操作
│   ├── atomic_write.rs     #    原子写入
│   └── validator.rs        #    名称/内容/路径验证
├── snapshot/               # 技能导出/导入
│   ├── mod.rs
│   └── types.rs
└── sync/                   # 内置技能同步
    ├── mod.rs
    └── manifest.rs
```

## 🔗 相关链接

- [Security 模块](../security/) — ThreatScanner 与 security 模块共享安全基础设施
- [Browser 模块](../browser/) — 浏览器技能可调用 BrowserSession
- [cc-haha 技能系统差距分析](./02-implementation.md#️-与-cc-haha-差距分析)
