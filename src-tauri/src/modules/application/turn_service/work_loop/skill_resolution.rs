//! Skill plan resolution and skill candidate scoring helpers.

use std::path::Path;

use crate::modules::application::prompt_planner::{
    PromptBlockKind, PromptBlockSource, PromptContribution,
};
use crate::modules::runtime::contracts::agent_loop::{
    SkillResolutionCandidate, SkillResolutionPlan,
};
use crate::modules::runtime::prompt::{collect_skill_index_entries, SkillIndexEntry};
use crate::modules::tools::builtin::skill::{
    check_required_env_vars, local_review_skill_content, resolve_skill_config_block,
    resolve_skill_path,
};

use super::{AUTO_SKILL_TOOLS, push_unique};

/// Build the deterministic skill-resolution plan for one turn.
#[must_use]
pub fn resolve_skill_plan(
    workdir: &Path,
    user_message: &str,
    active_skill_ids: &[String],
    available_toolsets: &[String],
) -> SkillResolutionPlan {
    let entries = collect_skill_index_entries(workdir, Some(available_toolsets));
    let query_tokens = query_tokens(user_message);
    let mut candidates = entries
        .iter()
        .filter_map(|entry| {
            let score = score_skill_candidate(&entry.name, &entry.description, &query_tokens);
            (score > 0).then(|| skill_candidate(entry, "matched current request keywords", score))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.name.cmp(&right.name))
    });
    candidates.truncate(5);

    for active_skill_id in active_skill_ids {
        if candidates
            .iter()
            .any(|candidate| candidate.name.eq_ignore_ascii_case(active_skill_id))
        {
            continue;
        }
        if let Some(entry) = entries
            .iter()
            .find(|entry| entry.name.eq_ignore_ascii_case(active_skill_id))
        {
            candidates.push(skill_candidate(
                entry,
                "active skill selected for this turn",
                100,
            ));
        } else {
            candidates.push(SkillResolutionCandidate {
                skill_id: Some(active_skill_id.clone()),
                name: active_skill_id.clone(),
                source: "unknown".to_string(),
                reason: "active skill id was not present in the approved local index".to_string(),
                score: 0,
                trusted_source: false,
                auto_load_allowed: false,
                loaded: false,
                blocked_reason: Some(
                    "active skill id not found in approved local/builtin skill index".to_string(),
                ),
                load_warning: None,
                when_to_use: None,
                allowed_tools: Vec::new(),
                model_hint: None,
                activation_evidence: vec!["active skill id requested".to_string()],
            });
        }
    }

    let plan = SkillResolutionPlan {
        active_skill_ids: active_skill_ids.to_vec(),
        candidates,
        auto_discovery_tools: AUTO_SKILL_TOOLS
            .iter()
            .map(|name| (*name).to_string())
            .collect(),
        should_load_find_skills: asks_for_skill_discovery(user_message),
        remote_install_policy: "quarantine_requires_user_approval".to_string(),
        loaded_skill_names: Vec::new(),
        blocked_skill_names: Vec::new(),
        load_warnings: Vec::new(),
    };

    // DW-004 — fire-and-forget DK lookup advisory.
    // `resolve_skill_plan` is sync + has no `KnowledgeStore` handle
    // today; the actual `lookup_for_skill_resolution` call awaits
    // a deeper-wiring Pack that plumbs a process-wide store
    // singleton through. Until then we still emit a
    // `DomainKnowledge:lookup_advisory` envelope so the
    // observability pipeline (WU-001 → frontend store) is
    // exercised on every skill resolution call.
    spawn_dk_lookup_advisory(user_message);

    plan
}

