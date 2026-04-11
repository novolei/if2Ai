# Prompt Builder 设计文档

> Prompt Builder 负责从系统配置、会话历史和运行时状态动态构建高质量的系统提示。这是 Agent 推理质量的基础。基于 hermes-agent 的模块化提示设计。

## 核心概念

```
系统配置
  ├─ Agent 身份 (角色、技能、背景)
  ├─ 可用工具定义
  └─ 约束和安全规则
        │
        ├─ Agent Instruction Builder
        │
用户指令 → ├─ Tool Definition Builder
        │
会话信息 ├─ Conversation Context Builder
        │
        ├─ Example/Few-shot Builder
        │
        └─ Output Format Specifier
              ↓
         最终提示词
```

## 系统架构

```rust
pub struct PromptBuilder {
    // 核心组件
    agent_definition: AgentDefinition,
    tool_definitions: Vec<ToolDefinition>,
    conversation_context: Vec<Message>,
    examples: Vec<Example>,

    // 配置
    config: PromptConfig,
}

pub struct PromptConfig {
    pub include_examples: bool,
    pub include_agent_id: bool,
    pub tool_format: ToolFormatStyle,       // OpenAI or Claude or Custom
    pub max_context_tokens: Option<u32>,
    pub language: String,                    // "zh", "en"
    pub instruction_template: String,
}

pub enum ToolFormatStyle {
    OpenAI {                                 // {"type": "function", "function": {...}}
        version: String,
    },
    Claude {                                 // {"name": "...", "description": "...", "input_schema": {...}}
        version: String,
    },
    Custom(String),                          // 自定义格式
}
```

## 提示词构建流程

### 1. Agent Definition Builder

```rust
pub struct AgentDefinition {
    pub name: String,                        // "AI Agent", "Code Assistant"
    pub role: String,                        // "You are a helpful AI assistant..."
    pub capabilities: Vec<String>,           // ["Code generation", "Analysis", ...]
    pub tone: String,                        // "professional", "friendly", "formal"
    pub language_preferences: LanguagePrefs,
    pub ethical_guidelines: Vec<String>,
    pub special_instructions: Vec<String>,
}

pub struct LanguagePrefs {
    pub primary_language: String,
    pub support_languages: Vec<String>,
    pub code_comments_language: String,
}

pub struct AgentDefinitionBuilder {
    agent: AgentDefinition,
}

impl AgentDefinitionBuilder {
    pub fn build(&self) -> String {
        let mut prompt = String::new();

        // 部分 1: 基础角色定义
        prompt.push_str(&format!(
            "# Agent Role & Identity\n\n\
             You are {}.\n\n\
             {}\n\n",
            self.agent.name,
            self.agent.role
        ));

        // 部分 2: 能力范围
        if !self.agent.capabilities.is_empty() {
            prompt.push_str("## Capabilities\n\n");
            for cap in &self.agent.capabilities {
                prompt.push_str(&format!("- {}\n", cap));
            }
            prompt.push_str("\n");
        }

        // 部分 3: 沟通风格
        prompt.push_str(&format!(
            "## Communication Style\n\n\
             - Tone: {}\n\
             - Primary Language: {}\n\n",
            self.agent.tone,
            self.agent.language_preferences.primary_language
        ));

        // 部分 4: 道德准则
        if !self.agent.ethical_guidelines.is_empty() {
            prompt.push_str("## Ethical Guidelines\n\n");
            for guideline in &self.agent.ethical_guidelines {
                prompt.push_str(&format!("- {}\n", guideline));
            }
            prompt.push_str("\n");
        }

        // 部分 5: 特殊指令
        if !self.agent.special_instructions.is_empty() {
            prompt.push_str("## Special Instructions\n\n");
            for instr in &self.agent.special_instructions {
                prompt.push_str(&format!("- {}\n", instr));
            }
            prompt.push_str("\n");
        }

        prompt
    }
}
```

### 2. Tool Definition Builder

