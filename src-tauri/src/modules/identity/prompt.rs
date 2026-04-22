use super::definition::{PersonaDefinition, SoulDefinition};

/// Render a durable Soul definition into a stable prompt block.
#[must_use]
pub fn render_soul_block(soul: &SoulDefinition) -> String {
    let mut lines = vec![
        format!("# Soul: {}", soul.name),
        format!("- Soul ID: {}", soul.id),
        format!("- Soul Version: {}", soul.version),
        format!("- Summary: {}", soul.summary),
        String::new(),
        "## Mission".to_string(),
        soul.mission.clone(),
    ];

    if !soul.core_principles.is_empty() {
        lines.push(String::new());
        lines.push("## Core Principles".to_string());
        lines.extend(soul.core_principles.iter().map(|item| format!("- {item}")));
    }

    lines.push(String::new());
    lines.push("## Decision Contract".to_string());
    lines.push(soul.decision_contract.clone());

    if !soul.non_negotiables.is_empty() {
        lines.push(String::new());
        lines.push("## Non-negotiables".to_string());
        lines.extend(soul.non_negotiables.iter().map(|item| format!("- {item}")));
    }

    lines.join("\n")
}

/// Render a Persona definition into a stable prompt block.
#[must_use]
pub fn render_persona_block(persona: &PersonaDefinition) -> String {
    let mut lines = vec![
        format!("# Persona: {}", persona.name),
        format!("- Persona ID: {}", persona.id),
        format!("- Persona Version: {}", persona.version),
        format!("- Soul ID: {}", persona.soul_id),
        format!("- Summary: {}", persona.summary),
    ];

    if !persona.tone_rules.is_empty() {
        lines.push(String::new());
        lines.push("## Tone Rules".to_string());
        lines.extend(persona.tone_rules.iter().map(|item| format!("- {item}")));
    }

    if !persona.collaboration_rules.is_empty() {
        lines.push(String::new());
        lines.push("## Collaboration Rules".to_string());
        lines.extend(
            persona
                .collaboration_rules
                .iter()
                .map(|item| format!("- {item}")),
        );
    }

    if !persona.output_preferences.is_empty() {
        lines.push(String::new());
        lines.push("## Output Preferences".to_string());
        lines.extend(
            persona
                .output_preferences
                .iter()
                .map(|item| format!("- {item}")),
        );
    }

    lines.join("\n")
}

/// Render an "Identity Naming" prompt block driven by the global
/// `agent_name` / `user_name` settings. The block is intentionally short
/// and bilingual: Chinese carries the relational warmth, English carries
/// the precise behavioral contract. Returns `None` when no agent name is
/// configured, so the planner can skip the block entirely (saves tokens
/// and avoids polluting prompts that intentionally stay anonymous).
#[must_use]
pub fn render_identity_naming_block(
    agent_name: Option<&str>,
    user_name: Option<&str>,
) -> Option<String> {
    let agent = agent_name.map(str::trim).filter(|s| !s.is_empty())?;
    let user = user_name.map(str::trim).filter(|s| !s.is_empty());

    let mut lines = vec!["# Identity Naming".to_string()];

    match user {
        Some(user) => {
            lines.push(format!("- 你叫 {agent}，是 {user} 的个人助手。"));
            lines.push(format!(
                "- You are {agent}, {user}'s personal assistant."
            ));
        }
        None => {
            lines.push(format!("- 你叫 {agent}。"));
            lines.push(format!("- You are {agent}."));
        }
    }
    lines.push(
        "- 不论当前激活哪个 Persona（表达模式），你的名字、记忆、对用户的承诺都不变。Persona 切换的是「状态」，不是「身份」。".to_string(),
    );
    lines.push(
        "- Your name, memory, and commitment to the user persist across persona switches. A persona changes your *mode*, not your *identity*.".to_string(),
    );
    lines.push(format!(
        "- 当被问到「你是谁 / 你叫什么」时，回答「{agent}」（必要时简短补充当前以哪个 Persona 模式协作）。"
    ));
    lines.push(format!(
        "- When asked who you are or your name, answer \"{agent}\" (briefly mention the active persona mode if context calls for it)."
    ));

    Some(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soul_block_renders_stable_sections() {
        let soul = SoulDefinition {
            id: "if2ai-core".to_string(),
            version: "1".to_string(),
            name: "If2Ai Core".to_string(),
            summary: "Durable agent identity".to_string(),
            mission: "Help users move work forward".to_string(),
            core_principles: vec!["Tell the truth".to_string()],
            decision_contract: "Prefer verification".to_string(),
            non_negotiables: vec!["Do not fake certainty".to_string()],
        };

        let rendered = render_soul_block(&soul);
        assert!(rendered.contains("# Soul: If2Ai Core"));
        assert!(rendered.contains("## Mission"));
        assert!(rendered.contains("## Core Principles"));
        assert!(rendered.contains("## Non-negotiables"));
    }

    #[test]
    fn identity_naming_block_skipped_when_no_agent_name() {
        assert!(render_identity_naming_block(None, Some("RL")).is_none());
        assert!(render_identity_naming_block(Some("   "), Some("RL")).is_none());
        assert!(render_identity_naming_block(Some(""), Some("RL")).is_none());
    }

    #[test]
    fn identity_naming_block_renders_with_agent_only() {
        let block = render_identity_naming_block(Some("Asa"), None).expect("should render");
        assert!(block.contains("# Identity Naming"));
        assert!(block.contains("你叫 Asa。"));
        assert!(block.contains("You are Asa."));
        // Persona-mode anchor must appear so the LLM does not adopt the
        // persona's "name" as its true identity.
        assert!(block.contains("Persona") || block.contains("persona"));
    }

    #[test]
    fn identity_naming_block_includes_user_when_present() {
        let block = render_identity_naming_block(Some("Asa"), Some("RL")).expect("should render");
        assert!(block.contains("RL 的个人助手"));
        assert!(block.contains("RL's personal assistant"));
    }

    #[test]
    fn identity_naming_block_trims_whitespace() {
        let block =
            render_identity_naming_block(Some("  Asa  "), Some("  RL  ")).expect("should render");
        assert!(block.contains("Asa"));
        assert!(!block.contains("  Asa"));
        assert!(block.contains("RL"));
    }

    #[test]
    fn persona_block_renders_stable_sections() {
        let persona = PersonaDefinition {
            id: "staff-architect".to_string(),
            soul_id: "if2ai-core".to_string(),
            version: "1".to_string(),
            name: "Staff Architect".to_string(),
            summary: "Architectural collaboration".to_string(),
            tone_rules: vec!["Be calm".to_string()],
            collaboration_rules: vec!["Name tradeoffs".to_string()],
            output_preferences: vec!["Recommendation first".to_string()],
        };

        let rendered = render_persona_block(&persona);
        assert!(rendered.contains("# Persona: Staff Architect"));
        assert!(rendered.contains("## Tone Rules"));
        assert!(rendered.contains("## Collaboration Rules"));
        assert!(rendered.contains("## Output Preferences"));
    }
}