/// DW-004 — Fire-and-forget DK lookup advisory.
///
/// Spawns a tokio task that calls `lookup_for_skill_resolution`
/// against an in-process `MockKnowledgeStore` placeholder + emits
/// the resulting `DomainKnowledge` envelope. Future deeper-wiring
/// Pack will replace the mock with the real
/// `Arc<dyn KnowledgeStore>` plumbed through `setup.rs` /
/// `desktop_host` state.
///
/// Failure-isolated: skipped silently when no current tokio runtime
/// is available (e.g. inside pure-sync test fixtures); the kill-
/// switch (`IF2AI_DISABLE_DK_LOOKUP=1`) is honored inside
/// `lookup_for_skill_resolution` itself.
fn spawn_dk_lookup_advisory(user_message: &str) {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
    };
    let query = user_message.to_string();
    handle.spawn(async move {
        use crate::modules::application::turn_service::dk_lookup_hook::lookup_for_skill_resolution;
        use crate::modules::runtime::contracts::common::{CorrelationIds, RuntimeEventType};
        use crate::modules::runtime::evolution_emitter::emit_evolution_event;

        // DW-004/WU-008: use the process-wide store so lookups see knowledge
        // upserted by the DK contributor in previous turns' finalize hooks.
        let store = crate::modules::skills::domain_knowledge::global_knowledge_store();
        let contributions = lookup_for_skill_resolution(&query, store.as_ref()).await;
        let payload = serde_json::json!({
            "entryId": "advisory",
            "kind": "task_sop",
            "source": "lookup",
            "matchedContributions": contributions.len(),
        });
        let _ = emit_evolution_event(
            None::<&tauri::AppHandle>,
            RuntimeEventType::DomainKnowledge,
            "lookup_advisory",
            CorrelationIds::default(),
            &payload,
            None,
        );
    });
}

/// Excerpt a SKILL.md body for token-saving auto-load mode.
///
/// Strategy:
///   1. If the body has a YAML frontmatter block (delimited by `---`),
///      preserve it verbatim — it's the trusted machine-readable contract
///      (name, version, etc.).
///   2. Take the first ~200 characters of the actual body content (post-
///      frontmatter) so the model sees enough to decide whether to call
///      `skill_view` for the full text.
///   3. Append a footer instructing the model how to fetch the rest:
///      `[truncated — call skill_view name="<skill>" for full body]`.
///
/// `skill_name` is interpolated into the footer so the model gets the exact
/// invocation it needs.
pub fn excerpt_skill_body(skill_name: &str, full: &str) -> String {
    const SNIPPET_CHARS: usize = 200;
    let trimmed = full.trim_start();

    let mut frontmatter_end: Option<usize> = None;
    if trimmed.starts_with("---\n") || trimmed.starts_with("---\r\n") {
        let after_open = if trimmed.starts_with("---\r\n") { 5 } else { 4 };
        if let Some(rel) = trimmed[after_open..].find("\n---\n") {
            frontmatter_end = Some(after_open + rel + "\n---\n".len());
        } else if let Some(rel) = trimmed[after_open..].find("\n---\r\n") {
            frontmatter_end = Some(after_open + rel + "\n---\r\n".len());
        }
    }

    let footer = format!(
        "\n\n[truncated — call `skill_view name=\"{}\"` for full body]",
        skill_name,
    );

    match frontmatter_end {
        Some(end) => {
            let frontmatter = &trimmed[..end];
            let body = trimmed[end..].trim_start();
            let snippet: String = body.chars().take(SNIPPET_CHARS).collect();
            format!("{}{}{}", frontmatter, snippet, footer)
        }
        None => {
            let snippet: String = trimmed.chars().take(SNIPPET_CHARS).collect();
            format!("{}{}", snippet, footer)
        }
    }
}

