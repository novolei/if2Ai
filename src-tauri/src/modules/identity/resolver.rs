use super::definition::{IdentitySource, ResolvedIdentity};
use super::registry::IdentityRegistry;
use super::settings::{IdentitySettings, SessionIdentityOverride};

/// Resolution output plus non-fatal warnings about invalid ids or
/// persona/soul mismatches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityResolution {
    pub resolved: ResolvedIdentity,
    pub warnings: Vec<String>,
}

/// Resolve the effective identity for a turn.
///
/// Precedence:
/// 1. Session override
/// 2. Global defaults
/// 3. Built-in fallback
#[must_use]
pub fn resolve_identity(
    registry: &IdentityRegistry,
    defaults: &IdentitySettings,
    session_override: Option<&SessionIdentityOverride>,
) -> IdentityResolution {
    let mut warnings = Vec::new();
    let session_soul = session_override.and_then(|override_| override_.soul_id.as_deref());
    let session_persona = session_override.and_then(|override_| override_.persona_id.as_deref());

    let (soul_id, source) = if let Some(soul_id) = session_soul {
        if registry.soul(soul_id).is_some() {
            (soul_id.to_string(), IdentitySource::SessionOverride)
        } else {
            warnings.push(format!(
                "session override soul_id '{soul_id}' was not found; falling back"
            ));
            fallback_soul(registry, defaults, &mut warnings)
        }
    } else {
        fallback_soul(registry, defaults, &mut warnings)
    };

    let soul = registry
        .soul(&soul_id)
        .expect("resolved soul id must exist in registry");

    let persona_candidate = session_persona.or(defaults.default_persona_id.as_deref());

    let (persona_id, persona_version) = match persona_candidate {
        Some(persona_id) => match registry.persona(persona_id) {
            Some(persona) if persona.soul_id == soul.id => {
                (Some(persona.id.clone()), Some(persona.version.clone()))
            }
            Some(persona) => {
                warnings.push(format!(
                    "persona '{}' belongs to soul '{}', not '{}'; dropping persona",
                    persona.id, persona.soul_id, soul.id
                ));
                (None, None)
            }
            None => {
                warnings.push(format!(
                    "persona_id '{persona_id}' was not found; dropping persona"
                ));
                (None, None)
            }
        },
        None => (None, None),
    };

    IdentityResolution {
        resolved: ResolvedIdentity {
            soul_id: soul.id.clone(),
            soul_version: soul.version.clone(),
            persona_id,
            persona_version,
            source,
        },
        warnings,
    }
}

fn fallback_soul(
    registry: &IdentityRegistry,
    defaults: &IdentitySettings,
    warnings: &mut Vec<String>,
) -> (String, IdentitySource) {
    if let Some(default_soul_id) = defaults.default_soul_id.as_deref() {
        if registry.soul(default_soul_id).is_some() {
            return (default_soul_id.to_string(), IdentitySource::GlobalDefault);
        }
        warnings.push(format!(
            "default soul_id '{default_soul_id}' was not found; using built-in fallback"
        ));
    }

    (
        registry.default_soul_id().to_string(),
        IdentitySource::BuiltInFallback,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::identity::settings::{IdentitySettings, SessionIdentityOverride};
    use crate::modules::identity::{PersonaDefinition, SoulDefinition};
    use std::collections::BTreeMap;

    #[test]
    fn session_override_wins_over_global_default() {
        let registry = IdentityRegistry::builtin();
        let defaults = IdentitySettings {
            default_soul_id: Some("if2ai-core".to_string()),
            default_persona_id: Some("staff-architect".to_string()),
            ..IdentitySettings::default()
        };
        let session = SessionIdentityOverride {
            soul_id: Some("if2ai-core".to_string()),
            persona_id: Some("execution-partner".to_string()),
        };

        let resolution = resolve_identity(&registry, &defaults, Some(&session));
        assert_eq!(resolution.resolved.source, IdentitySource::SessionOverride);
        assert_eq!(
            resolution.resolved.persona_id.as_deref(),
            Some("execution-partner")
        );
        assert!(resolution.warnings.is_empty());
    }

    #[test]
    fn mismatched_persona_is_dropped() {
        let souls = BTreeMap::from([
            (
                "if2ai-core".to_string(),
                SoulDefinition {
                    id: "if2ai-core".to_string(),
                    version: "1".to_string(),
                    name: "If2Ai Core".to_string(),
                    summary: "test".to_string(),
                    mission: "test".to_string(),
                    core_principles: Vec::new(),
                    decision_contract: "test".to_string(),
                    non_negotiables: Vec::new(),
                },
            ),
            (
                "other-soul".to_string(),
                SoulDefinition {
                    id: "other-soul".to_string(),
                    version: "1".to_string(),
                    name: "Other Soul".to_string(),
                    summary: "test".to_string(),
                    mission: "test".to_string(),
                    core_principles: Vec::new(),
                    decision_contract: "test".to_string(),
                    non_negotiables: Vec::new(),
                },
            ),
        ]);
        let personas = BTreeMap::from([(
            "foreign-persona".to_string(),
            PersonaDefinition {
                id: "foreign-persona".to_string(),
                soul_id: "other-soul".to_string(),
                version: "1".to_string(),
                name: "Foreign Persona".to_string(),
                summary: "test".to_string(),
                tone_rules: Vec::new(),
                collaboration_rules: Vec::new(),
                output_preferences: Vec::new(),
                avatar_id: None,
            },
        )]);
        let registry = IdentityRegistry::from_parts(souls, personas);
        let defaults = IdentitySettings {
            default_soul_id: Some("if2ai-core".to_string()),
            default_persona_id: Some("staff-architect".to_string()),
            ..IdentitySettings::default()
        };
        let session = SessionIdentityOverride {
            soul_id: Some("if2ai-core".to_string()),
            persona_id: Some("foreign-persona".to_string()),
        };

        let resolution = resolve_identity(&registry, &defaults, Some(&session));
        assert_eq!(resolution.resolved.persona_id, None);
        assert_eq!(resolution.resolved.soul_id, "if2ai-core");
        assert_eq!(resolution.warnings.len(), 1);
    }

    #[test]
    fn invalid_identity_falls_back_safely() {
        let registry = IdentityRegistry::builtin();
        let defaults = IdentitySettings {
            default_soul_id: Some("missing-soul".to_string()),
            default_persona_id: Some("missing-persona".to_string()),
            ..IdentitySettings::default()
        };

        let resolution = resolve_identity(&registry, &defaults, None);
        assert_eq!(resolution.resolved.soul_id, "if2ai-core");
        assert_eq!(resolution.resolved.source, IdentitySource::BuiltInFallback);
        assert_eq!(resolution.resolved.persona_id, None);
        assert_eq!(resolution.warnings.len(), 2);
    }

    #[test]
    fn global_default_is_used_when_valid() {
        let registry = IdentityRegistry::builtin();
        let defaults = IdentitySettings {
            default_soul_id: Some("if2ai-core".to_string()),
            default_persona_id: Some("staff-architect".to_string()),
            ..IdentitySettings::default()
        };

        let resolution = resolve_identity(&registry, &defaults, None);
        assert_eq!(resolution.resolved.source, IdentitySource::GlobalDefault);
        assert_eq!(resolution.resolved.soul_id, "if2ai-core");
        assert_eq!(
            resolution.resolved.persona_id.as_deref(),
            Some("staff-architect")
        );
    }
}
