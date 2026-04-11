# If2Ai Harness 框架

**Harness** 是用来"跑、测、评估、对比、复现" Agent 行为的一整套测试与执行框架。

这个实现参考了 OpenAI 的 harness-engineering 最佳实践。

## 📚 目录概览

```
harness/
├── README.md               # 本文件
├── runner.py               # 核心运行器
├── evaluators/             # 评估器实现
│   ├── base.py             # 基础评估器类
│   ├── behavior.py         # 行为评估
│   ├── correctness.py      # 正确性评估
│   └── performance.py      # 性能评估
├── fixtures/               # 测试夹具
│   ├── __init__.py
│   ├── agents.py           # Agent 夹具
│   ├── tools.py            # Tool 夹具
│   └── llm.py              # LLM Mock 夹具
└── runners/                # 运行器实现
    ├── __init__.py
    ├── local.py            # 本地运行器
    └── distributed.py      # 分布式运行器（未来）
```

## 🎯 核心概念

### 1. **Runner** (运行器)

执行 Agent 的容器

```python
runner = LocalRunner(
    agent_config=config,
    llm_provider="openai",
    timeout=30.0
)
result = runner.run(prompt="Do something")
```

### 2. **Evaluator** (评估器)

评估 Agent 行为的工具

```python
evaluator = BehaviorEvaluator()
score = evaluator.evaluate(
    input="Do something",
    output="I did something",
    expected="Did something correctly"
)
```

### 3. **Fixture** (夹具)

可复用的测试数据和 mock

```python
agent_fixture = AgentFixture(
    name="test_agent",
    tools=["search", "calculator"]
)
```

## 🚀 快速开始

### 1. 运行单个测试

```bash
# 使用默认配置运行
python -m harness.runner run \
    --config tests/configs/simple_agent.yaml \
    --prompt "What is 2+2?"

# 使用自定义评估器
python -m harness.runner run \
    --config tests/configs/simple_agent.yaml \
    --prompt "What is 2+2?" \
    --evaluator correctness
```

### 2. 运行测试套件

```bash
# 运行所有测试
python -m harness runner suite --suite default

# 运行特定标签的测试
python -m harness runner suite --tags "math,critical"

# 生成报告
python -m harness runner report --suite default --format html
```

### 3. 对比两个版本

```bash
# 对比 Agent 行为变化
python -m harness runner compare \
    --baseline config-v1.yaml \
    --candidate config-v2.yaml \
    --test-suite regression.yaml
```

### 4. 复现特定行为

```bash
# 使用种子和快照复现完全相同的行为
python -m harness runner reproduce \
    --snapshot snapshots/issue-123.json \
    --seed 42
```

## 🔬 评估器类型

### A. 行为评估 (BehaviorEvaluator)

评估 Agent 是否执行了预期的行为：

```python
evaluator = BehaviorEvaluator(
    tools_used=["search"],           # 预期使用的工具
    tool_calls={"search": 1},        # 预期使用次数
    response_contains="answer",      # 回复关键词
)

score = evaluator.evaluate(
    execution_trace=trace,
    output=output
)
```

### B. 正确性评估 (CorrectnessEvaluator)

评估 Agent 的输出是否正确：

```python
evaluator = CorrectnessEvaluator(
    expected_output="Paris",
    match_strategy="semantic",  # 精确匹配或语义匹配
)

results = evaluator.evaluate(
    output="The capital of France is Paris.",
)
```

### C. 性能评估 (PerformanceEvaluator)

评估效率指标：

```python
evaluator = PerformanceEvaluator(
    max_tokens=500,
    max_duration=5.0,
    max_tool_calls=3,
)

score = evaluator.evaluate(
    tokens_used=150,
    duration=2.3,
    tool_calls=2,
)
```

### D. 自定义评估器

继承 `BaseEvaluator` 创建自定义评估器：

```python
from harness.evaluators.base import BaseEvaluator

class MyEvaluator(BaseEvaluator):
    def evaluate(self, **kwargs) -> EvaluationResult:
        # 实现你的评估逻辑
        return EvaluationResult(
            score=0.95,
            details={"reason": "Good performance"}
        )
```

## 📊 执行跟踪和日志

每次运行都会生成详细的执行跟踪：

```json
{
  "run_id": "run-2026-04-11-001",
  "timestamp": "2026-04-11T10:30:00Z",
  "config": {...},
  "events": [
    {
      "type": "prompt_sent",
      "timestamp": "2026-04-11T10:30:01Z",
      "prompt": "What is 2+2?",
      "tokens": 5
    },
    {
      "type": "tool_called",
      "timestamp": "2026-04-11T10:30:02Z",
      "tool": "calculator",
      "input": "2+2",
      "output": "4",
      "tokens": 2
    },
    {
      "type": "response_generated",
      "timestamp": "2026-04-11T10:30:03Z",
      "response": "The answer is 4",
      "tokens": 4
    }
  ],
  "metrics": {
    "total_tokens": 11,
    "duration": 2.5,
    "tool_calls": 1,
    "errors": 0
  },
  "evaluations": {
    "behavior": 0.95,
    "correctness": 1.0,
    "performance": 0.8
  }
}
```

## 🔄 测试用例结构