/// Load trusted skill contents into a dedicated prompt contribution.
#[must_use]
pub fn auto_load_trusted_skill_context(
    workdir: &Path,
    plan: &mut SkillResolutionPlan,
) -> Option<PromptContribution> {
    plan.loaded_skill_names.clear();
    plan.blocked_skill_names.clear();
    plan.load_warnings.clear();

    let mut loaded_sections = Vec::new();
    for candidate in &mut plan.candidates {
        candidate
            .skill_id
            .get_or_insert_with(|| candidate.name.clone());
        candidate.loaded = false;
        candidate.load_warning = None;
        candidate.activation_evidence = vec![candidate.reason.clone()];
        candidate.trusted_source = source_family_can_auto_load(&candidate.source);
        candidate.auto_load_allowed = candidate.trusted_source;
        candidate.blocked_reason = None;

        if !candidate.trusted_source {
            let reason = format!(
                "source `{}` is not eligible for automatic skill loading",
                candidate.source
            );
            candidate.auto_load_allowed = false;
            candidate.blocked_reason = Some(reason);
            push_unique(&mut plan.blocked_skill_names, candidate.name.clone());
            continue;
        }

        let skill_path = match resolve_skill_path(&candidate.name, workdir) {
            Ok(path) => path,
            Err(reason) => {
                candidate.auto_load_allowed = false;
                candidate.blocked_reason = Some(reason.clone());
                let warning = format!("skill `{}` was not auto-loaded: {reason}", candidate.name);
                candidate.load_warning = Some(warning.clone());
                push_unique(&mut plan.blocked_skill_names, candidate.name.clone());
                push_unique(&mut plan.load_warnings, warning);
                continue;
            }
        };

        let content = match std::fs::read_to_string(&skill_path) {
            Ok(content) => content,
            Err(error) => {
                let warning = format!(
                    "failed to load skill `{}` from {}: {error}",
                    candidate.name,
                    skill_path.display()
                );
                candidate.load_warning = Some(warning.clone());
                push_unique(&mut plan.blocked_skill_names, candidate.name.clone());
                push_unique(&mut plan.load_warnings, warning);
                continue;
            }
        };

        let metadata = parse_skill_runtime_metadata(&content);
        candidate.when_to_use = metadata.when_to_use.clone();
        candidate.allowed_tools = metadata.allowed_tools.clone();
        candidate.model_hint = metadata.model_hint.clone();
        if let Some(when_to_use) = metadata.when_to_use.as_ref() {
            candidate
                .activation_evidence
                .push(format!("frontmatter whenToUse: {when_to_use}"));
        }

        if let Err(reason) = local_review_skill_content(&content) {
            candidate.auto_load_allowed = false;
            candidate.blocked_reason = Some(reason.clone());
            let warning = format!(
                "skill `{}` failed local content review during auto-load: {reason}",
                candidate.name
            );
            candidate.load_warning = Some(warning.clone());
            push_unique(&mut plan.blocked_skill_names, candidate.name.clone());
            push_unique(&mut plan.load_warnings, warning);
            continue;
        }

        let env_warning = check_required_env_vars(&content);
        if let Some(warning) = env_warning.as_ref() {
            candidate.load_warning = Some(warning.clone());
            push_unique(&mut plan.load_warnings, warning.clone());
        }
        let config_block = resolve_skill_config_block(&candidate.name, &content, workdir);
        let metadata_block = skill_metadata_prompt_block(candidate);
        let autoload_mode = crate::modules::runtime::budget::skill_autoload_mode();
        let (header_line, body_section) = match autoload_mode {
            crate::modules::runtime::budget::SkillAutoloadMode::Disabled => {
                // Skip body injection entirely; only the active_skill_ids list
                // and metadata reach the model. The model must call `skill_view`
                // for any content. Still mark the skill as loaded so the plan
                // accounting reflects that we considered it.
                candidate.loaded = true;
                push_unique(&mut plan.loaded_skill_names, candidate.name.clone());
                continue;
            }
            crate::modules::runtime::budget::SkillAutoloadMode::Full => (
                "The full SKILL.md content is already loaded. Do not reload it with tools.",
                crate::modules::skills::escape_markdown_skill_section(&content),
            ),
            crate::modules::runtime::budget::SkillAutoloadMode::Excerpt => {
                let excerpted = excerpt_skill_body(&candidate.name, &content);
                (
                    "An excerpt of SKILL.md is shown below. Call `skill_view` for the full body if you need more.",
                    crate::modules::skills::escape_markdown_skill_section(&excerpted),
                )
            }
        };
        let mut section = format!(
            "## Skill: {} [{}]\n{}\n{}\n\n{}",
            candidate.name, candidate.source, header_line, metadata_block, body_section,
        );
        if let Some(warning) = env_warning {
            section.push_str("\n\n");
            section.push_str(&warning);
        }
        if !config_block.is_empty() {
            section.push_str("\n\n");
            section.push_str(&config_block);
        }

        candidate.loaded = true;
        push_unique(&mut plan.loaded_skill_names, candidate.name.clone());
        loaded_sections.push(section);
    }

    if loaded_sections.is_empty() {
        return None;
    }

    Some(PromptContribution {
        kind: PromptBlockKind::Skill,
        title: "Auto-loaded Skills".to_string(),
        body: format!(
            "[auto_loaded_skills]\nOnly the trusted skills below were loaded automatically.\n\n{}",
            loaded_sections.join("\n\n---\n\n")
        ),
        source: PromptBlockSource {
            subsystem: "skills".to_string(),
            reference: Some("turn_service.work_loop.auto_load".to_string()),
        },
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillRuntimeMetadata {
    pub when_to_use: Option<String>,
    pub allowed_tools: Vec<String>,
    pub model_hint: Option<String>,
}

/// Parse YAML-style frontmatter from a SKILL.md body to extract runtime metadata.
pub fn parse_skill_runtime_metadata(content: &str) -> SkillRuntimeMetadata {
    let Some(body) = content.strip_prefix("---") else {
        return SkillRuntimeMetadata::default();
    };
    let Some((header, _)) = body.split_once("---") else {
        return SkillRuntimeMetadata::default();
    };

    let mut metadata = SkillRuntimeMetadata::default();
    for line in header.lines() {
        let line = line.trim();
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim().trim_matches('"').trim_matches('\'');
        match key.trim() {
            "whenToUse" | "when_to_use" => metadata.when_to_use = Some(value.to_string()),
            "allowedTools" | "allowed_tools" => {
                metadata.allowed_tools = parse_frontmatter_list(value)
            }
            "modelHint" | "model_hint" | "model" => metadata.model_hint = Some(value.to_string()),
            _ => {}
        }
    }
    metadata
}

/// Parse a YAML-style inline list value from frontmatter.
pub fn parse_frontmatter_list(value: &str) -> Vec<String> {
    let inner = value.trim_start_matches('[').trim_end_matches(']');
    inner
        .split(',')
        .map(|part| part.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

/// Build the metadata prompt block for a skill candidate.
pub fn skill_metadata_prompt_block(candidate: &SkillResolutionCandidate) -> String {
    let mut lines = Vec::new();
    if let Some(when_to_use) = candidate.when_to_use.as_ref() {
        lines.push(format!("whenToUse: {when_to_use}"));
    }
    if !candidate.allowed_tools.is_empty() {
        lines.push(format!(
            "allowedTools: {}",
            candidate.allowed_tools.join(", ")
        ));
    }
    if let Some(model_hint) = candidate.model_hint.as_ref() {
        lines.push(format!("modelHint: {model_hint}"));
    }
    if !candidate.activation_evidence.is_empty() {
        lines.push(format!(
            "activationEvidence: {}",
            candidate.activation_evidence.join(" | ")
        ));
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("\nRuntime metadata:\n{}", lines.join("\n"))
    }
}

/// Build a `SkillResolutionCandidate` from a skill index entry.
pub fn skill_candidate(
    entry: &SkillIndexEntry,
    reason: impl Into<String>,
    score: u32,
) -> SkillResolutionCandidate {
    let trusted_source = source_family_can_auto_load(&entry.source);
    let reason = reason.into();
    SkillResolutionCandidate {
        skill_id: Some(entry.name.clone()),
        name: entry.name.clone(),
        source: entry.source.clone(),
        reason: reason.clone(),
        score,
        trusted_source,
        auto_load_allowed: trusted_source,
        loaded: false,
        blocked_reason: None,
        load_warning: None,
        when_to_use: None,
        allowed_tools: Vec::new(),
        model_hint: None,
        activation_evidence: vec![reason],
    }
}

/// Return true when a skill source family is trusted for automatic loading.
pub fn source_family_can_auto_load(source: &str) -> bool {
    matches!(source, "builtin" | "workspace" | "user")
}

/// Return true when the user message is asking for skill discovery.
pub fn asks_for_skill_discovery(user_message: &str) -> bool {
    let lower = user_message.to_lowercase();
    [
        "find skill",
        "discover skill",
        "recommend skill",
        "查找skill",
        "推荐skill",
        "找一个skill",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

/// Tokenize a user message for skill scoring.
pub fn query_tokens(user_message: &str) -> Vec<String> {
    user_message
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .map(str::to_lowercase)
        .filter(|token| token.chars().count() >= 4)
        .take(24)
        .collect()
}

/// Score a skill candidate against query tokens.
pub fn score_skill_candidate(name: &str, description: &str, query_tokens: &[String]) -> u32 {
    let haystack = format!("{} {}", name.to_lowercase(), description.to_lowercase());
    query_tokens
        .iter()
        .map(|token| {
            if name.to_lowercase().contains(token) {
                5
            } else if haystack.contains(token) {
                2
            } else {
                0
            }
        })
        .sum()
}
