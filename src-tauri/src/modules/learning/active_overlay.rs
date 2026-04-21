//! Active strategy overlay resolver (Phase M5 closeout, m5.7+
//! production flip minimum loop).
//!
//! Reads every record currently in [`RolloutState::Active`]
//! from the candidate registry and projects their typed
//! [`StrategyDefinition`]s into a single
//! [`ActiveStrategyOverlay`] that runtime consumers (today: the
//! prompt planner) can apply.
//!
//! Honest scope:
//!
//! - **Read-only**.  No mutation of registry state.  Every
//!   call walks the registry store fresh — caching is M6
//!   territory.
//! - **Singleton-active assumption**: rollout service enforces
//!   at most one Active record at any time, so the resolver's
//!   output usually contains zero or one effect.  The
//!   resolver still iterates a `Vec` so a future relaxation
//!   (per-scope multi-active) can drop in without changing
//!   shape.
//! - **No production behaviour change beyond the prompt
//!   overlay block**.  M5 closeout opens the seam; M6 may add
//!   tool-mask enforcement, provider routing, etc.  Each
//!   variant of [`StrategyDefinition`] that does not yet have
//!   a runtime consumer is silently surfaced as an audit-only
//!   note in the overlay text (so reviewers can still see
//!   what was active even if the consumer hook is M6).

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use super::strategy_registry::{CandidateStrategy, RolloutState, StrategyDefinition};
use super::strategy_registry_service::StrategyRegistryService;

/// Stable contract version.
pub const ACTIVE_STRATEGY_OVERLAY_VERSION: &str = "active-strategy-overlay@m5.closeout";

/// Per-active-record overlay summary.  Used by the prompt
/// planner / future hook surfaces; mirrors only the fields
/// runtime consumers need (full record stays in the registry).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveStrategyEffect {
    pub strategy_id: String,
    pub label: String,
    pub kind: String,
    /// Rendered text the prompt planner appends as a system-prompt
    /// block.  Empty when the definition does not project to a
    /// prompt (future variants).
    pub prompt_overlay: String,
}

/// Aggregate overlay across all currently-active strategies.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActiveStrategyOverlay {
    pub overlay_version: String,
    pub effects: Vec<ActiveStrategyEffect>,
}

impl ActiveStrategyOverlay {
    /// True iff at least one effect carries a non-empty
    /// `prompt_overlay`.
    #[must_use]
    pub fn has_prompt_text(&self) -> bool {
        self.effects.iter().any(|e| !e.prompt_overlay.is_empty())
    }

    /// Concatenate every active effect's prompt overlay into
    /// one block.  Empty string when no effect produces text.
    #[must_use]
    pub fn render_prompt_block(&self) -> String {
        let parts: Vec<&str> = self
            .effects
            .iter()
            .filter_map(|e| {
                if e.prompt_overlay.is_empty() {
                    None
                } else {
                    Some(e.prompt_overlay.as_str())
                }
            })
            .collect();
        parts.join("\n\n")
    }

    /// Convenience constructor used by callers that want to
    /// short-circuit the resolver (e.g. when the registry is
    /// disabled in tests).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            overlay_version: ACTIVE_STRATEGY_OVERLAY_VERSION.to_string(),
            effects: Vec::new(),
        }
    }
}

/// Resolver service.  Holds a [`StrategyRegistryService`]; cheap
/// to construct.  Stateless.
#[derive(Clone)]
pub struct ActiveStrategyOverlayResolver {
    registry: StrategyRegistryService,
}

impl ActiveStrategyOverlayResolver {
    #[must_use]
    pub fn new(registry: StrategyRegistryService) -> Self {
        Self { registry }
    }

    #[must_use]
    pub fn with_default_root() -> Self {
        Self::new(StrategyRegistryService::with_default_root())
    }

