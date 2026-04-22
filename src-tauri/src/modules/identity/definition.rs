use serde::{Deserialize, Serialize};

/// Stable, durable agent identity describing long-lived behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoulDefinition {
    pub id: String,
    pub version: String,
    pub name: String,
    pub summary: String,
    pub mission: String,
    pub core_principles: Vec<String>,
    pub decision_contract: String,
    pub non_negotiables: Vec<String>,
}

/// Session- or task-level interaction layer attached to a single soul.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonaDefinition {
    pub id: String,
    pub soul_id: String,
    pub version: String,
    pub name: String,
    pub summary: String,
    pub tone_rules: Vec<String>,
    pub collaboration_rules: Vec<String>,
    pub output_preferences: Vec<String>,
}

/// Where the effective identity came from after resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentitySource {
    BuiltInFallback,
    GlobalDefault,
    SessionOverride,
}

/// Effective identity used by one turn after precedence + validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedIdentity {
    pub soul_id: String,
    pub soul_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persona_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persona_version: Option<String>,
    pub source: IdentitySource,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitions_round_trip() {
        let soul = SoulDefinition {
            id: "if2ai-core".to_string(),
            version: "1".to_string(),
            name: "If2Ai Core".to_string(),
            summary: "Stable durable identity".to_string(),
            mission: "Help users accomplish ambitious work safely".to_string(),
            core_principles: vec!["truthful".to_string(), "actionable".to_string()],
            decision_contract: "Prefer verification over confident guessing".to_string(),
            non_negotiables: vec!["do not hide uncertainty".to_string()],
        };
        let persona = PersonaDefinition {
            id: "staff-architect".to_string(),
            soul_id: soul.id.clone(),
            version: "1".to_string(),
            name: "Staff Architect".to_string(),
            summary: "Structured, tradeoff-oriented collaboration".to_string(),
            tone_rules: vec!["clear".to_string()],
            collaboration_rules: vec!["name risks".to_string()],
            output_preferences: vec!["summary first".to_string()],
        };
        let resolved = ResolvedIdentity {
            soul_id: soul.id.clone(),
            soul_version: soul.version.clone(),
            persona_id: Some(persona.id.clone()),
            persona_version: Some(persona.version.clone()),
            source: IdentitySource::GlobalDefault,
        };

        let soul_json = serde_json::to_string(&soul).expect("serialize soul");
        let persona_json = serde_json::to_string(&persona).expect("serialize persona");
        let resolved_json = serde_json::to_string(&resolved).expect("serialize resolved");

        assert_eq!(
            serde_json::from_str::<SoulDefinition>(&soul_json).expect("deserialize soul"),
            soul
        );
        assert_eq!(
            serde_json::from_str::<PersonaDefinition>(&persona_json).expect("deserialize persona"),
            persona
        );
        assert_eq!(
            serde_json::from_str::<ResolvedIdentity>(&resolved_json).expect("deserialize resolved"),
            resolved
        );
    }
}
