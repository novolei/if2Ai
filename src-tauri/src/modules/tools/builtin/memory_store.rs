//! Memory Store tool - stores a fact in long-term memory
//!
//! Provides persistent memory storage across sessions, scoped to the current session
//! when a `session_id` is available in the tool context.

use std::sync::Arc;

use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};
use crate::modules::memory::policy::{MemoryPolicyEngine, PolicyDecision};
use crate::modules::memory::scope::MemoryScopeResolver;
use crate::modules::memory::{MemoryCategory, SharedMemoryProvider};
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Creates the memory_store tool entry for the registry.
#[allow(dead_code)]
#[must_use]
pub fn entry(memory: SharedMemoryProvider) -> ToolEntry {
    let handler: ToolHandler =
        Arc::new(move |args: serde_json::Value, context: SharedToolContext| {
            let memory = memory.clone();
            Box::pin(async move {
                let key = args
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::Handler("missing required parameter: key".to_string())
                    })?
                    .to_string();

                let content = args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::Handler("missing required parameter: content".to_string())
                    })?
                    .to_string();

                let category = args
                    .get("category")
                    .and_then(|v| v.as_str())
                    .map(|c| match c {
                        "core" => MemoryCategory::Core,
                        "daily" => MemoryCategory::Daily,
                        "conversation" => MemoryCategory::Conversation,
                        other => MemoryCategory::Custom(other.to_string()),
                    })
                    .unwrap_or(MemoryCategory::Conversation);

                // Resolve scope from the tool execution context.  When a session_id
                // is present, the entry is tagged so recall_scoped() can filter by session.
                let scope = context
                    .lock()
                    .ok()
                    .map(|ctx| MemoryScopeResolver::from_tool_context(&ctx))
                    .unwrap_or_else(crate::modules::memory::scope::MemoryExecutionScope::global);

                let audit_ctx = AuditContext::from_scope(&scope);

                // Emit: memory captured (agent has identified info to remember).
                MemoryAuditEmitter::memory_captured(&audit_ctx, &key, category.as_str());

                // Evaluate write policy (shadow mode by default — logs decisions, never blocks).
                let policy = MemoryPolicyEngine::default_shadow();
                let policy_result = policy.evaluate_write(&key, &content, &category, &scope);

                // Emit: write decision (allow / deny / prompt + reason_code).
                MemoryAuditEmitter::memory_write_decision(
                    &audit_ctx,
                    &key,
                    &policy_result.decision,
                    &policy_result.reason_code,
                    &policy_result.message,
                );

                // In enforce mode a Deny decision blocks the write.
                // In shadow mode Deny is downgraded to Allow, so this branch only fires in Enforce.
                if policy_result.decision == PolicyDecision::Deny {
                    MemoryAuditEmitter::memory_rejected(
                        &audit_ctx,
                        &key,
                        &policy_result.reason_code,
                        &policy_result.message,
                    );
                    return Err(ToolError::Handler(format!(
                        "memory write denied by policy: {} — {}",
                        policy_result.reason_code.label(),
                        policy_result.message
                    )));
                }

                memory
                    .store_scoped(&key, &content, category.clone(), &scope)
                    .await
                    .map_err(|e| ToolError::Handler(format!("failed to store memory: {}", e)))?;

                // Emit: memory successfully persisted.
                MemoryAuditEmitter::memory_persisted(&audit_ctx, &key, category.as_str());

                Ok(format!("Stored memory: {}", key))
            })
        });

    ToolEntry {
        name: "memory_store".to_string(),
        toolset: "memory".to_string(),
        description: "Store a fact in long-term memory".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "key": {
                    "type": "string",
                    "description": "Unique key for this memory"
                },
                "content": {
                    "type": "string",
                    "description": "Content to store"
                },
                "category": {
                    "type": "string",
                    "description": "Category: core, daily, conversation, or custom string"
                }
            },
            "required": ["key", "content"]
        }),
        max_result_size: Some(1024),
        timeout_secs: Some(10),
        disabled: false,
        handler,
    }
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;
    use crate::modules::memory::InMemoryMemoryProvider;

    fn test_memory() -> SharedMemoryProvider {
        Arc::new(InMemoryMemoryProvider::new())
    }

    #[tokio::test]
    async fn memory_store_tool_entry_has_correct_structure() {
        let entry = entry(test_memory());
        assert_eq!(entry.name, "memory_store");
        assert_eq!(entry.toolset, "memory");
        assert!(!entry.disabled);
    }
}
