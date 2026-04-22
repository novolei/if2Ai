use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::definition::{PersonaDefinition, SoulDefinition};
use super::registry::IdentityRegistry;

/// User-editable identity overrides persisted under `~/.if2ai/prompt/`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityCustomizationPack {
    #[serde(default)]
    pub souls: BTreeMap<String, SoulCustomization>,
    #[serde(default)]
    pub personas: BTreeMap<String, PersonaCustomization>,
}

pub const IDENTITY_CUSTOMIZATION_PACK_SCHEMA: &str = "if2ai.identity-pack";
pub const IDENTITY_CUSTOMIZATION_PACK_VERSION: u32 = 1;

/// Partial override for a built-in Soul definition.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoulCustomization {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mission: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub core_principles: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_contract: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub non_negotiables: Option<Vec<String>>,
}

/// Partial override for a built-in Persona definition.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonaCustomization {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone_rules: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collaboration_rules: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_preferences: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedIdentityCustomizationPack {
    pub schema: String,
    pub version: u32,
    #[serde(default)]
    pub souls: BTreeMap<String, SoulCustomization>,
    #[serde(default)]
    pub personas: BTreeMap<String, PersonaCustomization>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyIdentityCustomizationPack {
    #[serde(default)]
    pub souls: BTreeMap<String, SoulCustomization>,
    #[serde(default)]
    pub personas: BTreeMap<String, PersonaCustomization>,
}

impl SoulCustomization {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.summary.is_none()
            && self.mission.is_none()
            && self.core_principles.is_none()
            && self.decision_contract.is_none()
            && self.non_negotiables.is_none()
    }
}

impl PersonaCustomization {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.summary.is_none()
            && self.tone_rules.is_none()
            && self.collaboration_rules.is_none()
            && self.output_preferences.is_none()
    }
}

/// Resolve the persisted custom identity pack path.
#[must_use]
pub fn default_identity_customization_pack_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(|home| {
            PathBuf::from(home)
                .join(".if2ai")
                .join("prompt")
                .join("identity-pack.json")
        })
        .unwrap_or_else(|| {
            PathBuf::from(".if2ai")
                .join("prompt")
                .join("identity-pack.json")
        })
}

/// Load the persisted identity customization pack.
pub fn read_identity_customization_pack() -> Result<IdentityCustomizationPack, String> {
    let path = default_identity_customization_pack_path();
    match std::fs::read_to_string(&path) {
        Ok(raw) => parse_identity_customization_pack_json(&raw),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(IdentityCustomizationPack::default())
        }
        Err(error) => Err(error.to_string()),
    }
}

/// Persist the identity customization pack to disk.
pub fn write_identity_customization_pack(
    pack: &IdentityCustomizationPack,
) -> Result<IdentityCustomizationPack, String> {
    let normalized = normalize_identity_customization_pack(pack.clone());
    let path = default_identity_customization_pack_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let persisted = PersistedIdentityCustomizationPack {
        schema: IDENTITY_CUSTOMIZATION_PACK_SCHEMA.to_string(),
        version: IDENTITY_CUSTOMIZATION_PACK_VERSION,
        souls: normalized.souls.clone(),
        personas: normalized.personas.clone(),
    };
    let json = serde_json::to_string_pretty(&persisted).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(normalized)
}

/// Merge built-in definitions with the saved customization pack.
#[must_use]
pub fn apply_identity_customization_pack(
    registry: &IdentityRegistry,
    pack: &IdentityCustomizationPack,
) -> IdentityRegistry {
    let souls = registry
        .souls()
        .map(|soul| {
            let customized = pack.souls.get(&soul.id).map_or_else(
                || soul.clone(),
                |override_item| apply_soul_override(soul, override_item),
            );
            (customized.id.clone(), customized)
        })
        .collect();
    let personas = registry
        .personas()
        .map(|persona| {
            let customized = pack.personas.get(&persona.id).map_or_else(
                || persona.clone(),
                |override_item| apply_persona_override(persona, override_item),
            );
            (customized.id.clone(), customized)
        })
        .collect();
    IdentityRegistry::from_parts(souls, personas)
}

