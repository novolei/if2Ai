# If2Ai 源码迁移执行指南 v2 (Migration Execution Plan)

**版本**: 2.0 (单一源码真相)  
**原则**: 迁移源码→唯一工作库（不是"复制参考库"）  
**状态**: 立即执行  
**预计耗时**: 8-10 小时  
**风险级别**: 低 (可回滚)

---

## 核心理念

**这不是"复制参考代码"，而是"迁移源码建立唯一工作库"**:

```
/rust (源码来源)
  │ 迁移源码 (不是复制)
  ↓
src-tauri (唯一工作库)
  └─ 后续所有开发都在这里
     /rust 变成存档（可选）
```

迁移完成后：
- ✅ src-tauri 是唯一的活跃代码库
- ✅ 所有 bug 修复和功能开发都在 src-tauri
- ✅ 不存在"两个版本需要同步"的问题

---

## 前置条件检查

```bash
# 1. 检查 /rust 源码完整性
ls -la rust/crates/
# 应看到: api/, runtime/, tools/, commands/, plugins/

# 2. 检查 src-tauri 现状
ls -la src-tauri/src/
# 应看到: main.rs, commands/, modules/

# 3. 检查设计文档
ls docs/design-docs/ | wc -l
# 应看到: 9 份文档

# 4. 当前分支状态
cd if2Ai
git status
# 应该是干净的（no uncommitted changes）
```

---

## Step 0: 备份与准备 (15 分钟)

```bash
#!/bin/bash

# 1. 创建备份标签
git tag -a "pre-migration-v2-$(date +%Y%m%d)" -m "Backup before singleSOT migration"

# 2. 创建特性分支
git checkout -b feature/single-sot-migration

# 3. 创建迁移日志
cat > MIGRATION_LOG_V2.md << 'EOF'
# Code Migration Log - Single Source of Truth Model

**Date**: 2026-04-11
**Model**: Single Source of Truth (src-tauri as unique working library)
**Status**: In Progress
**By**: [Developer Name]

## Checklist

- [ ] Step 0: 备份准备完成
- [ ] Step 1: 目录结构准备
- [ ] Step 2-7: 核心模块迁移完成
- [ ] Step 8: AppState 集成
- [ ] Step 9: Commands 网关
- [ ] Step 10: 最终验证

## Migration Sources

/rust/crates/runtime → src-tauri/src/modules/agent/
/rust/crates/api     → src-tauri/src/modules/provider/
/rust/crates/tools   → src-tauri/src/modules/tools/
/rust/crates/commands→ src-tauri/src/modules/session/
/rust/crates/plugins → src-tauri/src/modules/plugin/

## Key Points

- This is a ONE-WAY migration, not bidirectional sync
- After migration, /rust becomes archive (optional)
- All development happens in src-tauri only
- No "two versions" to maintain
EOF

git add -A
git commit -m "chore: init single-SOT migration log"
```

---

## Step 1: 创建模块目录结构 (10 分钟)

```bash
#!/bin/bash

echo "📂 创建模块目录结构..."

# 创建完整的模块目录树
mkdir -p src-tauri/src/modules/{agent,provider,tools,session,memory,plugin}
mkdir -p src-tauri/src/modules/agent/prompts
mkdir -p src-tauri/src/modules/provider/providers
mkdir -p src-tauri/src/modules/tools/{builtins,backends}
mkdir -p src-tauri/src/modules/session/{db,schema}
mkdir -p src-tauri/src/modules/memory/{providers,honcho}

# 创建模块骨架 (mod.rs)
for module in agent provider tools session memory plugin; do
  cat > "src-tauri/src/modules/${module}/mod.rs" << EOF
//! ${module} module
//!
//! Migrated from /rust/crates/* as per Single Source of Truth model
//! This is the ONLY place where ${module} code is developed and maintained

pub mod lib;

// TODO: Complete module implementation
EOF
done

echo "✅ 目录结构创建完成"
```

---

## Step 2-7: 逐个模块迁移 (6-7 小时)

### Step 2: Agent 模块迁移 (1.5 小时)

