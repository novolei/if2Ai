//! Tool executor — bridges async `ToolRegistry` to the sync `ToolExecutor`
//! trait used by `ConversationRuntime`. Owns the
//! `ControlPlaneRuntimeSwitches` config loader.
//!
//! Extracted from `commands/agent.rs` in GFR-006a (pure structural move,
//! function bodies byte-identical).

use std::collections::HashSet;
use std::sync::Arc;

use crate::modules::control_plane::{AuditEmitter, SessionExecutionContext, ToolExecutionBroker};
use crate::modules::runtime::block_conversion::parse_tool_input_json;
use crate::modules::runtime::conversation::{ToolError, ToolExecutor};

#[derive(Debug, Clone)]
pub(crate) struct ControlPlaneRuntimeSwitches {
    pub(crate) control_plane_v2_enabled: bool,
    pub(crate) boundary_enforce_mode: crate::modules::runtime::config::BoundaryEnforceMode,
    pub(crate) sandbox_strict_mode: bool,
}

pub(crate) fn load_control_plane_switches(
    workdir: &std::path::Path,
) -> ControlPlaneRuntimeSwitches {
    let mut switches = crate::modules::runtime::config::ConfigLoader::default_for(workdir)
        .load()
        .map(|loaded| ControlPlaneRuntimeSwitches {
            control_plane_v2_enabled: loaded.control_plane().control_plane_v2_enabled(),
            boundary_enforce_mode: loaded.control_plane().boundary_enforce_mode(),
            sandbox_strict_mode: loaded.control_plane().sandbox_strict_mode(),
        })
        .unwrap_or(ControlPlaneRuntimeSwitches {
            control_plane_v2_enabled: true,
            boundary_enforce_mode: crate::modules::runtime::config::BoundaryEnforceMode::Enforce,
            sandbox_strict_mode: true,
        });
    if let Ok(value) = std::env::var("IF2AI_CONTROL_PLANE_V2_ENABLED") {
        switches.control_plane_v2_enabled = value != "0";
    }
    if let Ok(value) = std::env::var("IF2AI_BOUNDARY_ENFORCE_MODE") {
        switches.boundary_enforce_mode = if value.eq_ignore_ascii_case("shadow") {
            crate::modules::runtime::config::BoundaryEnforceMode::Shadow
        } else {
            crate::modules::runtime::config::BoundaryEnforceMode::Enforce
        };
    }
    if let Ok(value) = std::env::var("IF2AI_SANDBOX_STRICT_MODE") {
        switches.sandbox_strict_mode = value != "0";
    }
    switches
}

/// Bridge from async ToolRegistry to sync ToolExecutor trait.
///
/// This allows ConversationRuntime to use the ToolRegistry for tool calls.
pub(crate) struct ToolRegistryExecutor {
    tool_registry: Arc<crate::modules::tools::ToolRegistry>,
    broker: ToolExecutionBroker,
    pub(crate) execution_context: SessionExecutionContext,
    /// When set, only these tool names are advertised to the LLM and accepted in `execute`.
    definition_allowlist: Option<HashSet<String>>,
}

impl ToolRegistryExecutor {
    pub(crate) fn new_with_context(
        tool_registry: Arc<crate::modules::tools::ToolRegistry>,
        execution_context: SessionExecutionContext,
    ) -> Self {
        Self {
            tool_registry: tool_registry.clone(),
            broker: ToolExecutionBroker::new(tool_registry),
            execution_context,
            definition_allowlist: None,
        }
    }

    #[must_use]
    pub(crate) fn with_definition_allowlist(mut self, allowlist: Option<HashSet<String>>) -> Self {
        self.definition_allowlist = allowlist;
        self
    }

    pub(crate) fn execute_with_trace(
        &mut self,
        tool_name: &str,
        input: &str,
        trace_id: &str,
        request_id: Option<&str>,
        attempt_id: Option<&str>,
    ) -> Result<String, ToolError> {
        if let Some(ref allow) = self.definition_allowlist {
            if !allow.contains(tool_name) {
                return Err(ToolError::new(format!(
                    "tool `{tool_name}` is not available while low-trust skills are active (skill trust attenuation)"
                )));
            }
        }
        let args = parse_tool_input_json(input);
        let switches = load_control_plane_switches(&self.execution_context.workdir);
        tracing::info!(
            "[tool_executor] control_plane_v2_enabled={}, boundary_enforce_mode={}, sandbox_strict_mode={}, attempt_id='{}'",
            switches.control_plane_v2_enabled,
            switches.boundary_enforce_mode.as_str(),
            switches.sandbox_strict_mode,
            attempt_id.unwrap_or("none"),
        );
        // P1-9 + P2-14 — emit a bounded, redacted observer event before
        // dispatch. No-op unless `IF2AI_OBSERVER=log`; payload is always
        // run through `redact_tool_args_summary` so secrets never reach
        // the observer surface.
        let args_summary =
            crate::modules::security::redaction::redact_tool_args_summary(&args, 512);
        crate::modules::observability::emit(
            "tool.execute.start",
            &format!("tool={tool_name} trace={trace_id} args={args_summary}"),
        );
        let result = tokio::task::block_in_place(|| {
            let handle = tokio::runtime::Handle::current();
            if switches.control_plane_v2_enabled {
                handle.block_on(self.broker.execute_with_trace(
                    &self.execution_context,
                    tool_name,
                    args,
                    trace_id,
                    request_id,
                ))
            } else {
                tracing::warn!(
                    "[tool_executor] controlPlaneV2Enabled=false, falling back to direct dispatch_with_context"
                );
                // Phase 7C, slice 7C.2 — registry now returns ToolOutput;
                // collapse to legacy String here so the existing executor
                // contract (Result<String, ToolError>) stays intact.  Slice
                // 7C.3+ will lift the broker + executor to ToolOutput.
                handle.block_on(self.tool_registry.dispatch_with_context_legacy(
                    tool_name,
                    args,
                    self.broker.to_tool_context(&self.execution_context),
                ))
            }
        })
        .map_err(|e: crate::modules::tools::ToolError| {
            crate::modules::observability::emit(
                "tool.execute.err",
                &format!("tool={tool_name} trace={trace_id} err={}", e),
            );
            ToolError::new(e.to_string())
        })?;
        crate::modules::observability::emit(
            "tool.execute.ok",
            &format!("tool={tool_name} trace={trace_id} bytes={}", result.len()),
        );
        Ok(result)
    }
}

impl ToolExecutor for ToolRegistryExecutor {
    fn execute(&mut self, tool_name: &str, input: &str) -> Result<String, ToolError> {
        let trace_id = AuditEmitter::new_trace_id();
        self.execute_with_trace(tool_name, input, &trace_id, None, None)
    }

    fn get_definitions(&self) -> Vec<crate::modules::api::ToolDefinition> {
        let definitions = self.tool_registry.get_definitions(None);
        let mut out: Vec<crate::modules::api::ToolDefinition> = definitions
            .into_iter()
            .filter_map(|def| {
                let obj = def.as_object()?;
                let func = obj.get("function")?.as_object()?;
                Some(crate::modules::api::ToolDefinition {
                    name: func.get("name")?.as_str()?.to_string(),
                    description: func
                        .get("description")
                        .and_then(|d| d.as_str())
                        .map(String::from),
                    input_schema: func.get("parameters")?.clone(),
                })
            })
            .collect();
        if let Some(ref allow) = self.definition_allowlist {
            out.retain(|d| allow.contains(&d.name));
        }
        out
    }
}
