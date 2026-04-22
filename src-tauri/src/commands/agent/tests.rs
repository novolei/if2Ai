use super::*;
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::modules::memory;
use crate::modules::runtime::permissions::PermissionPolicy;
use crate::modules::runtime::session::{MessageRole, Session as RuntimeSession};
use crate::modules::scheduler;
use crate::modules::tools::{register_builtin_tools, ToolContext, ToolRegistry};

struct TempDirGuard {
    path: std::path::PathBuf,
}

impl TempDirGuard {
    fn new(path: std::path::PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct ScriptedSkillApiClient {
    call_count: usize,
}

impl ApiClient for ScriptedSkillApiClient {
    fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError> {
        self.call_count += 1;
        match self.call_count {
            1 => {
                let has_skill_definition = request
                    .tools
                    .as_ref()
                    .is_some_and(|tools| tools.iter().any(|tool| tool.name == "skill"));
                assert!(
                    has_skill_definition,
                    "request should include skill tool definition"
                );
                Ok(vec![
                    AssistantEvent::ToolUse {
                        id: "tool-skill-1".to_string(),
                        name: "skill".to_string(),
                        input: r#"{"skill":"demo-skill"}"#.to_string(),
                    },
                    AssistantEvent::MessageStop,
                ])
            }
            2 => {
                let last_message = request
                    .messages
                    .last()
                    .ok_or_else(|| RuntimeError::api_error("missing tool result message"))?;
                assert_eq!(last_message.role, MessageRole::Tool);
                let has_expected_skill_content = last_message.blocks.iter().any(|block| {
                    matches!(
                        block,
                        ContentBlock::ToolResult { output, .. }
                            if output.contains("# Demo Skill")
                    )
                });
                assert!(
                    has_expected_skill_content,
                    "tool result should contain loaded SKILL.md content"
                );
                Ok(vec![
                    AssistantEvent::TextDelta("技能已执行".to_string()),
                    AssistantEvent::MessageStop,
                ])
            }
            _ => Err(RuntimeError::api_error("unexpected extra API call")),
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_loop_executes_skill_tool_end_to_end() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be valid")
        .as_nanos();
    let workdir = std::env::temp_dir().join(format!("if2ai-agent-skill-e2e-{nanos}"));
    let _workdir_guard = TempDirGuard::new(workdir.clone());
    let skill_dir = workdir.join(".if2ai/skills/demo-skill");
    fs::create_dir_all(&skill_dir).expect("create skill directory");
    fs::write(
        skill_dir.join("SKILL.md"),
        "# Demo Skill\n\nUse demo skill.",
    )
    .expect("write skill markdown");
    fs::write(
        skill_dir.join("skill.json"),
        r#"{
  "id": "demo-skill",
  "version": "1.0.0",
  "apiVersion": "v1",
  "minAppVersion": "0.1.0",
  "capabilities": ["custom"],
  "review": {
"status": "active",
"riskLevel": "low",
"lastReviewedAt": "2026-04-14T00:00:00Z"
  }
}"#,
    )
    .expect("write skill manifest");

    let context = std::sync::Arc::new(Mutex::new(ToolContext::default_for_workdir(
        workdir.clone(),
    )));
    let registry = Arc::new(ToolRegistry::new(context));
    let test_browser_registry = crate::modules::browser::BrowserRegistry::for_test(
        std::path::PathBuf::from("/tmp/browser-cold-state-test.json"),
    );
    register_builtin_tools(
        &registry,
        memory::default_memory_provider().await,
        scheduler::default_scheduler(),
        test_browser_registry,
        std::sync::Arc::new(crate::modules::memory::NullPinnedStore::new()),
        None, // MEM-MOD-P4 — agent tests don't exercise the decision tree
    );

    let execution_context =
        SessionExecutionContext::stateless(workdir.clone(), PermissionMode::DangerFullAccess);
    let tool_executor = ToolRegistryExecutor::new_with_context(registry, execution_context);
    let mut runtime = ConversationRuntime::new(
        RuntimeSession::new(),
        ScriptedSkillApiClient { call_count: 0 },
        tool_executor,
        PermissionPolicy::new(PermissionMode::DangerFullAccess),
        vec!["system".to_string()],
    );

    let summary = runtime
        .run_turn("请运行 demo-skill", None)
        .expect("runtime should complete skill loop");
    assert_eq!(summary.iterations, 2);
    assert_eq!(summary.tool_results.len(), 1);
}
