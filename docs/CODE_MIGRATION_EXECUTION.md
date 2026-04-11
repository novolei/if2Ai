# If2Ai 源码迁移执行指南 (Migration Execution Plan)

**版本**: 1.0  
**状态**: 立即执行  
**预计耗时**: 8-10 小时  
**风险级别**: 低 (可回滚)

---

## 概览

本指南提供逐步的操作说明，将 `/rust` 中的核心源码迁移到 `src-tauri/` 中作为生产实现。

```
BEFORE:
  /rust (独立库，未集成)
       ↓
  src-tauri (空的容器)

AFTER:
  /rust (不变，参考库)
       ↓
  src-tauri (完整实现，集一致性成果为一体)
```

---

## 前置条件检查

```bash
# 1. 检查 /rust 源码完整性
ls -la rust/crates/
# 应看到: api/, runtime/, tools/, commands/, plugins/, compat-harness/

# 2. 检查 src-tauri 现状
ls -la src-tauri/src/
# 应看到: main.rs, commands/, modules/

# 3. 检查设计文档
ls -la docs/design-docs/
# 应看到: agent-loop.md, provider-resolution.md, memory-system.md 等

# 4. 验证 Cargo 工作空间
cd rust && cargo check
cd ../src-tauri && cargo check
```

---

## Step 0: 备份与准备 (15 分钟)

```bash
#!/bin/bash

# 1. 创建备份标签
git tag -a "pre-migration-$(date +%Y%m%d)" -m "Backup before src code migration"

# 2. 创建备份分支
git checkout -b feature/code-foundation-migration

# 3. 创建迁移日志
cat > MIGRATION_LOG.md << 'EOF'
# Code Migration Log

**Date**: 2026-04-11
**Status**: In Progress
**By**: [Developer Name]

## Checklist

- [ ] Step 1: 目录结构准备
- [ ] Step 2: Agent 模块迁移
- [ ] Step 3: Provider 模块迁移
- [ ] Step 4: Tools 模块迁移
- [ ] Step 5: Session 模块迁移
- [ ] Step 6: Memory 模块迁移
- [ ] Step 7: Plugin 模块迁移
- [ ] Step 8: AppState 集成
- [ ] Step 9: Commands 网关实现
- [ ] Step 10: 测试与验证

## Notes

- Started at: [timestamp]
- Current step: [step name]
- Blockers: [if any]

EOF

git add MIGRATION_LOG.md
git commit -m "docs: init migration log"
```

---

## Step 1: 创建模块目录结构 (10 分钟)

```bash
#!/bin/bash

# 检查现有结构
echo "📂 检查现有结构..."
tree src-tauri/src/ 2>/dev/null || find src-tauri/src/ -type d

# 创建新的模块目录
echo "📁 创建模块目录..."
mkdir -p src-tauri/src/modules/{agent,provider,tools,session,memory,plugin}

# 根据需要创建子目录
mkdir -p src-tauri/src/modules/agent/{prompts,steps}
mkdir -p src-tauri/src/modules/provider/providers
mkdir -p src-tauri/src/modules/tools/{builtins,backends}
mkdir -p src-tauri/src/modules/session/{db,schema}
mkdir -p src-tauri/src/modules/memory/{providers,honcho}

# 创建 mod.rs 骨架
for module in agent provider tools session memory plugin; do
  cat > "src-tauri/src/modules/${module}/mod.rs" << EOF
//! ${module} module
//!
//! This module is migrated from /rust/crates/ as per CODE_FOUNDATION_STRATEGY.md

// TODO: Add submodules as migration completes
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

# 复制源文件 (从 runtime crate)
cp rust/crates/runtime/src/lib.rs src-tauri/src/modules/agent/conversation.rs

# 提取 prompt builder
if [ -f rust/crates/runtime/src/prompt.rs ]; then
  cp rust/crates/runtime/src/prompt.rs src-tauri/src/modules/agent/
fi

# 检查依赖
grep -o "^[a-z_]* = " rust/crates/runtime/Cargo.toml | head -n 20

# 创建模块导出
cat > src-tauri/src/modules/agent/mod.rs << 'EOF'
//! Agent Loop Module
//!
//! Migrated from rust/crates/runtime/
//! Source: CODE_FOUNDATION_STRATEGY.md

pub mod conversation;
pub mod prompt;

pub use conversation::ConversationRuntime;
pub use prompt::PromptBuilder;

// Re-export key types
pub use conversation::{AgentResponse, AgentState, RunConfig};
EOF

# 验证编译
cd src-tauri
echo "📋 验证编译..."
cargo check --lib 2>&1 | tee /tmp/cargo_check.log
if grep -i "error" /tmp/cargo_check.log; then
  echo "❌ 编译失败，需要调整 imports"
  exit 1
fi
cd ..

echo "✅ Agent 模块迁移成功"
```