/// Return true when the given Soul id has a persisted customization override.
#[must_use]
pub fn soul_has_customization(pack: &IdentityCustomizationPack, soul_id: &str) -> bool {
    pack.souls.contains_key(soul_id)
}

/// Return true when the given Persona id has a persisted customization override.
#[must_use]
pub fn persona_has_customization(pack: &IdentityCustomizationPack, persona_id: &str) -> bool {
    pack.personas.contains_key(persona_id)
}

fn parse_identity_customization_pack_json(raw: &str) -> Result<IdentityCustomizationPack, String> {
    let value: Value =
        serde_json::from_str(raw).map_err(|e| format!("identity pack JSON parse error: {e}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "identity pack must be a JSON object".to_string())?;

    if object.contains_key("schema") || object.contains_key("version") {
        let persisted: PersistedIdentityCustomizationPack =
            serde_json::from_value(value).map_err(|e| {
                format!("identity pack schema validation failed for versioned envelope: {e}")
            })?;
        if persisted.schema != IDENTITY_CUSTOMIZATION_PACK_SCHEMA {
            return Err(format!(
                "identity pack schema mismatch: expected `{}`, got `{}`",
                IDENTITY_CUSTOMIZATION_PACK_SCHEMA, persisted.schema
            ));
        }
        if persisted.version != IDENTITY_CUSTOMIZATION_PACK_VERSION {
            return Err(format!(
                "identity pack version `{}` is unsupported; expected version `{}`",
                persisted.version, IDENTITY_CUSTOMIZATION_PACK_VERSION
            ));
        }
        return Ok(normalize_identity_customization_pack(
            IdentityCustomizationPack {
                souls: persisted.souls,
                personas: persisted.personas,
            },
        ));
    }

    let legacy: LegacyIdentityCustomizationPack =
        serde_json::from_value(Value::Object(object.clone())).map_err(|e| {
            format!("identity pack schema validation failed for legacy envelope: {e}")
        })?;

    Ok(normalize_identity_customization_pack(
        IdentityCustomizationPack {
            souls: legacy.souls,
            personas: legacy.personas,
        },
    ))
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_optional_lines(value: Option<Vec<String>>) -> Option<Vec<String>> {
    value.and_then(|items| {
        let normalized: Vec<String> = items
            .into_iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect();
        if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        }
    })
}

/// Remove empty entries and trim content before persisting.
#[must_use]
pub fn normalize_identity_customization_pack(
    mut pack: IdentityCustomizationPack,
) -> IdentityCustomizationPack {
    pack.souls = pack
        .souls
        .into_iter()
        .filter_map(|(id, mut item)| {
            item.summary = normalize_optional_text(item.summary);
            item.mission = normalize_optional_text(item.mission);
            item.core_principles = normalize_optional_lines(item.core_principles);
            item.decision_contract = normalize_optional_text(item.decision_contract);
            item.non_negotiables = normalize_optional_lines(item.non_negotiables);
            (!item.is_empty()).then_some((id, item))
        })
        .collect();
    pack.personas = pack
        .personas
        .into_iter()
        .filter_map(|(id, mut item)| {
            item.summary = normalize_optional_text(item.summary);
            item.tone_rules = normalize_optional_lines(item.tone_rules);
            item.collaboration_rules = normalize_optional_lines(item.collaboration_rules);
            item.output_preferences = normalize_optional_lines(item.output_preferences);
            (!item.is_empty()).then_some((id, item))
        })
        .collect();
    pack
}

fn apply_soul_override(base: &SoulDefinition, override_item: &SoulCustomization) -> SoulDefinition {
    SoulDefinition {
        id: base.id.clone(),
        version: base.version.clone(),
        name: base.name.clone(),
        summary: override_item
            .summary
            .clone()
            .unwrap_or_else(|| base.summary.clone()),
        mission: override_item
            .mission
            .clone()
            .unwrap_or_else(|| base.mission.clone()),
        core_principles: override_item
            .core_principles
            .clone()
            .unwrap_or_else(|| base.core_principles.clone()),
        decision_contract: override_item
            .decision_contract
            .clone()
            .unwrap_or_else(|| base.decision_contract.clone()),
        non_negotiables: override_item
            .non_negotiables
            .clone()
            .unwrap_or_else(|| base.non_negotiables.clone()),
    }
}

fn apply_persona_override(
    base: &PersonaDefinition,
    override_item: &PersonaCustomization,
) -> PersonaDefinition {
    PersonaDefinition {
        id: base.id.clone(),
        soul_id: base.soul_id.clone(),
        version: base.version.clone(),
        name: base.name.clone(),
        summary: override_item
            .summary
            .clone()
            .unwrap_or_else(|| base.summary.clone()),
        tone_rules: override_item
            .tone_rules
            .clone()
            .unwrap_or_else(|| base.tone_rules.clone()),
        collaboration_rules: override_item
            .collaboration_rules
            .clone()
            .unwrap_or_else(|| base.collaboration_rules.clone()),
        output_preferences: override_item
            .output_preferences
            .clone()
            .unwrap_or_else(|| base.output_preferences.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::identity::registry::{
        IdentityRegistry, DEFAULT_SOUL_ID, STAFF_ARCHITECT_PERSONA_ID,
    };

    #[test]
    fn normalize_drops_empty_entries() {
        let mut pack = IdentityCustomizationPack::default();
        pack.personas.insert(
            STAFF_ARCHITECT_PERSONA_ID.to_string(),
            PersonaCustomization {
                summary: Some("   ".to_string()),
                tone_rules: Some(vec!["  ".to_string()]),
                collaboration_rules: None,
                output_preferences: None,
            },
        );

        let normalized = normalize_identity_customization_pack(pack);
        assert!(normalized.personas.is_empty());
    }

    #[test]
    fn apply_pack_overrides_selected_fields() {
        let registry = IdentityRegistry::builtin();
        let mut pack = IdentityCustomizationPack::default();
        pack.souls.insert(
            DEFAULT_SOUL_ID.to_string(),
            SoulCustomization {
                summary: Some("Custom summary".to_string()),
                mission: None,
                core_principles: Some(vec!["Custom principle".to_string()]),
                decision_contract: None,
                non_negotiables: None,
            },
        );
        pack.personas.insert(
            STAFF_ARCHITECT_PERSONA_ID.to_string(),
            PersonaCustomization {
                summary: Some("Custom persona".to_string()),
                tone_rules: Some(vec!["Custom tone".to_string()]),
                collaboration_rules: None,
                output_preferences: None,
            },
        );

        let customized = apply_identity_customization_pack(&registry, &pack);
        let soul = customized.soul(DEFAULT_SOUL_ID).expect("custom soul");
        let persona = customized
            .persona(STAFF_ARCHITECT_PERSONA_ID)
            .expect("custom persona");

        assert_eq!(soul.summary, "Custom summary");
        assert_eq!(soul.core_principles, vec!["Custom principle".to_string()]);
        assert_eq!(persona.summary, "Custom persona");
        assert_eq!(persona.tone_rules, vec!["Custom tone".to_string()]);
    }

    #[test]
    fn parses_legacy_pack_shape() {
        let raw = r#"
        {
          "souls": {
            "default": {
              "summary": "Legacy summary"
            }
          },
          "personas": {}
        }
        "#;

        let parsed = parse_identity_customization_pack_json(raw).expect("legacy pack should parse");
        assert_eq!(
            parsed
                .souls
                .get("default")
                .and_then(|item| item.summary.clone()),
            Some("Legacy summary".to_string())
        );
    }

    #[test]
    fn parses_versioned_pack_shape() {
        let raw = r#"
        {
          "schema": "if2ai.identity-pack",
          "version": 1,
          "souls": {
            "default": {
              "summary": "Versioned summary"
            }
          },
          "personas": {}
        }
        "#;

        let parsed =
            parse_identity_customization_pack_json(raw).expect("versioned pack should parse");
        assert_eq!(
            parsed
                .souls
                .get("default")
                .and_then(|item| item.summary.clone()),
            Some("Versioned summary".to_string())
        );
    }

    #[test]
    fn rejects_unknown_versioned_schema() {
        let raw = r#"
        {
          "schema": "if2ai.identity-pack.v999",
          "version": 1,
          "souls": {},
          "personas": {}
        }
        "#;

        let error = parse_identity_customization_pack_json(raw).expect_err("schema should reject");
        assert!(error.contains("schema mismatch"));
    }
}
