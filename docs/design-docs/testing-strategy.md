# If2Ai 测试策略 (Testing Strategy Design)

> 本文档定义 If2Ai 项目的完整测试策略，包括单元测试、集成测试、E2E 测试和 Harness 评估框架。

## 🎯 测试哲学

### 核心原则

1. **可测试性优于覆盖率**
   - 好的测试设计比高覆盖率数字更重要
   - 无法测试的代码表示设计有问题

2. **确定性优于速度**
   - 所有测试必须可靠地通过或失败
   - 不允许"偶尔"失败的测试

3. **Agent 行为可评估**
   - 测试不仅验证代码，还评估 Agent 行为质量
   - 使用 Harness 框架进行行为评估

4. **金字塔策略**
   ```
       E2E Tests (10%)
      /          \
     集成测试     (30%)
     /          \
   单元测试     (60%)
   ```

## 📊 测试分类

### 1. 单元测试 (Unit Tests)

**范围**: 单个模块/函数  
**工具**: `pytest` + `pytest-asyncio`  
**覆盖率目标**: ≥ 80%

#### 测试位置
```
src-tauri/src/
├── modules/
│   ├── agents/
│   │   ├── orchestrator.rs
│   │   └── orchestrator_tests.rs  ← 单元测试
│   ├── tools/
│   │   ├── registry.rs
│   │   └── registry_tests.rs
```

#### 单元测试涵盖
- ✅ 正常路径（Happy path）
- ✅ 边界情况（Boundary cases）
- ✅ 错误处理（Error cases）
- ✅ 约束验证（Constraint validation）

#### 例子
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_init() {
        let agent = Agent::new("test");
        assert_eq!(agent.name(), "test");
        assert!(agent.is_idle());
    }

    #[tokio::test]
    async fn test_llm_call_success() {
        let mut mock_llm = MockLLM::new();
        mock_llm.expect_complete()
            .returning(|_| Ok("response".to_string()));
        
        let agent = Agent::with_llm(mock_llm);
        let result = agent.run("test").await;
        assert!(result.is_ok());
    }

    #[test]
    #[should_panic(expected = "invalid config")]
    fn test_invalid_config() {
        let _agent = Agent::new_with_config(invalid_config());
    }
}
```

### 2. 集成测试 (Integration Tests)

**范围**: 跨模块交互（不包含外部 API）  
**工具**: `pytest`  
**覆盖率目标**: 关键路径 100%

#### 测试位置
```
tests/
├── integration/
│   ├── agent_tool_interaction.py
│   ├── prompt_building.py
│   ├── context_compression.py
├── fixtures/
│   ├── mocks.py
│   ├── factories.py
└── conftest.py
```

#### 集成测试涵盖
- ✅ Agent 和 Tool System 的交互
- ✅ Prompt Builder 和 Context Compressor
- ✅ LLM Router 的故障转移
- ✅ Memory System 的持久化

#### 例子
```python
# tests/integration/test_agent_with_tools.py

@pytest.fixture
def mock_tools():
    return {
        'calculator': MockCalculator(),
        'search': MockSearch(),
    }

@pytest.fixture
def agent(mock_tools):
    config = AgentConfig(tools=mock_tools)
    return Agent(config)

@pytest.mark.asyncio
async def test_agent_uses_tool(agent, mock_tools):
    """Agent should invoke calculator tool when needed"""
    result = await agent.run("What is 2+2?")
    
    assert mock_tools['calculator'].called
    assert "4" in result.response
    assert result.metrics.tool_calls == 1
```

### 3. E2E 测试 (End-to-End Tests)

**范围**: 完整用户工作流（包含实际 LLM 调用）  
**工具**: `pytest` + `playwright` （UI 测试）  
**覆盖率目标**: 关键用户路径（5-10 个）

#### 测试位置
```
tests/e2e/
├── chat_workflow.py
├── agent_lifecycle.py
├── tool_execution.py
└── ui/
    ├── chat_interface.spec.ts
    └── settings.spec.ts
