# Harness Testing Framework - 完整流程设计

> Harness Framework 是 If2Ai 项目的核心测试和评估框架。这个文档定义了完整的测试流程、评估指标和 CI 集成。基于 OpenAI harness-engineering 文章的最佳实践。

## Harness 核心概念

```
测试数据 (Cases)
    ↓
┌─────────────────────────┐
│  Runner                 │ ← 执行 Agent
│  (Local/Docker/Remote)  │
└────────┬────────────────┘
         ↓
    Agent 执行
    (消息、工具调用、指标)
         ↓
┌─────────────────────────┐
│  Evaluators             │ ← 多维度评估
│  (Correctness/Behavior) │
└────────┬────────────────┘
         ↓
    评估结果 (Scores)
         ↓
┌─────────────────────────┐
│  Comparison             │ ← 版本比较
│  (A/B Testing)          │
└────────┬────────────────┘
         ↓
    报告和分析
```

## 1. Test Cases（测试用例）

```rust
pub struct TestCase {
    pub id: String,
    pub name: String,
    pub description: String,

    // 输入
    pub prompt: String,
    pub user_message: Option<String>,
    pub conversation_history: Option<Vec<Message>>,

    // 配置
    pub agent_config: AgentConfig,
    pub tool_allowlist: Option<Vec<String>>,    // 限制使用的工具
    pub expected_tools: Option<Vec<String>>,   // 期望使用的工具

    // 预期结果
    pub expected_output: Option<String>,
    pub expected_concepts: Option<Vec<String>>,  // 期望包含的概念
    pub should_contain: Option<Vec<String>>,
    pub should_not_contain: Option<Vec<String>>,

    // 约束
    pub max_iterations: Option<u32>,
    pub max_tokens: Option<u32>,
    pub timeout_secs: u32,

    // 标签
    pub tags: Vec<String>,                      // "research", "coding", "analysis"
    pub difficulty: String,                     // "easy", "medium", "hard"
}

pub struct TestSuite {
    pub id: String,
    pub name: String,
    pub description: String,
    pub test_cases: Vec<TestCase>,
    pub runs_in_parallel: bool,
    pub target_pass_rate: f32,                  // 如 0.95 (95%)
}

// 预定义的测试套件
pub fn create_foundational_suite() -> TestSuite {
    TestSuite {
        id: "foundational-suite".to_string(),
        name: "Foundational Agent Capabilities",
        test_cases: vec![
            // 信息收集
            TestCase {
                name: "Simple Web Search".to_string(),
                prompt: "What is the capital of France?".to_string(),
                expected_tools: Some(vec!["web_search".to_string()]),
                tags: vec!["research".to_string()],
                ..Default::default()
            },

            // 文件处理
            TestCase {
                name: "File Reading".to_string(),
                prompt: "Read and summarize README.md".to_string(),
                expected_tools: Some(vec!["read_file".to_string()]),
                tags: vec!["file_handling".to_string()],
                ..Default::default()
            },

            // 代码执行
            TestCase {
                name: "Code Execution".to_string(),
                prompt: "Write and execute Python code to calculate fibonacci".to_string(),
                expected_tools: Some(vec!["execute_code".to_string()]),
                tags: vec!["coding".to_string()],
                ..Default::default()
            },

            // 多步骤任务
            TestCase {
                name: "Multi-step Task".to_string(),
                prompt: "Search for latest AI news, then write a summary in file".to_string(),
                expected_tools: Some(vec!["web_search".to_string(), "write_file".to_string()]),
                tags: vec!["research", "file_handling".to_string()],
                ..Default::default()
            },
        ],
        target_pass_rate: 0.95,
        ..Default::default()
    }
}
```

## 2. Runners（执行器）