```rust
pub struct ToolDefinitionBuilder {
    tools: Vec<ToolDefinition>,
    format: ToolFormatStyle,
}

pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: JsonSchema,
    pub required_params: Vec<String>,
    pub examples: Option<Vec<ToolExample>>,
    pub cost_estimate: Option<String>,    // 如 "API 费用", "执行时间"
    pub safety_notes: Option<String>,
}

pub struct ToolExample {
    pub scenario: String,
    pub input: serde_json::Value,
    pub expected_output: String,
}

impl ToolDefinitionBuilder {
    pub fn build(&self) -> String {
        let mut prompt = String::from("# Available Tools\n\n");

        for tool in &self.tools {
            prompt.push_str(&format!("## {}\n\n", tool.name));
            prompt.push_str(&format!("{}\n\n", tool.description));

            // 参数文档
            prompt.push_str("**Parameters:**\n");
            for (param_name, param_schema) in &tool.parameters.properties {
                let required = if tool.required_params.contains(param_name) {
                    "required"
                } else {
                    "optional"
                };
                prompt.push_str(&format!(
                    "- `{}` ({}): {}\n",
                    param_name,
                    param_schema.get("type").unwrap_or(&"any".into()),
                    param_schema.get("description").unwrap_or(&"".into())
                ));
            }
            prompt.push_str("\n");

            // 示例
            if let Some(examples) = &tool.examples {
                prompt.push_str("**Examples:**\n");
                for example in examples {
                    prompt.push_str(&format!(
                        "- Scenario: {}\n  Input: {}\n  Output: {}\n",
                        example.scenario,
                        serde_json::to_string(&example.input).unwrap(),
                        example.expected_output
                    ));
                }
                prompt.push_str("\n");
            }

            // 成本和安全注意
            if let Some(cost) = &tool.cost_estimate {
                prompt.push_str(&format!("*Cost: {}*\n", cost));
            }
            if let Some(safety) = &tool.safety_notes {
                prompt.push_str(&format!("⚠️ *Safety Note: {}*\n", safety));
            }

            prompt.push_str("\n");
        }

        prompt
    }

    pub fn to_openai_format(&self) -> Vec<serde_json::Value> {
        self.tools
            .iter()
            .map(|tool| json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters,
                }
            }))
            .collect()
    }

    pub fn to_claude_format(&self) -> Vec<serde_json::Value> {
        self.tools
            .iter()
            .map(|tool| json!({
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool.parameters,
            }))
            .collect()
    }
}
```

### 3. Conversation Context Builder

```rust
pub struct ConversationContextBuilder {
    messages: Vec<Message>,
    max_tokens: u32,
}

impl ConversationContextBuilder {
    pub fn build(&self) -> (String, u32) {
        let mut prompt = String::from("# Conversation History\n\n");
        let mut tokens_used = 0;

        for msg in &self.messages {
            let msg_text = format!("{}: {}\n\n", msg.role, msg.content);
            let msg_tokens = estimate_tokens(&msg_text);

            if tokens_used + msg_tokens > self.max_tokens {
                prompt.push_str("[... conversation truncated due to token limit ...]\n\n");
                break;
            }

            prompt.push_str(&msg_text);
            tokens_used += msg_tokens;
        }

        (prompt, tokens_used)
    }
}
```

### 4. Few-shot Example Builder

```rust
pub struct ExampleBuilder {
    examples: Vec<Example>,
}

pub struct Example {
    pub title: String,
    pub description: String,
    pub input_prompt: String,
    pub expected_output: String,
    pub tools_used: Vec<String>,
}

impl ExampleBuilder {
    pub fn build(&self) -> String {
        let mut prompt = String::from("# Examples\n\n");

        for (idx, example) in self.examples.iter().enumerate() {
            prompt.push_str(&format!("## Example {}\n\n", idx + 1));
            prompt.push_str(&format!("**{}**\n\n", example.title));
            prompt.push_str(&format!("{}\n\n", example.description));

            prompt.push_str("**User Input:**\n");
            prompt.push_str(&format!("{}\n\n", example.input_prompt));

            prompt.push_str("**Expected Output:**\n");
            prompt.push_str(&format!("{}\n\n", example.expected_output));

            if !example.tools_used.is_empty() {
                prompt.push_str("**Tools Used:**\n");
                for tool in &example.tools_used {
                    prompt.push_str(&format!("- {}\n", tool));
                }
                prompt.push_str("\n");
            }
        }

        prompt
    }
}
```

### 5. Output Format Specifier