```yaml
# tests/suites/default.yaml
name: 'Default Test Suite'
description: 'Basic Agent functionality'

fixtures:
  simple_agent:
    type: agent
    config: configs/simple_agent.yaml

  math_tools:
    type: tools
    tools:
      - name: calculator
        spec: specs/calculator.yaml
      - name: search
        spec: specs/search.yaml

test_cases:
  - name: 'Simple Math'
    description: 'Agent can answer basic math'
    agent_fixture: simple_agent
    tool_fixtures:
      - math_tools
    prompt: 'What is 2+2?'
    expected_output: '4'
    evaluators:
      - name: correctness
        config:
          match_strategy: 'semantic'
      - name: behavior
        config:
          tools_used: ['calculator']
    tags: ['math', 'basic']

  - name: 'Multi-step Problem'
    description: 'Agent can solve multi-step problems'
    agent_fixture: simple_agent
    tool_fixtures:
      - math_tools
    prompt: 'If I have 10 apples and you give me 5, how many do I have?'
    expected_output: '15'
    evaluators:
      - name: correctness
      - name: behavior
        config:
          tool_calls: { 'calculator': 1 }
    tags: ['math', 'multi-step']
```

## 📈 报告生成

### HTML 报告

```bash
python -m harness runner report \
    --suite default \
    --format html \
    --output results/report.html
```

生成的报告包含：

- ✅ 测试执行摘要
- ✅ 每个测试的详细结果
- ✅ 评估分数对比
- ✅ 性能指标
- ✅ 错误和失败分析

### JSON 报告

```bash
python -m harness runner report \
    --suite default \
    --format json \
    --output results/report.json
```

便于programmatic 处理和 CI/CD 集成。

## 🔍 故障排查和调试

### 1. 启用详细日志

```bash
python -m harness runner run \
    --config test.yaml \
    --log-level DEBUG \
    --output-dir debug/
```

### 2. 保存执行快照

```bash
python -m harness runner run \
    --config test.yaml \
    --save-snapshot snapshots/debug.json \
    --save-artifacts artifacts/
```

### 3. 使用交互式调试器

```bash
python -m harness runner debug \
    --config test.yaml \
    --prompt "test prompt" \
    --interactive
```

## 🎭 Mock 和夹具

### Mock LLM 提供商

用于确定性测试：

```python
from harness.fixtures.llm import MockLLMProvider

mock_llm = MockLLMProvider(
    responses={
        "What is 2+2?": "The answer is 4",
        "What is the capital of France?": "Paris",
    }
)

runner = LocalRunner(
    agent_config=config,
    llm_provider=mock_llm,
)
```

### Mock 工具

用于隔离 Agent 逻辑：

```python
from harness.fixtures.tools import MockTool

mock_tool = MockTool(
    name="calculator",
    outputs={
        "2+2": "4",
        "3*4": "12",
    }
)
```

## 📋 最佳实践

### 1. 隔离测试

- 每个测试应该独立运行
- 使用 fixtures 隔离外部依赖
- 清理每个测试的状态

### 2. 确定性结果

- 使用种子值确保可复现性
- Mock 非确定性操作（如 API 调用）
- 避免依赖时间或随机数

### 3. 清晰的断言

```python
# ✅ GOOD - 清晰的评估目标
evaluator = CorrectnessEvaluator(
    expected_output="42",
    match_strategy="exact"
)

# ❌ BAD - 模糊的评估
# "somehow check if output is right"
```

### 4. 定期运行

- 在每个 PR 上运行完整测试套件
- 保持测试夹具和 Mock 最新
- 定期审查和更新测试

## 🛠️ 工具和集成

### VS Code 集成

推荐安装 [Harness Runner](vscode-extension-link)：

- 按一键运行测试
- 内联查看结果
- 快速查看执行跟踪

### CI/CD 集成

在 GitHub Actions 中集成：

```yaml
- name: Run Harness Tests
  run: python -m harness runner suite --suite default --ci-mode
```

### 性能基准

```bash
python -m harness benchmark \
    --config test.yaml \
    --iterations 100 \
    --compare baseline.json
```

## 📚 进阶主题

### 自定义评估器

见 [evaluators/custom-example.md](./evaluators/custom-example.md)

### 分布式运行

见 [runners/distributed.md](./runners/distributed.md)（未来功能）

### Agent 对比分析

见 [analysis/](./analysis/)

## 🚨 常见问题

**Q: 为什么测试失败了，但看起来本应通过？**
A: 检查：

1. LLM 响应是否确定（使用 Mock）
2. Tool Mock 是否与实际行为一致
3. 评估器阈值是否太严格

**Q: 如何加速测试？**
A:

1. 使用 Mock LLM 和工具
2. 减少评估器数量
3. 并行运行独立测试（coming soon）

**Q: 如何在自己的项目中使用 Harness？**
A: 详见 [examples/](./examples/) 目录

## 📞 支持和反馈

- 发现 Bug？创建 Issue
- 有改进建议？创建 Discussion
- 想贡献？提交 PR

---

**版本**: 0.1.0 | **最后更新**: 2026-04-11

相关文档：

- [DESIGN.md](../DESIGN.md) - 设计原则
- [ARCHITECTURE.md](../ARCHITECTURE.md) - 系统架构
- [docs/design-docs/testing-strategy.md](../docs/design-docs/testing-strategy.md) - 测试策略