```rust
pub trait Runner: Send + Sync {
    async fn run(&self, test_case: &TestCase) -> Result<ExecutionResult>;
}

pub struct LocalRunner {
    agent_orchestrator: Arc<OrchestratorState>,
    environment: RuntimeEnvironment,
}

pub struct ExecutionResult {
    pub test_case_id: String,

    // 执行信息
    pub final_output: String,
    pub messages: Vec<Message>,
    pub tool_calls: Vec<ToolCallRecord>,

    // 元数据
    pub total_iterations: u32,
    pub total_tokens: u32,
    pub duration_secs: f32,
    pub error: Option<String>,

    // 性能
    pub tool_execution_times: HashMap<String, u32>,  // ms
    pub llm_calls_count: u32,

    // 时间戳
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ToolCallRecord {
    pub tool_name: String,
    pub arguments: JsonValue,
    pub result: String,
    pub execution_time_ms: u32,
    pub status: String,                         // "success", "error"
}

impl LocalRunner {
    pub async fn run(&self, test_case: &TestCase) -> Result<ExecutionResult> {
        let start_time = Utc::now();

        // Phase 1: 设置代理配置
        let mut agent_state = self.prepare_agent(&test_case.agent_config)?;

        // Phase 2: 限制工具（如果指定）
        if let Some(allowlist) = &test_case.tool_allowlist {
            agent_state.restrict_tools(allowlist);
        }

        // Phase 3: 执行
        let result = timeout(
            Duration::from_secs(test_case.timeout_secs as u64),
            agent_state.run_conversation(&test_case.prompt, None)
        ).await??;

        // Phase 4: 收集执行结果
        Ok(ExecutionResult {
            test_case_id: test_case.id.clone(),
            final_output: result.messages.last().map(|m| m.content.clone()).unwrap_or_default(),
            messages: result.messages,
            tool_calls: extract_tool_calls(&result),
            total_iterations: result.metrics.iterations,
            total_tokens: result.metrics.tokens,
            duration_secs: (Utc::now() - start_time).num_seconds() as f32 / 1000.0,
            started_at: start_time,
            completed_at: Utc::now(),
            ..Default::default()
        })
    }
}

pub struct DockerRunner {
    image: String,
    network: String,
}

impl Runner for DockerRunner {
    async fn run(&self, test_case: &TestCase) -> Result<ExecutionResult> {
        // 在 Docker 容器中运行 Agent
        // 1. Spin up 容器
        // 2. 传递测试用例
        // 3. 收集输出
    }
}

pub struct RemoteRunner {
    endpoint: String,
    api_key: String,
}

impl Runner for RemoteRunner {
    async fn run(&self, test_case: &TestCase) -> Result<ExecutionResult> {
        // 调用远程 API 服务
    }
}
```

## 3. Evaluators（评估器）

```rust
pub trait Evaluator: Send + Sync {
    async fn evaluate(&self, result: &ExecutionResult) -> Result<EvaluationScore>;
    fn name(&self) -> &str;
}

pub struct EvaluationScore {
    pub evaluator_name: String,
    pub dimension: String,                      // "correctness", "behavior", etc.
    pub score: f32,                             // 0.0 - 1.0
    pub confidence: f32,                        // 0.0 - 1.0
    pub message: String,
    pub details: JsonValue,                     // 细节数据
}

// === 评估器 1: 正确性评估 ===
pub struct CorrectnessEvaluator {
    llm_scorer: Arc<dyn LLMClient>,
}

impl Evaluator for CorrectnessEvaluator {
    async fn evaluate(&self, result: &ExecutionResult) -> Result<EvaluationScore> {
        // 使用 LLM 评估输出是否正确
        let scoring_prompt = format!(
            "Evaluate the correctness of this output:\n{}",
            result.final_output
        );

        let response = self.llm_scorer.complete(
            &[Message::user(scoring_prompt)],
            &[],
            false,
        ).await?;

        let score = self.parse_score(&response.content)?;

        Ok(EvaluationScore {
            evaluator_name: "correctness".to_string(),
            dimension: "output_quality".to_string(),
            score,
            confidence: 0.85,
            message: "Evaluated output correctness".to_string(),
            details: json!({}),
        })
    }
}

// === 评估器 2: 行为评估 ===
pub struct BehaviorEvaluator {
    config: BehaviorConfig,
}

pub struct BehaviorConfig {
    pub expected_tools: Option<Vec<String>>,
    pub disallowed_tools: Option<Vec<String>>,
    pub expected_iterations: Option<(u32, u32)>,  // (min, max)
    pub expected_tokens: Option<(u32, u32)>,
}

impl Evaluator for BehaviorEvaluator {
    async fn evaluate(&self, result: &ExecutionResult) -> Result<EvaluationScore> {
        let mut score = 1.0;
        let mut issues = Vec::new();

        // 检查工具使用
        let used_tools: HashSet<String> = result.tool_calls
            .iter()
            .map(|c| c.tool_name.clone())
            .collect();

        if let Some(expected) = &self.config.expected_tools {
            let expected_set: HashSet<String> = expected.iter().cloned().collect();
            if !expected_set.is_subset(&used_tools) {
                let missing: Vec<_> = expected_set.difference(&used_tools).cloned().collect();
                score -= 0.2;
                issues.push(format!("Missing expected tools: {:?}", missing));
            }
        }

        if let Some(disallowed) = &self.config.disallowed_tools {
            let disallowed_set: HashSet<String> = disallowed.iter().cloned().collect();
            if !used_tools.is_disjoint(&disallowed_set) {
                let found: Vec<_> = used_tools.intersection(&disallowed_set).cloned().collect();
                score -= 0.3;
                issues.push(format!("Used disallowed tools: {:?}", found));
            }
        }

        // 检查迭代计数
        if let Some((min, max)) = self.config.expected_iterations {
            if result.total_iterations < min || result.total_iterations > max {
                score -= 0.1 * ((result.total_iterations as f32 - min as f32).abs() / max as f32);
                issues.push(format!(
                    "Iteration count {} outside expected range [{}, {}]",
                    result.total_iterations, min, max
                ));
            }
        }

        score = score.max(0.0);

        Ok(EvaluationScore {
            evaluator_name: "behavior".to_string(),
            dimension: "execution_behavior".to_string(),
            score,
            confidence: 0.95,
            message: issues.join("; "),
            details: json!({
                "tools_used": Vec::from_iter(used_tools),
                "total_iterations": result.total_iterations,
                "issues": issues,
            }),
        })
    }
}

// === 评估器 3: 性能评估 ===
pub struct PerformanceEvaluator {
    config: PerformanceConfig,
}

pub struct PerformanceConfig {
    pub max_duration_secs: Option<f32>,
    pub max_tokens: Option<u32>,
    pub expected_tool_latency: Option<HashMap<String, u32>>,  // ms
}

impl Evaluator for PerformanceEvaluator {
    async fn evaluate(&self, result: &ExecutionResult) -> Result<EvaluationScore> {
        let mut score = 1.0;
        let mut issues = Vec::new();

        if let Some(max_dur) = self.config.max_duration_secs {
            if result.duration_secs > max_dur {
                score -= 0.2 * (result.duration_secs / max_dur - 1.0).min(0.5);
                issues.push(format!("Execution time {} > max {}", result.duration_secs, max_dur));
            }
        }

        if let Some(max_tokens) = self.config.max_tokens {
            if result.total_tokens > max_tokens {
                score -= 0.15 * ((result.total_tokens as f32 / max_tokens as f32 - 1.0).min(0.5));
                issues.push(format!("Token usage {} > max {}", result.total_tokens, max_tokens));
            }
        }

        score = score.max(0.0);

        Ok(EvaluationScore {
            evaluator_name: "performance".to_string(),
            dimension: "execution_efficiency".to_string(),
            score,
            confidence: 0.9,
            message: issues.join("; "),
            details: json!({
                "duration_secs": result.duration_secs,
                "total_tokens": result.total_tokens,
            }),
        })
    }
}

// === 评估器 4: 可靠性评估 ===
pub struct ReliabilityEvaluator;

impl Evaluator for ReliabilityEvaluator {
    async fn evaluate(&self, result: &ExecutionResult) -> Result<EvaluationScore> {
        let score = if result.error.is_none() { 1.0 } else { 0.0 };

        Ok(EvaluationScore {
            evaluator_name: "reliability".to_string(),
            dimension: "execution_reliability".to_string(),
            score,
            confidence: 1.0,
            message: result.error.as_ref().cloned().unwrap_or_default(),
            details: json!({}),
        })
    }
}
```

