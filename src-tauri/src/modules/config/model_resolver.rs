//! ModelResolver — runtime model resolution.
//!
//! Provides the single source of truth for resolving model references
//! to full connection details (base_url, api_key, api protocol).
//!
//! ## Resolution Priority
//!
//! 1. `~/.if2ai/models.json` + `~/.if2ai/config.json` (if2AI native config)
//!
//! ## Model Reference Format
//!
//! Model references use the `"provider_id/model_id"` string format,
//! which unambiguously identifies a model even when multiple providers
//! have overlapping model names (e.g., "gpt-4o" on both OpenAI and custom).
//!
//! ## Role-Based Model Routing
//!
//! Supports per-role model assignments (chat, utility, summarizer, etc.)
//! via `AppConfig.role_models`. The resolution fallback chain is:
//! 1. Explicit role assignment in `role_models`
//! 2. `active_model` from config
//! 3. First model in `selected_models`

use std::collections::HashMap;

use crate::modules::config::store::{config_json_path, models_json_path, read_json};
use crate::modules::config::types::{
    AppConfig, ModelRef, ModelRoleConfig, ModelSelection, ModelsJson,
};

/// Fully resolved model with connection details.
#[derive(Debug, Clone)]
pub struct ResolvedModel {
    /// Provider identifier (e.g. "anthropic", "openai")
    pub provider_id: String,
    /// Model identifier (e.g. "claude-sonnet-4-6")
    pub model_id: String,
    /// API base URL
    pub base_url: String,
    /// API key for authentication
    pub api_key: Option<String>,
    /// API protocol: "openai-completions" | "anthropic-messages"
    pub api: String,
}

/// Available model entry for UI display.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AvailableModel {
    /// Provider identifier
    pub provider_id: String,
    /// Model identifier
    pub model_id: String,
    /// Display name
    pub name: String,
    /// Context window size (if known)
    pub context_window: Option<u64>,
    /// P-MULTI-API — model exposes reasoning / chain-of-thought.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reasoning: bool,
    /// P-MULTI-API — assistant tool_call history rows must carry
    /// `reasoning_content` (Kimi-thinking-preview / DeepSeek-R1).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reasoning_required_in_tool_calls: bool,
    /// P-MULTI-API — model accepts top-level `reasoning_effort`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub supports_reasoning_effort: bool,
}

/// Available model group — all models for one provider.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AvailableModelGroup {
    /// Provider identifier
    pub provider_id: String,
    /// Provider display name
    pub provider_name: String,
    /// Models available for this provider
    pub models: Vec<AvailableModel>,
}

/// Runtime model resolver.
pub struct ModelResolver;

impl ModelResolver {
    /// Load models.json from `~/.if2ai/`.
    pub async fn load_models_json() -> Result<ModelsJson, String> {
        let path = models_json_path();
        let result = read_json::<ModelsJson>(&path)
            .await
            .map_err(|e| format!("Failed to read models.json at {}: {}", path.display(), e))?;
        result.ok_or_else(|| "models.json not found".to_string())
    }

    /// Load config.json from `~/.if2ai/`.
    pub async fn load_config() -> Result<AppConfig, String> {
        let path = config_json_path();
        let result = read_json::<AppConfig>(&path)
            .await
            .map_err(|e| format!("Failed to read config.json at {}: {}", path.display(), e))?;
        result.ok_or_else(|| "config.json not found".to_string())
    }