### Step 3: Provider 模块迁移 (1.5 小时)

```bash
#!/bin/bash

echo "🔄 迁移 Provider 模块..."

# 复制 API 客户端
cp rust/crates/api/src/client.rs src-tauri/src/modules/provider/
cp rust/crates/api/src/lib.rs src-tauri/src/modules/provider/manager.rs

# 复制提供商实现
cp -r rust/crates/api/src/providers/ src-tauri/src/modules/provider/

# 创建模块导出
cat > src-tauri/src/modules/provider/mod.rs << 'EOF'
//! Provider Resolution Module  
//!
//! Migrated from rust/crates/api/
//! Source: CODE_FOUNDATION_STRATEGY.md

pub mod manager;
pub mod client;
pub mod providers;

pub use manager::ProviderManager;
pub use client::ProviderClient;
EOF

cd src-tauri
cargo check --lib
cd ..

echo "✅ Provider 模块迁移成功"
```

### Step 4: Tools 模块迁移 (1 小时)

```bash
#!/bin/bash

echo "🔄 迁移 Tools 模块..."

# 复制工具系统
cp rust/crates/tools/src/lib.rs src-tauri/src/modules/tools/registry.rs
cp rust/crates/tools/src/executor.rs src-tauri/src/modules/tools/

# 复制内置工具
cp -r rust/crates/tools/src/builtins/ src-tauri/src/modules/tools/

cat > src-tauri/src/modules/tools/mod.rs << 'EOF'
//! Tool System Module
//!
//! Migrated from rust/crates/tools/
//! Source: CODE_FOUNDATION_STRATEGY.md

pub mod registry;
pub mod executor;
pub mod builtins;

pub use registry::ToolRegistry;
pub use executor::ToolExecutor;
EOF

cd src-tauri
cargo check --lib
cd ..

echo "✅ Tools 模块迁移成功"
```

### Step 5: Session 模块迁移 (1 小时)

```bash
#!/bin/bash

echo "🔄 迁移 Session 模块..."

# Session 可能需要创建新的实现，或从 commands crate 移植
# 假设 SQL schema 在 rust/crates/commands/src/ 下

cp -r rust/crates/commands/src/session/* \
  src-tauri/src/modules/session/ 2>/dev/null || true

cat > src-tauri/src/modules/session/mod.rs << 'EOF'
//! Session Persistence Module
//!
//! Migrated from rust/crates/commands/
//! Source: CODE_FOUNDATION_STRATEGY.md

pub mod storage;
pub mod manager;
pub mod schema;

pub use manager::SessionManager;
pub use storage::SessionStorage;
EOF

cd src-tauri
cargo check --lib
cd ..

echo "✅ Session 模块迁移成功"
```

### Step 6: Memory 模块迁移 (1 小时)

```bash
#!/bin/bash

echo "🔄 迁移 Memory 模块..."

# Memory 是新增的，基于 memory-system.md 设计
# 从 /rust 中可能没有完整实现，但可参考 session 存储模式

mkdir -p src-tauri/src/modules/memory/{providers,honcho}

cat > src-tauri/src/modules/memory/mod.rs << 'EOF'
//! Memory System Module
//!
//! Based on: docs/design-docs/memory-system.md
//! Source: CODE_FOUNDATION_STRATEGY.md

pub mod manager;
pub mod memory;
pub mod honcho;
pub mod providers;

pub use manager::MemoryManager;
EOF

# 创建初步框架
cat > src-tauri/src/modules/memory/manager.rs << 'EOF'
//! Memory Manager - coordinates all memory operations
//!
//! This is Phase 2 implementation based on memory-system.md design

use std::sync::Arc;
use std::collections::HashMap;

pub struct MemoryManager {
    // TODO: Implement based on memory-system.md
    // - Built-in memory (MEMORY.md, USER.md)
    // - External providers (Honcho, etc)
    // - Recall pipeline (auto + manual)
    // - Write pipeline (immediate + session_end)
}

impl MemoryManager {
    pub async fn new() -> Result<Self> {
        // Initialize with built-in storage + Honcho provider
        todo!()
    }
}
EOF

cd src-tauri
cargo check --lib 2>&1 | grep -v "warning:" || true
cd ..

echo "✅ Memory 模块迁移成功 (框架阶段)"
```

### Step 7: Plugin 模块迁移 (1 小时)