```

#### E2E 测试涵盖
- ✅ 用户启动对话到得到答案
- ✅ 工具调用和结果处理
- ✅ 错误处理和恢复
- ✅ UI 交互和响应

#### 例子
```python
# tests/e2e/test_chat_workflow.py

@pytest.mark.e2e
class TestChatWorkflow:
    async def test_user_can_get_math_answer(self, browser, server):
        """Complete workflow: user asks math question → gets answer"""
        # 1. 加载应用
        page = await browser.new_page()
        await page.goto(f"http://localhost:{server.port}")
        
        # 2. 输入问题
        input_field = page.locator('input[placeholder="Ask me anything"]')
        await input_field.fill("What is the capital of France?")
        
        # 3. 提交
        submit_btn = page.locator('button:has-text("Send")')
        await submit_btn.click()
        
        # 4. 等待响应
        response = page.locator('text=Paris')
        await response.wait_for()
        
        # 5. 验证
        assert await response.is_visible()
```

### 4. Harness 评估 (Harness Evaluation)

**范围**: Agent 行为评估和对比  
**工具**: 自定义 Harness 框架  
**目标**: 评估 Agent 质量指标

#### 评估维度

| 维度 | 评估器 | 指标 |
|------|-------|------|
| 正确性 | CorrectnessEvaluator | 输出正确性 (0-1) |
| 效率 | PerformanceEvaluator | Token 使用、时间、工具调用数 |
| 行为 | BehaviorEvaluator | 工具使用模式、决策路径 |
| 可靠性 | ReliabilityEvaluator | 错误恢复率、重试成功率 |

#### Harness 测试位置
```
harness/
├── tests/
│   ├── agent_behavior.yaml
│   ├── tool_execution.yaml
│   ├── llm_routing.yaml
├── fixtures/
│   ├── agents.py
│   ├── tools.py
│   └── llms.py
└── evaluators/
    ├── correctness.py
    ├── performance.py
    └── behavior.py
```

#### 例子
```yaml
# harness/tests/agent_behavior.yaml

name: "Agent Behavior Tests"

test_cases:
  - name: "Math Problem"
    prompt: "What is 2+2?"
    expected_output: "4"
    evaluators:
      - type: correctness
        config:
          match_strategy: semantic
      - type: performance
        config:
          max_tokens: 100
          max_duration: 5.0
    assertions:
      - output_contains: "4"
      - tools_used: ["calculator"]
      - duration_lt: 5.0
```

## 🔍 覆盖率目标

### Rust 后端

| 模块 | 目标 | 说明 |
|------|------|------|
| Agent Orchestrator | ≥ 85% | 核心逻辑 |
| Tool System | ≥ 85% | 工具注册和执行 |
| LLM Router | ≥ 75% | 提供商切换（模拟外部调用） |
| Context Compressor | ≥ 90% | 算法验证 |
| Memory System | ≥ 80% | 存储和检索 |
| Prompt Builder | ≥ 80% | 提示词构建 |
| Commands | ≥ 70% | Tauri 命令处理（UI 测试） |

### Svelte 前端

| 模块 | 目标 | 说明 |
|------|------|------|
| 核心组件 | ≥ 80% | Chat, Dashboard 等 |
| 状态管理 | ≥ 85% | Store 逻辑 |
| 工具函数 | ≥ 90% | Utils, Formatters |
| 集成 | UI Tests | 浏览器测试 |

### 整体目标
- **代码覆盖率**: ≥ 80%
- **路径覆盖率**: 关键路径 100%
- **E2E 流程**: 5+ 个主要工作流

## 🚀 测试执行流程

### 本地开发

```bash
# 运行所有单元测试
cargo test --all

# 运行特定模块的测试
cargo test --lib agent::orchestrator

# 运行带覆盖率
cargo tarpaulin --out Html

# 运行集成测试
pytest tests/integration