```rust
pub struct OutputFormatSpec {
    pub format_type: OutputFormat,
    pub include_reasoning: bool,
    pub include_tool_calls: bool,
    pub include_confidence: bool,
}

pub enum OutputFormat {
    Natural {
        structure: String,              // "json", "markdown", "plain"
    },
    Structured {
        schema: JsonSchema,
    },
    Custom {
        template: String,
    },
}

impl OutputFormatSpec {
    pub fn build_instruction(&self) -> String {
        let mut instruction = String::from("# Output Format\n\n");

        match &self.format_type {
            OutputFormat::Natural { structure } => {
                instruction.push_str(&format!("Respond in {} format.\n\n", structure));
            }
            OutputFormat::Structured { schema } => {
                instruction.push_str("Respond with a JSON object matching this schema:\n");
                instruction.push_str(&serde_json::to_string_pretty(schema).unwrap());
                instruction.push_str("\n\n");
            }
            OutputFormat::Custom { template } => {
                instruction.push_str("Follow this output template:\n");
                instruction.push_str(template);
                instruction.push_str("\n\n");
            }
        }

        if self.include_reasoning {
            instruction.push_str("Include your reasoning process.\n");
        }

        if self.include_tool_calls {
            instruction.push_str("Explicitly list any tool calls made.\n");
        }

        if self.include_confidence {
            instruction.push_str("Include a confidence score (0-1) for your final answer.\n");
        }

        instruction
    }
}
```

## 完整的提示词构建器

```rust
pub struct CompletePromptBuilder {
    agent_def_builder: AgentDefinitionBuilder,
    tool_def_builder: ToolDefinitionBuilder,
    context_builder: ConversationContextBuilder,
    example_builder: ExampleBuilder,
    format_spec: OutputFormatSpec,
    config: PromptConfig,
}

impl CompletePromptBuilder {
    pub fn build_system_prompt(&self) -> PromptResult {
        let mut sections = Vec::new();
        let mut total_tokens = 0;

        // 1. Agent Definition
        let agent_part = self.agent_def_builder.build();
        total_tokens += estimate_tokens(&agent_part);
        sections.push(agent_part);

        // 2. Tool Definitions
        let tools_part = self.tool_def_builder.build();
        total_tokens += estimate_tokens(&tools_part);
        sections.push(tools_part);

        // 3. Examples (if enabled)
        if self.config.include_examples {
            let examples_part = self.example_builder.build();
            total_tokens += estimate_tokens(&examples_part);
            sections.push(examples_part);
        }

        // 4. Output Format
        let format_part = self.format_spec.build_instruction();
        total_tokens += estimate_tokens(&format_part);
        sections.push(format_part);

        let system_prompt = sections.join("\n");

        PromptResult {
            system_prompt,
            estimated_tokens: total_tokens,
            sections_included: vec![
                "agent_definition",
                "tool_definitions",
                if self.config.include_examples { "examples" } else { "" },
                "output_format",
            ].into_iter().filter(|s| !s.is_empty()).collect(),
        }
    }

    pub fn build_user_message(&self) -> PromptResult {
        let (context, tokens) = self.context_builder.build();

        PromptResult {
            system_prompt: context,
            estimated_tokens: tokens,
            sections_included: vec!["conversation_context".to_string()],
        }
    }
}

pub struct PromptResult {
    pub system_prompt: String,
    pub estimated_tokens: u32,
    pub sections_included: Vec<String>,
}
```

## 配置示例

```yaml
prompt_builder:
  agent_definition:
    name: 'Research Assistant'
    role: 'You are an expert research assistant...'
    capabilities:
      - Web Research
      - Data Analysis
      - Code Writing
    tone: professional
    language: en

  tool_format_style: OpenAI # or: Claude, Custom

  include_examples: true
  include_agent_id: true
  max_context_tokens: 2000

  output_format:
    format_type: structured
    include_reasoning: true
    include_confidence: true
```

## Harness 集成

### Prompt Quality 评估

```yaml
test_case:
  name: 'Prompt Quality Assessment'
  prompt: 'Analyze this dataset'
  evaluators:
    - name: behavior
      config:
        expected_tools: ['web_search', 'analyze']
        reasoning_included: true
    - name: correctness
      config:
        confidence_present: true
        format_matches_spec: true
```

---

**版本**: 0.1.0 | **最后更新**: 2026-04-11  
**参考**: [docs/references/hermes-agent-patterns.md](../../references/hermes-agent-patterns.md)