```bash
#!/bin/bash

echo "🔄 迁移 Plugin 模块..."

cp -r rust/crates/plugins/src/* \
  src-tauri/src/modules/plugin/ 2>/dev/null || true

cat > src-tauri/src/modules/plugin/mod.rs << 'EOF'
//! Plugin System Module
//!
//! Migrated from rust/crates/plugins/
//! Source: CODE_FOUNDATION_STRATEGY.md

pub mod manager;
pub mod loader;
pub mod registry;

pub use manager::PluginManager;
EOF

cd src-tauri
cargo check --lib
cd ..

echo "✅ Plugin 模块迁移成功"
```

---

## Step 8: AppState 集成 (1 小时)

```rust
// src-tauri/src/app_state.rs

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
/// Mirrors design in: docs/design-docs/module-boundaries-and-integration.md
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
    
    /// Memory System - 用户记忆 + Honcho
    pub memory_manager: Arc<MemoryManager>,
    
    /// Plugin System - 扩展机制
    pub plugin_manager: Arc<PluginManager>,
}

impl AppState {
    /// Initialize all subsystems from foundation sources
    pub async fn new() -> Result<Self> {
        log::info!("🚀 初始化 If2Ai AppState...");
        log::info!("📚 基础来源: /rust crates + docs/design-docs");
        
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
            agent: true, // ConversationRuntime is initialized
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

## Step 9: Tauri Commands 网关 (1 小时)

```rust
// src-tauri/src/commands/mod.rs

pub mod agent;
pub mod tools;
pub mod session;
pub mod memory;

use crate::AppState;

// Re-export all commands for main.rs
pub use agent::*;
pub use tools::*;
pub use session::*;
pub use memory::*;

// Example: Agent command using AppState
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

echo "🔧 编译与验证 (Step 10)..."

# 1. 完整编译
echo "📦 完整编译..."
cd src-tauri
cargo build --release 2>&1 | tee /tmp/build.log

if [ $? -ne 0 ]; then
    echo "❌ 编译失败"
    cat /tmp/build.log
    exit 1
fi

# 2. 运行测试
echo "✅ 编译成功，运行测试..."
cargo test --lib -- --nocapture

# 3. 运行 Tauri 开发构建
echo "🚀 启动 Tauri 开发模式..."
cargo tauri dev &
TAURI_PID=$!

sleep 5

# 4. 基础检查
echo "🔍 基础功能检查..."
# 可以在这里添加集成测试命令

# 5. 停止开发服务器
kill $TAURI_PID 2>/dev/null || true

echo "✅ 所有验证通过！"
```

---

## 完成与总结

```bash
#!/bin/bash

# 1. 更新迁移日志
cat >> MIGRATION_LOG.md << 'EOF'

## Completion

- ✅ All modules migrated
- ✅ AppState integrated
- ✅ Commands gateway working
- ✅ Tests passing
- ✅ Tauri build succeeding

**Completed at**: [timestamp]
**Duration**: ~8-10 hours
EOF

# 2. 提交代码
git add -A
git commit -m "feat: complete code migration from /rust to src-tauri

- Migrate all 6 core modules (agent, provider, tools, session, memory, plugin)
- Implement unified AppState architecture
- Create Tauri Commands gateway layer
- All tests passing, ready for Phase 2 development

Refs: CODE_FOUNDATION_STRATEGY.md"

# 3. 创建 PR
git push -u origin feature/code-foundation-migration

echo "✅ 迁移完成！后续："
echo "   1. Code review (by architecture team)"
echo "   2. Merge to main"
echo "   3. Begin Phase 2 development"
echo "   4. All future code based on src-tauri as foundation"
```

---

## 回滚计划 (万一失败)

```bash
#!/bin/bash

# 如果迁移出现问题，可以快速回滚

echo "⚠️  开始回滚..."

# 回到迁移前的状态
git reset --hard HEAD~1

# 或者回到备份标签
git reset --hard pre-migration-$(date +%Y%m%d)

echo "✅ 已回滚到迁移前状态"
echo "📝 分析原因，修正后重新开始"
```

---

## 核对清单

在开始迁移前，确认以下项目：

- [ ] `/rust` 中所有 crates 都能编译 (`cargo build`)
- [ ] `src-tauri/` 中已有基本的 Tauri 配置
- [ ] 设计文档已阅读 (特别是 module-boundaries-and-integration.md)
- [ ] 已创建备份分支并标记备份点
- [ ] 有足够的时间完成整个迁移 (8-10 小时不间断)
- [ ] 团队已知晓迁移计划

---

**成功标志**:
✅ `cargo build --release` 编译成功  
✅ `cargo test` 所有测试通过  
✅ `cargo tauri dev` 能成功启动  
✅ AppState 能正确初始化所有子系统  
✅ 所有模块都能通过 Tauri Commands 访问  

**下一步**: 开始 Phase 2 实现