## 4. Test Suite 执行和报告

```rust
pub struct TestSuiteRunner {
    runner: Arc<dyn Runner>,
    evaluators: Vec<Arc<dyn Evaluator>>,
}

pub struct TestResultSuite {
    pub suite_id: String,
    pub results: Vec<TestResult>,
    pub summary: TestSuiteSummary,
}

pub struct TestResult {
    pub test_case_id: String,
    pub execution_result: ExecutionResult,
    pub evaluation_scores: Vec<EvaluationScore>,
    pub overall_score: f32,
    pub passed: bool,
}

pub struct TestSuiteSummary {
    pub total_tests: usize,
    pub passed: usize,
    pub failed: usize,
    pub average_score: f32,
    pub score_by_dimension: HashMap<String, f32>,
    pub execution_time_total_secs: f32,
}

impl TestSuiteRunner {
    pub async fn run_suite(
        &self,
        suite: &TestSuite,
    ) -> Result<TestResultSuite> {
        let start_time = Utc::now();
        let mut results = Vec::new();

        // 并行或顺序执行测试用例
        for test_case in &suite.test_cases {
            // 执行
            let execution_result = self.runner.run(test_case).await?;

            // 评估
            let mut evaluation_scores = Vec::new();
            for evaluator in &self.evaluators {
                let score = evaluator.evaluate(&execution_result).await?;
                evaluation_scores.push(score);
            }

            // 汇总
            let overall_score = evaluation_scores
                .iter()
                .map(|s| s.score)
                .sum::<f32>() / evaluation_scores.len() as f32;

            let passed = overall_score >= 0.7;  // 70% 通过阈值

            results.push(TestResult {
                test_case_id: test_case.id.clone(),
                execution_result,
                evaluation_scores,
                overall_score,
                passed,
            });
        }

        // 生成摘要
        let passed_count = results.iter().filter(|r| r.passed).count();
        let failed_count = results.len() - passed_count;
        let average_score = results.iter().map(|r| r.overall_score).sum::<f32>() / results.len() as f32;

        let mut score_by_dimension = HashMap::new();
        // ... 计算维度平均分

        let duration = (Utc::now() - start_time).num_seconds() as f32 / 1000.0;

        Ok(TestResultSuite {
            suite_id: suite.id.clone(),
            results,
            summary: TestSuiteSummary {
                total_tests: suite.test_cases.len(),
                passed: passed_count,
                failed: failed_count,
                average_score,
                score_by_dimension,
                execution_time_total_secs: duration,
            },
        })
    }
}
```