# 运行 Harness 评估
python -m harness runner suite --suite default
```

### CI/CD 流程

```yaml
# .github/workflows/test.yml

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      
      # 1. 单元测试
      - name: Unit Tests
        run: cargo test --all
      
      # 2. 覆盖率检查
      - name: Code Coverage
        run: cargo tarpaulin --fail-under 80
      
      # 3. 集成测试
      - name: Integration Tests
        run: pytest tests/integration
      
      # 4. Harness 评估
      - name: Harness Evaluation
        run: python -m harness runner suite --ci-mode
      
      # 5. E2E 测试（可选，计划中）
      - name: E2E Tests
        run: |
          npm run build
          pytest tests/e2e --e2e
      
      # 6. 结果上传
      - name: Upload Results
        uses: codecov/codecov-action@v2
```

## 📋 测试清单

### 新功能前
- [ ] 编写单元测试（TDD 风格或实现后）
- [ ] 添加集成测试路径
- [ ] 在 Harness 中创建评估用例
- [ ] 编写功能文档

### PR 前
- [ ] 运行 `cargo test --all`
- [ ] 检查覆盖率 ≥ 80%
- [ ] 运行 `pytest tests/integration`
- [ ] 运行 Harness 测试
- [ ] 本地手动测试（关键路径）

### 合并前
- [ ] CI 通过所有检查
- [ ] 代码审查检查测试质量
- [ ] 无新的警告（clippy）

## 🧪 Mock 和 Fixture 策略

### Mock 原则
- ✅ Mock 外部依赖（LLM、数据库、网络）
- ✅ Mock 不稳定的行为（随机数、时间）
- ❌ 不 Mock 核心业务逻辑
- ❌ 不 Mock 同一模块的其他函数

### Fixture 层次

```python
# 1. 原始数据 Fixtures
@pytest.fixture
def agent_config():
    return AgentConfig(name="test", ...)

# 2. Factory Fixtures
@pytest.fixture
def agent_factory(agent_config):
    return AgentFactory(config=agent_config)

# 3. Mock Fixtures
@pytest.fixture
def mock_llm():
    return MockLLMProvider()

# 4. 完整组装 Fixtures
@pytest.fixture
def ready_agent(agent_factory, mock_llm):
    agent = agent_factory.create(llm=mock_llm)
    return agent
```

## 📊 持续集成和监控

### 测试指标追踪

- 运行时间（目标 < 5 分钟CI 中）
- 覆盖率趋势
- 失败频率和模式
- Harness 评估分数趋势

### 质量门控

```
覆盖率 < 80% → 阻止合并
关键路径未测试 → 阻止合并
>2 个相同失败 → 阻止合并
E2E 失败 → 需人工审查
```

## 🎓 测试最佳实践

### 1. 清晰的测试名称
```rust
// ✅ GOOD
#[test]
fn agent_should_call_calculator_tool_for_math() { ... }

// ❌ BAD
#[test]
fn test1() { ... }
```

### 2. Arrange-Act-Assert 模式
```rust
#[test]
fn test_example() {
    // Arrange - 设置
    let agent = create_test_agent();
    
    // Act - 执行
    let result = agent.run("test");
    
    // Assert - 验证
    assert!(result.is_ok());
}
```

### 3. 独立且可重复
```rust
// ✅ GOOD - 完全独立
#[test]
fn test_calculation() {
    let result = calculate(2, 2);
    assert_eq!(result, 4);
}

// ❌ BAD - 依赖全局状态
static mut GLOBAL_STATE: i32 = 0;
#[test]
fn test_uses_global() {
    GLOBAL_STATE = 2;
    // 结果依赖全局状态...
}
```

### 4. 快速反馈
```bash
# 分层运行 - 快速反馈
cargo test --lib                    # < 1s
pytest tests/integration --fast     # < 10s
python -m harness runner quick      # < 30s
```

## 📚 测试文档

- [Test Fixtures Guide](../fixtures/)
- [Mock Strategy](../mocks/)
- [Assertion Patterns](../patterns.md)
- [Debugging Failed Tests](../debugging.md)

---

**版本**: 0.1.0 | **最后更新**: 2026-04-11

相关文档：
- [DESIGN.md](../../DESIGN.md) - 设计原则
- [harness/README.md](../../harness/README.md) - Harness 框架
- [ARCHITECTURE.md](../../ARCHITECTURE.md) - 系统架构