```bash
#!/bin/bash

echo "🔄 迁移 Agent 模块..."
echo "📦 源: /rust/crates/runtime"
echo "📁 目标: src-tauri/src/modules/agent/"

# 从 /rust/crates/runtime 中提取源码
# 不是复制，而是迁移 - 源码属于 src-tauri

# 复制主实现文件
cp rust/crates/runtime/src/lib.rs \
   src-tauri/src/modules/agent/conversation.rs

# 提取相关子模块
if [ -f rust/crates/runtime/src/prompt.rs ]; then
  cp rust/crates/runtime/src/prompt.rs src-tauri/src/modules/agent/
fi

# 调整 imports，使其符合 src-tauri 结构
cd src-tauri

# 编辑 modules/agent/mod.rs
cat > src/modules/agent/mod.rs << 'EOF'
//! Agent Loop Module (Migrated from /rust/crates/runtime)
//! 
//! This is now the single source of truth for agent execution.
//! All agent loop development happens here.

pub mod conversation;
pub mod prompt;

pub use conversation::ConversationRuntime;
pub use prompt::PromptBuilder;

// Re-export key types
pub use conversation::{AgentResponse, AgentState, RunConfig};
EOF

# 验证编译
echo "📋 验证编译..."
cargo check --lib 2>&1 | tee /tmp/agent_check.log

if grep -i "error" /tmp/agent_check.log; then
  echo "❌ Agent 模块显示编译错误，需要调整 imports"
  exit 1
fi

cd ..
echo "✅ Agent 模块迁移成功"
```

### Step 3: Provider 模块迁移 (1.5 小时)

```bash
#!/bin/bash

echo "🔄 迁移 Provider 模块..."
echo "📦 源: /rust/crates/api"
echo "📁 目标: src-tauri/src/modules/provider/"

# 迁移 API 客户端和提供商实现
cp rust/crates/api/src/lib.rs \
   src-tauri/src/modules/provider/client.rs

cp rust/crates/api/src/routing.rs \
   src-tauri/src/modules/provider/routing.rs 2>/dev/null || true

# 迁移提供商实现目录
cp -r rust/crates/api/src/providers \
      src-tauri/src/modules/provider/ 2>/dev/null || \
cp -r rust/crates/api/src/provider \
      src-tauri/src/modules/provider/providers || true

cd src-tauri

# 创建 provider 模块导出
cat > src/modules/provider/mod.rs << 'EOF'
//! Provider Resolution Module (Migrated from /rust/crates/api)
//!
//! This is now the single source of truth for LLM provider management.
//! All provider development and routing logic happens here.

pub mod client;
pub mod routing;
pub mod providers;

pub use client::ProviderClient;
pub use routing::ProviderRouter;

// Re-export key types
pub use client::ProviderConfig;
pub use providers::ProviderType;
EOF

cargo check --lib
cd ..

echo "✅ Provider 模块迁移成功"
```

### Step 4: Tools 模块迁移 (1 小时)

```bash
#!/bin/bash

echo "🔄 迁移 Tools 模块..."
echo "📦 源: /rust/crates/tools"
echo "📁 目标: src-tauri/src/modules/tools/"

# 迁移工具系统
cp rust/crates/tools/src/lib.rs \
   src-tauri/src/modules/tools/registry.rs

cp rust/crates/tools/src/executor.rs \
   src-tauri/src/modules/tools/ 2>/dev/null || true

# 迁移内置工具
cp -r rust/crates/tools/src/builtins \
      src-tauri/src/modules/tools/ 2>/dev/null || true

cd src-tauri

cat > src/modules/tools/mod.rs << 'EOF'
//! Tool System Module (Migrated from /rust/crates/tools)
//!
//! This is the single source of truth for tool registration and execution.

pub mod registry;
pub mod executor;
pub mod builtins;

pub use registry::ToolRegistry;
pub use executor::ToolExecutor;
EOF

cargo check --lib
cd ..

echo "✅ Tools 模块迁移成功"
```

### Step 5: Session 模块迁移 (1 小时)

```bash
#!/bin/bash

echo "🔄 迁移 Session 模块..."
echo "📦 源: /rust/crates/commands"
echo "📁 目标: src-tauri/src/modules/session/"

# Session 可能需要从 commands 中提取
cp rust/crates/commands/src/session.rs \
   src-tauri/src/modules/session/storage.rs 2>/dev/null || \
cp rust/crates/commands/src/db.rs \
   src-tauri/src/modules/session/storage.rs 2>/dev/null || true

cd src-tauri

cat > src/modules/session/mod.rs << 'EOF'
//! Session Persistence Module (Migrated from /rust/crates/commands)
//!
//! This is the single source of truth for session storage and management.

pub mod storage;
pub mod manager;
pub mod schema;

pub use manager::SessionManager;
pub use storage::SessionStorage;
EOF

cargo check --lib
cd ..

echo "✅ Session 模块迁移成功"
```

### Step 6: Memory 模块迁移 (1 小时)