## 5. A/B Testing & Comparison

```rust
pub struct ComparisonExperiment {
    pub id: String,
    pub name: String,
    pub baseline_version: String,
    pub candidate_version: String,
    pub test_suite_id: String,
}

pub struct ComparisonResult {
    pub experiment_id: String,
    pub baseline_results: TestResultSuite,
    pub candidate_results: TestResultSuite,
    pub winner: String,                         // "baseline", "candidate", "tie"
    pub confidence: f32,
    pub analysis: ComparisonAnalysis,
}

pub struct ComparisonAnalysis {
    pub score_improvement: f32,                 // % improvement
    pub statistical_significance: f32,          // p-value
    pub dimension_winners: HashMap<String, String>,
}

pub async fn run_comparison(
    experiment: &ComparisonExperiment,
) -> Result<ComparisonResult> {
    // 并行运行两个版本
    let (baseline, candidate) = tokio::join!(
        run_version(&experiment.baseline_version, &experiment.test_suite_id),
        run_version(&experiment.candidate_version, &experiment.test_suite_id),
    );

    let baseline_results = baseline?;
    let candidate_results = candidate?;

    // 计算统计显著性
    let improvement = (candidate_results.summary.average_score - baseline_results.summary.average_score)
        / baseline_results.summary.average_score;

    let winner = if improvement > 0.05 {
        "candidate"
    } else if improvement < -0.05 {
        "baseline"
    } else {
        "tie"
    };

    Ok(ComparisonResult {
        experiment_id: experiment.id.clone(),
        baseline_results,
        candidate_results,
        winner: winner.to_string(),
        confidence: 0.95,
        analysis: ComparisionAnalysis {
            score_improvement: improvement * 100.0,
            ..Default::default()
        },
    })
}
```

## 6. 报告生成

```rust
pub struct Reporter;

impl Reporter {
    pub fn generate_html_report(result: &TestResultSuite) -> String {
        format!(
            r#"<!DOCTYPE html>
<html>
<head>
  <title>Test Report - {}</title>
  <style>
    /* CSS 样式 */
  </style>
</head>
<body>
  <h1>Test Suite Report</h1>
  <div class="summary">
    <h2>Summary</h2>
    <p>Total: {} | Passed: {} | Failed: {}</p>
    <p>Average Score: {:.2}%</p>
  </div>
  <div class="results">
    <h2>Detailed Results</h2>
    <!-- 详细结果表格 -->
  </div>
</body>
</html>"#,
            result.suite_id,
            result.summary.total_tests,
            result.summary.passed,
            result.summary.failed,
            result.summary.average_score * 100.0
        )
    }

    pub fn generate_json_report(result: &TestResultSuite) -> String {
        serde_json::to_string_pretty(result).unwrap()
    }
}
```

## 7. CI/CD 集成

```yaml
# .github/workflows/harness-test.yml
name: Harness Testing

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

jobs:
  harness-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Cache cargo registry
        uses: actions/cache@v3
        with:
          path: ~/.cargo/registry
          key: ${{ runner.os }}-cargo-registry-${{ hashFiles('**/Cargo.lock') }}

      - name: Run Harness Tests
        run: |
          cargo test --test harness_integration
          python3 harness/runner.py --suite foundational --output junit.xml

      - name: Upload Test Results
        if: always()
        uses: actions/upload-artifact@v3
        with:
          name: harness-results
          path: |
            junit.xml
            test-report.html

      - name: Comment PR with Results
        if: github.event_name == 'pull_request'
        uses: actions/github-script@v6
        with:
          script: |
            // 从结果中提取数据，评论 PR
            const fs = require('fs');
            const report = JSON.parse(fs.readFileSync('harness-results.json', 'utf8'));
            github.rest.issues.createComment({
              issue_number: context.issue.number,
              owner: context.repo.owner,
              repo: context.repo.repo,
              body: `## Harness Test Results\n\n| Metric | Value |\n|--------|-------|\n| Tests | ${report.total_tests} |\n| Passed | ${report.passed} |\n| Score | ${(report.average_score * 100).toFixed(2)}% |`
            });
```

---

**版本**: 0.1.0 | **最后更新**: 2026-04-11  
**参考**: [harness/README.md](../../harness/README.md), [docs/design-docs/testing-strategy.md](testing-strategy.md)