    /// Resolve a model reference ("provider_id/model_id") to full connection details.
    ///
    /// # Arguments
    ///
    /// * `model_ref` - Model reference in "provider_id/model_id" format
    ///
    /// # Resolution Flow
    ///
    /// 1. Parse the reference to extract provider_id and model_id
    /// 2. Load models.json to get base_url, api_key, api protocol
    /// 3. Return ResolvedModel with all connection details
    pub async fn resolve_model(model_ref: &str) -> Result<ResolvedModel, String> {
        let model = ModelRef::parse(model_ref).ok_or_else(|| {
            format!("Invalid model reference '{model_ref}'. Expected 'provider_id/model_id'.")
        })?;

        let models_json = Self::load_models_json().await?;

        // Try exact provider_id key first
        let entry = models_json.providers.get(&model.provider_id);

        // Try with auth_variant key format: "provider_id::variant"
        let entry = entry.or_else(|| {
            models_json
                .providers
                .iter()
                .find(|(k, _)| k.starts_with(&format!("{}::", model.provider_id)))
                .map(|(_, v)| v)
        });

        let entry = entry.ok_or_else(|| {
            format!(
                "Provider '{}' not found in models.json. Available providers: {}",
                model.provider_id,
                models_json
                    .providers
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;

        // Find the specific model in the provider's model list
        let _model_entry = entry
            .models
            .iter()
            .find(|m| m.id == model.model_id)
            .ok_or_else(|| {
                format!(
                    "Model '{}' not found for provider '{}'. Available: {}",
                    model.model_id,
                    model.provider_id,
                    entry
                        .models
                        .iter()
                        .map(|m| m.id.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?;

        let base_url = entry
            .base_url
            .clone()
            .ok_or_else(|| format!("Provider '{}' missing base_url", model.provider_id))?;

        let api = entry
            .api
            .clone()
            .unwrap_or_else(|| "openai-completions".to_string());

        Ok(ResolvedModel {
            provider_id: model.provider_id,
            model_id: model.model_id,
            base_url,
            api_key: entry.api_key.clone(),
            api,
        })
    }

    /// Get the model for a specific role.
    ///
    /// # Resolution Fallback Chain
    ///
    /// 1. Explicit role assignment in `role_models`
    /// 2. `active_model` from config
    /// 3. First model in `selected_models`
    pub async fn resolve_role_model(role: &str) -> Result<ResolvedModel, String> {
        // 1. Try explicit role assignment
        let config = Self::load_config().await?;
        if let Some(role_config) = config.role_models.iter().find(|r| r.role == role) {
            if let Some(ref model_ref) = role_config.model_ref {
                return Self::resolve_model(model_ref).await;
            }
        }

        // 2. Try active_model
        if let Some(ref selection) = config.active_model {
            let model_ref = format!("{}/{}", selection.provider_id, selection.model_id);
            return Self::resolve_model(&model_ref).await;
        }

        // 3. Try first selected_model
        if let Some(first) = config.selected_models.first() {
            let model_ref = format!("{}/{}", first.provider_id, first.model_id);
            return Self::resolve_model(&model_ref).await;
        }

        Err("No model configured".to_string())
    }

    /// List all available models grouped by provider.
    ///
    /// Combines models from:
    /// 1. `~/.if2ai/models.json` (configured models)
    /// 2. Builtin provider registry (all known models)
    pub async fn list_available_models() -> Vec<AvailableModelGroup> {
        let mut groups: HashMap<String, AvailableModelGroup> = HashMap::new();

        // 1. Load configured models from models.json
        if let Ok(models_json) = Self::load_models_json().await {
            for (provider_key, entry) in models_json.providers {
                // Extract base provider_id from key (strip auth_variant)
                let provider_id = provider_key
                    .split_once("::")
                    .map(|(id, _)| id.to_string())
                    .unwrap_or(provider_key);

                let group =
                    groups
                        .entry(provider_id.clone())
                        .or_insert_with(|| AvailableModelGroup {
                            provider_id: provider_id.clone(),
                            provider_name: provider_id.clone(),
                            models: vec![],
                        });

                for model in &entry.models {
                    let cap = crate::modules::provider::capabilities::resolve(
                        provider_id.as_str(),
                        &model.id,
                        None,
                        crate::modules::provider::capabilities::GlobalThinkingPolicy::Auto,
                    );
                    group.models.push(AvailableModel {
                        provider_id: provider_id.clone(),
                        model_id: model.id.clone(),
                        name: model.name.clone(),
                        context_window: model.context_window,
                        reasoning: cap.reasoning,
                        reasoning_required_in_tool_calls: cap.reasoning_required_in_tool_calls,
                        supports_reasoning_effort: cap.supports_reasoning_effort,
                    });
                }
            }
        }

        // 2. Add builtin provider names for display
        for provider in crate::modules::provider::registry::builtin_providers() {
            let group = groups
                .entry(provider.id.clone())
                .or_insert_with(|| AvailableModelGroup {
                    provider_id: provider.id.clone(),
                    provider_name: provider.name.clone(),
                    models: vec![],
                });
            // Use the builtin provider name as the display name
            group.provider_name = provider.name.clone();
        }

        // Sort groups by provider_id for consistent ordering
        let mut sorted: Vec<_> = groups.into_values().collect();
        sorted.sort_by(|a, b| a.provider_id.cmp(&b.provider_id));
        sorted
    }

    /// Get the current active model selection.
    ///
    /// Resolution priority:
    /// 1. The chat role's `model_ref` ("provider/model") — but recover the
    ///    `auth_variant` from `cfg.active_model` when (provider_id, model_id)
    ///    match (the chat role string format does not carry variant).
    /// 2. Fall back to `cfg.active_model` directly.
    pub async fn get_active_model() -> Result<Option<ModelSelection>, String> {
        let config = Self::load_config().await?;
        if let Some(chat_role) = config.role_models.iter().find(|r| r.role == "chat") {
            if let Some(model_ref) = chat_role.model_ref.as_deref() {
                if let Some(model) = ModelRef::parse(model_ref) {
                    let recovered =
                        recover_active_model_with_variant(&model, config.active_model.as_ref());
                    return Ok(Some(recovered));
                }
            }
        }
        Ok(config.active_model.clone())
    }

    /// Set the active model selection.
    ///
    /// `auth_variant` is propagated end-to-end so that multi-auth providers
    /// (e.g. moonshot-cn vs moonshot-code) round-trip correctly through
    /// `get_active_model()` after a restart. See ER-01.
    pub async fn set_active_model(
        provider_id: &str,
        model_id: &str,
        auth_variant: Option<&str>,
    ) -> Result<(), String> {
        crate::modules::provider::service::select_model(provider_id, model_id, auth_variant)
            .await?;
        let model_ref = format!("{provider_id}/{model_id}");
        Self::set_role_config_with_variant("chat", &model_ref, auth_variant).await
    }

    /// Get all role-based model assignments.
    pub async fn get_role_config() -> Result<Vec<ModelRoleConfig>, String> {
        let config = Self::load_config().await?;
        Ok(config.role_models.clone())
    }

    /// Set a role's model assignment (no `auth_variant` — preserved for
    /// callers that don't carry variant information). For the chat role,
    /// `cfg.active_model.auth_variant` is written as `None`.
    pub async fn set_role_config(role: &str, model_ref: &str) -> Result<(), String> {
        Self::set_role_config_with_variant(role, model_ref, None).await
    }

    /// Set a role's model assignment, propagating `auth_variant` into
    /// `cfg.active_model` when the role is `"chat"` (the user-visible
    /// composer default). See ER-01.
    pub async fn set_role_config_with_variant(
        role: &str,
        model_ref: &str,
        auth_variant: Option<&str>,
    ) -> Result<(), String> {
        let config = Self::load_config().await?;

        // Validate the model reference format
        let model = ModelRef::parse(model_ref).ok_or_else(|| {
            format!("Invalid model reference '{model_ref}'. Expected 'provider_id/model_id'.")
        })?;

        // Check if the model exists in models.json
        let models_json = Self::load_models_json().await?;
        let provider_found = models_json.providers.keys().any(|k| {
            let pid = k.split_once("::").map(|(id, _)| id).unwrap_or(k);
            pid == model.provider_id
        });
        if !provider_found {
            return Err(format!(
                "Provider '{}' not found in configured models",
                model.provider_id
            ));
        }

        // Update or add role config
        let mut role_models = config.role_models.clone();
        let existing = role_models.iter_mut().find(|r| r.role == role);
        if let Some(existing) = existing {
            existing.model_ref = Some(model_ref.to_string());
        } else {
            role_models.push(ModelRoleConfig {
                role: role.to_string(),
                model_ref: Some(model_ref.to_string()),
            });
        }

        // Save updated config
        let service = crate::modules::config::ConfigService::new();
        service
            .save_config(&{
                let mut cfg = config;
                cfg.role_models = role_models;
                if role == "chat" {
                    cfg.active_model = Some(build_active_model_selection(
                        &model.provider_id,
                        &model.model_id,
                        auth_variant,
                    ));
                }
                cfg
            })
            .await
            .map_err(|e| format!("Failed to save role config: {e}"))
    }
}

// ── Pure helpers (ER-01) ───────────────────────────────────────────────────
//
// Variant-handling logic is extracted into pure functions so the round-trip
// invariant ("auth_variant survives set → get") can be unit-tested without
// touching the filesystem.

/// Construct a [`ModelSelection`] preserving the optional `auth_variant`.
///
/// Used everywhere `cfg.active_model` is written so we never silently drop
/// the variant for multi-auth providers (e.g. moonshot-cn vs moonshot-code).
#[must_use]
pub fn build_active_model_selection(
    provider_id: &str,
    model_id: &str,
    auth_variant: Option<&str>,
) -> ModelSelection {
    ModelSelection {
        provider_id: provider_id.to_string(),
        model_id: model_id.to_string(),
        auth_variant: auth_variant.map(str::to_string),
    }
}

/// Recover the full [`ModelSelection`] (including `auth_variant`) for the
/// given `parsed_chat_ref`. The chat role only stores `"provider/model"`
/// without variant; we look at the provided snapshot of `cfg.active_model`
/// and copy the variant when (provider_id, model_id) match.
#[must_use]
pub fn recover_active_model_with_variant(
    parsed_chat_ref: &ModelRef,
    cfg_active_model: Option<&ModelSelection>,
) -> ModelSelection {
    let auth_variant = cfg_active_model
        .filter(|m| {
            m.provider_id == parsed_chat_ref.provider_id && m.model_id == parsed_chat_ref.model_id
        })
        .and_then(|m| m.auth_variant.clone());
    ModelSelection {
        provider_id: parsed_chat_ref.provider_id.clone(),
        model_id: parsed_chat_ref.model_id.clone(),
        auth_variant,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_ref_parse_valid() {
        let model = ModelRef::parse("anthropic/claude-sonnet-4-6").unwrap();
        assert_eq!(model.provider_id, "anthropic");
        assert_eq!(model.model_id, "claude-sonnet-4-6");
    }

    #[test]
    fn test_model_ref_parse_invalid() {
        assert!(ModelRef::parse("invalid").is_none());
        assert!(ModelRef::parse("").is_none());
        assert!(ModelRef::parse("/model").is_none());
        assert!(ModelRef::parse("provider/").is_none());
    }

    #[test]
    fn test_model_ref_display() {
        let model = ModelRef::parse("openai/gpt-4o").unwrap();
        assert_eq!(model.to_string(), "openai/gpt-4o");
    }

    // ── ER-01 round-trip: auth_variant must survive set → get ──────────────
    //
    // Bug: `set_active_model` and `get_active_model` previously hardcoded
    // `auth_variant: None`, silently dropping the variant for multi-auth
    // providers (e.g. moonshot-cn vs moonshot-code). These tests target
    // the pure mapping helpers so we don't depend on filesystem I/O.

    #[test]
    fn build_active_model_selection_carries_variant() {
        let sel = build_active_model_selection("moonshot", "kimi-k2-0905-preview", Some("cn"));
        assert_eq!(sel.provider_id, "moonshot");
        assert_eq!(sel.model_id, "kimi-k2-0905-preview");
        assert_eq!(sel.auth_variant.as_deref(), Some("cn"));
    }

    #[test]
    fn build_active_model_selection_without_variant_is_none() {
        let sel = build_active_model_selection("ollama", "qwen3:4b", None);
        assert_eq!(sel.provider_id, "ollama");
        assert_eq!(sel.model_id, "qwen3:4b");
        assert!(sel.auth_variant.is_none());
    }

    #[test]
    fn recover_active_model_with_variant_uses_snapshot_when_match() {
        // chat role only stores "provider_id/model_id"; variant must be
        // recovered from cfg.active_model when (provider_id, model_id) match.
        let parsed = ModelRef::parse("moonshot/kimi-k2-0905-preview").unwrap();
        let snapshot = ModelSelection {
            provider_id: "moonshot".into(),
            model_id: "kimi-k2-0905-preview".into(),
            auth_variant: Some("cn".into()),
        };
        let recovered = recover_active_model_with_variant(&parsed, Some(&snapshot));
        assert_eq!(recovered.auth_variant.as_deref(), Some("cn"));
    }

    #[test]
    fn recover_active_model_with_variant_ignores_snapshot_on_mismatch() {
        // If the snapshot points at a different (provider, model), the
        // variant must NOT leak into the chat-role-derived selection.
        let parsed = ModelRef::parse("anthropic/claude-sonnet-4-6").unwrap();
        let snapshot = ModelSelection {
            provider_id: "moonshot".into(),
            model_id: "kimi-k2-0905-preview".into(),
            auth_variant: Some("cn".into()),
        };
        let recovered = recover_active_model_with_variant(&parsed, Some(&snapshot));
        assert!(recovered.auth_variant.is_none());
        assert_eq!(recovered.provider_id, "anthropic");
        assert_eq!(recovered.model_id, "claude-sonnet-4-6");
    }

    #[test]
    fn recover_active_model_with_variant_handles_missing_snapshot() {
        let parsed = ModelRef::parse("ollama/qwen3:4b").unwrap();
        let recovered = recover_active_model_with_variant(&parsed, None);
        assert!(recovered.auth_variant.is_none());
    }

    #[test]
    fn test_available_model_group_sorting() {
        // Test that groups are sorted by provider_id
        let mut groups = [
            AvailableModelGroup {
                provider_id: "zai".to_string(),
                provider_name: "Zai".to_string(),
                models: vec![],
            },
            AvailableModelGroup {
                provider_id: "anthropic".to_string(),
                provider_name: "Anthropic".to_string(),
                models: vec![],
            },
            AvailableModelGroup {
                provider_id: "openai".to_string(),
                provider_name: "OpenAI".to_string(),
                models: vec![],
            },
        ];
        groups.sort_by(|a, b| a.provider_id.cmp(&b.provider_id));
        assert_eq!(groups[0].provider_id, "anthropic");
        assert_eq!(groups[1].provider_id, "openai");
        assert_eq!(groups[2].provider_id, "zai");
    }
}