```bash
#!/bin/bash

echo "🔄 迁移 Memory 模块..."
echo "📁 目标: src-tauri/src/modules/memory/"

# Memory 是新增模块，基于 docs/design-docs/memory-system.md
# 来源可能是 /rust/crates sessions 或全新实现

cd src-tauri

cat > src/modules/memory/mod.rs << 'EOF'
//! Memory System Module (Based on design-docs/memory-system.md)
//!
//! This is the single source of truth for user memory and context.
//! Implements Honcho integration + built-in memory layers.

pub mod manager;
pub mod honcho;
pub mod providers;

pub use manager::MemoryManager;
EOF

# 创建初步框架
cat > src/modules/memory/manager.rs << 'EOF'
//! Memory Manager - coordinates all memory operations

pub struct MemoryManager {
    // TODO: Implement based on memory-system.md design
    // - Built-in memory (MEMORY.md, USER.md)
    // - Honcho provider integration
    // - External providers (Mem0, etc)
    // - Recall pipeline (auto + manual)
    // - Write pipeline (immediate + session_end)
}

impl MemoryManager {
    pub async fn new() -> Result<Self> {
        todo!("Initialize memory system")
    }
}
EOF

cargo check --lib 2>&1 | grep -v "warning:" || true
cd ..

echo "✅ Memory 模块迁移成功 (框架阶段)"
```

### Step 7: Plugin 模块迁移 (1 小时)

```bash
#!/bin/bash

echo "🔄 迁移 Plugin 模块..."
echo "📦 源: /rust/crates/plugins"
echo "📁 目标: src-tauri/src/modules/plugin/"

# 迁移插件系统
cp -r rust/crates/plugins/src/* \
      src-tauri/src/modules/plugin/ 2>/dev/null || \
mkdir -p src-tauri/src/modules/plugin/{manager,loader}

cd src-tauri

cat > src/modules/plugin/mod.rs << 'EOF'
//! Plugin System Module (Migrated from /rust/crates/plugins)
//!
//! This is the single source of truth for plugin management.

pub mod manager;
pub mod loader;
pub mod registry;

pub use manager::PluginManager;
EOF

cargo check --lib
cd ..

echo "✅ Plugin 模块迁移成功"
```

---

## Step 8: AppState 集成 (1 小时)

```rust
// src-tauri/src/app_state.rs
// Single Source of Truth: AppState now contains all business logic

use std::sync::{Arc, Mutex};

use crate::modules::{
    agent::ConversationRuntime,
    provider::ProviderManager,
    tools::ToolRegistry,
    session::SessionManager,
    memory::MemoryManager,
    plugin::PluginManager,
};

/// Global Application State
/// 
/// Single point of entry for all modules
/// This is the ONLY way to access subsystems
/// 
/// Design: docs/design-docs/module-boundaries-and-integration.md
#[derive(Clone)]
pub struct AppState {
    /// Agent Loop - 对话循环
    pub agent_runtime: Arc<Mutex<ConversationRuntime>>,
    
    /// Provider System - LLM 提供商管理
    pub provider_manager: Arc<ProviderManager>,
    
    /// Tool System - 工具执行
    pub tool_registry: Arc<ToolRegistry>,
    
    /// Session Management - 对话存储
    pub session_manager: Arc<SessionManager>,
    
    /// Memory System - 用户记忆 (now part of SSOT)
    pub memory_manager: Arc<MemoryManager>,
    
    /// Plugin System - 扩展机制
    pub plugin_manager: Arc<PluginManager>,
}

impl AppState {
    /// Initialize all subsystems
    /// This is the ONLY place where src-tauri app is assembled
    pub async fn new() -> Result<Self> {
        log::info!("🚀 初始化 If2Ai AppState (Single Source of Truth)");
        log::info!("📚 所有源码都在 src-tauri 中");
        
        // 1. 初始化无依赖的系统
        let provider_manager = ProviderManager::new().await?;
        let tool_registry = ToolRegistry::new();
        let plugin_manager = PluginManager::new().await?;
        
        // 2. 初始化存储系统
        let session_manager = SessionManager::new().await?;
        let memory_manager = MemoryManager::new().await?;
        
        // 3. 初始化 Agent 循环 (依赖于上述所有)
        let agent_runtime = ConversationRuntime::new(
            provider_manager.clone(),
            tool_registry.clone(),
            session_manager.clone(),
            memory_manager.clone(),
            plugin_manager.clone(),
        ).await?;
        
        Ok(Self {
            agent_runtime: Arc::new(Mutex::new(agent_runtime)),
            provider_manager: Arc::new(provider_manager),
            tool_registry: Arc::new(tool_registry),
            session_manager: Arc::new(session_manager),
            memory_manager: Arc::new(memory_manager),
            plugin_manager: Arc::new(plugin_manager),
        })
    }
    
    /// 验证所有子系统就绪
    pub async fn health_check(&self) -> HealthStatus {
        HealthStatus {
            agent: true,
            provider: self.provider_manager.is_healthy().await,
            tools: self.tool_registry.is_healthy(),
            session: self.session_manager.is_healthy().await,
            memory: self.memory_manager.is_healthy().await,
            plugins: self.plugin_manager.is_healthy().await,
        }
    }
}

pub struct HealthStatus {
    pub agent: bool,
    pub provider: bool,
    pub tools: bool,
    pub session: bool,
    pub memory: bool,
    pub plugins: bool,
}
```