    /// Walk the registry for every record currently in
    /// `RolloutState::Active` and project each into an
    /// [`ActiveStrategyEffect`].  Returns an empty overlay
    /// when nothing is active.
    pub async fn resolve(&self) -> ActiveStrategyOverlay {
        let entries = match self.registry.store().list().await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    target: "learning.overlay",
                    error = %e,
                    "[active_overlay] registry list failed; rendering empty overlay"
                );
                return ActiveStrategyOverlay::empty();
            }
        };
        let mut effects: Vec<ActiveStrategyEffect> = Vec::new();
        for entry in entries {
            if entry.rollout_state != RolloutState::Active.label() {
                continue;
            }
            match self.registry.store().load(&entry.strategy_id).await {
                Ok(Some(record)) => {
                    if let Some(effect) = project_effect(&record) {
                        effects.push(effect);
                    }
                }
                _ => continue,
            }
        }
        ActiveStrategyOverlay {
            overlay_version: ACTIVE_STRATEGY_OVERLAY_VERSION.to_string(),
            effects,
        }
    }
}

/// Pure projection from a typed [`StrategyDefinition`] into the
/// overlay-effect shape.  Returns `None` when the definition
/// has no runtime effect (e.g. `Noop`).
#[must_use]
pub fn project_effect(record: &CandidateStrategy) -> Option<ActiveStrategyEffect> {
    if !record.definition.has_runtime_effect() {
        return None;
    }
    let prompt_overlay = match &record.definition {
        StrategyDefinition::Noop => String::new(),
        StrategyDefinition::PromptOverlay { text } => format!(
            "[active_strategy:{}] {}",
            record.identity.strategy_id, text
        ),
        StrategyDefinition::DiscourageTool { tool_name } => format!(
            "[active_strategy:{}] Avoid tool '{}' unless strictly necessary; prefer a safer alternative.",
            record.identity.strategy_id, tool_name
        ),
    };
    Some(ActiveStrategyEffect {
        strategy_id: record.identity.strategy_id.clone(),
        label: record.identity.label.clone(),
        kind: record.definition.kind_label().to_string(),
        prompt_overlay,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::learning::strategy_registry::{
        CandidateStrategy, StrategyDefinition, StrategyIdentity, StrategySource,
    };
    use chrono::Utc;

    fn record_with_def(id: &str, def: StrategyDefinition) -> CandidateStrategy {
        let mut r = CandidateStrategy::new_draft(
            StrategyIdentity {
                strategy_id: id.into(),
                label: format!("label-{id}"),
                policy_version: None,
                definition_ref: None,
            },
            StrategySource::Manual,
            None,
            Utc::now(),
        );
        r.definition = def;
        r
    }

    #[test]
    fn project_effect_skips_noop() {
        let r = record_with_def("a", StrategyDefinition::Noop);
        assert!(project_effect(&r).is_none());
    }

    #[test]
    fn project_effect_renders_prompt_overlay() {
        let r = record_with_def(
            "b",
            StrategyDefinition::PromptOverlay {
                text: "Always cite sources".into(),
            },
        );
        let e = project_effect(&r).unwrap();
        assert!(e.prompt_overlay.contains("Always cite sources"));
        assert!(e.prompt_overlay.contains("active_strategy:b"));
        assert_eq!(e.kind, "prompt_overlay");
    }

    #[test]
    fn project_effect_renders_tool_discourage() {
        let r = record_with_def(
            "c",
            StrategyDefinition::DiscourageTool {
                tool_name: "rm_rf".into(),
            },
        );
        let e = project_effect(&r).unwrap();
        assert!(e.prompt_overlay.contains("rm_rf"));
        assert_eq!(e.kind, "discourage_tool");
    }

    #[test]
    fn render_prompt_block_concatenates_effects() {
        let mut o = ActiveStrategyOverlay::empty();
        o.effects.push(ActiveStrategyEffect {
            strategy_id: "a".into(),
            label: "A".into(),
            kind: "prompt_overlay".into(),
            prompt_overlay: "first".into(),
        });
        o.effects.push(ActiveStrategyEffect {
            strategy_id: "b".into(),
            label: "B".into(),
            kind: "prompt_overlay".into(),
            prompt_overlay: "second".into(),
        });
        let block = o.render_prompt_block();
        assert!(block.contains("first"));
        assert!(block.contains("second"));
        assert!(o.has_prompt_text());
    }
}