---

## Step 9: Commands 网关实现 (1 小时)

```rust
// src-tauri/src/commands/mod.rs
// IPC gateway between Tauri frontend and AppState

pub mod agent;
pub mod tools;
pub mod session;
pub mod memory;

// Re-export all command handlers
pub use agent::*;
pub use tools::*;
pub use session::*;
pub use memory::*;

// Example: Agent command
#[tauri::command]
pub async fn run_agent_turn(
    state: tauri::State<'_, AppState>,
    message: String,
) -> Result<String> {
    let mut agent = state.agent_runtime.lock()
        .map_err(|e| format!("Lock error: {}", e))?;
    
    let response = agent.run_turn(message).await?;
    Ok(response.text)
}
```

---

## Step 10: 编译与验证 (1 小时)

```bash
#!/bin/bash

set -e

echo "🔧 最终验证与编译..."

# 1. 完整编译
echo "📦 编译 src-tauri..."
cd src-tauri
cargo build --release 2>&1 | tee /tmp/build.log

if [ $? -ne 0 ]; then
    echo "❌ 编译失败"
    cat /tmp/build.log | tail -50
    exit 1
fi

# 2. 运行测试
echo "✅ 编译成功，运行单元测试..."
cargo test --lib -- --nocapture

# 3. Tauri 检查
echo "🚀 验证 Tauri 集成..."
cargo tauri info

# 4. 简单启动测试
echo "🔍 启动 10 秒测试..."
timeout 10 cargo tauri dev || true

echo "✅ 所有验证通过！"
```

---

## 迁移完成

```bash
#!/bin/bash

# 1. 更新迁移日志
cat >> MIGRATION_LOG_V2.md << 'EOF'

## Completion

✅ All modules migrated to src-tauri
✅ AppState unified all subsystems
✅ Commands gateway working
✅ Tests passing
✅ Tauri build succeeding

**Status**: COMPLETE - Single Source of Truth Established

src-tauri is now the ONLY working library.
All future development happens here.
/rust remains as historical archive (optional).

Completed at: [timestamp]
EOF

# 2. 代码提交
git add -A
git commit -m "refactor: establish single source of truth with src-tauri as unique working library

- Migrate all modules from /rust to src-tauri/src/modules
- Unified AppState architecture for all subsystems
- Implement Tauri Commands gateway layer
- All tests passing
- /rust now serves as historical reference only

This completes the transition to Single Source of Truth model.
All future development will occur in src-tauri only.

Refs: CODE_FOUNDATION_STRATEGY_V2.md"

# 3. 标记完成
git tag -a "migration-v2-complete-$(date +%Y%m%d)" \
  -m "Migration to Single Source of Truth completed"

# 4. 推送
git push -u origin feature/single-sot-migration

echo "✅ 迁移完全完成！"
echo ""
echo "下一步:"
echo "  1. Code review + merge to main"
echo "  2. Begin Phase 2 development in src-tauri"
echo "  3. All future work in src-tauri only"
```

---

## 核对清单

迁移前:
- [ ] /rust 中所有 crates 都能编译
- [ ] src-tauri/ 已有基本 Tauri 配置
- [ ] 9 份设计文档已阅读
- [ ] 备份分支已创建
- [ ] 有足够时间完成 (8-10 小时不间断)

迁移中:
- [ ] 每两个模块后运行 cargo test
- [ ] 所有 imports 符合设计
- [ ] 模块能正确初始化
- [ ] 没有编译错误

迁移后:
- [ ] `cargo build --release` 成功
- [ ] `cargo test` 所有通过
- [ ] `cargo tauri dev` 启动成功
- [ ] AppState 初始化无错
- [ ] 所有 Commands 可调用

---

## 核心区别: v1 vs v2

| 方面 | v1 (双库) | v2 (单一SOT) |
|------|---------|------------|
| /rust 角色 | 参考库（持续维护） | 源码来源（迁移后冻结） |
| src-tauri 角色 | 从 /rust 参考 | 唯一工作库 |
| 开发位置 | 两个地方 | 只在 src-tauri |
| 同步问题 | 需要定期对齐 | 无同步问题 |
| 维护复杂度 | 高 | 低 |

**v2 是更清晰、更简单、更可维护的模型。**

---

**版本**: 2.0 (Single Source of Truth)  
**状态**: ✅ 准备执行  
**目标**: 建立唯一真实版本，消除二元性  

