//! FEAT-SE-001 — Skill sedimentation pipeline.
//!
//! Distills repeated tool-invocation patterns from a session's
//! `InputMessage` history into draft skill cards (YAML frontmatter +
//! markdown body). **Draft-only** by contract: nothing is written to
//! disk and `skills::manager` is not invoked. Downstream Packs will
//! consume `Vec<SkillDraft>` for dedup (SE-002), constitution
//! filtering (SE-003), vector indexing (SE-004), and finally
//! persistence in a future wiring Pack.
//!
//! Algorithm:
//! 1. Extract the ordered sequence of tool names called by the
//!    assistant across `messages`.
//! 2. Find every length ≥ 2 contiguous sub-sequence that occurs
//!    [`MIN_REPEATS`] (= 3) or more times.
//! 3. For each qualifying pattern, render a compact prompt and ask
//!    [`UtilityLlm`] to synthesize a skill draft.
//! 4. Parse the LLM response into [`SkillDraft`]; on any LLM /
//!    parsing failure, drop the pattern silently rather than crash.

#![allow(dead_code)]

pub mod dedup;

#[allow(unused_imports)]
pub use dedup::{
    cosine_similarity, dedup_drafts, DedupedSkill, Embedder, DEDUP_SIMILARITY_THRESHOLD,
};

use crate::modules::api::{InputContentBlock, InputMessage};
use crate::modules::memory::UtilityLlm;

/// Marker constant retained for FEAT-EVO-000 invariants.
pub const SEDIMENTATION_STUB_VERSION: &str = "FEAT-EVO-000";

/// Minimum number of repetitions a tool-call sub-sequence must have
/// before it qualifies as a sedimentation candidate. Pinned to 3 per
/// Pack spec; lower thresholds produce noisy false-positive skills.
pub const MIN_REPEATS: usize = 3;

/// Minimum length of a tool-sequence pattern. Length-1 patterns
/// (single tool used many times) are rarely worth sedimenting.
pub const MIN_PATTERN_LEN: usize = 2;

/// Maximum sequence length we consider. Longer chains are usually
/// task-specific and shouldn't be generalized.
pub const MAX_PATTERN_LEN: usize = 6;

/// Per-skill summarization budget. Keeps the LLM honest.
const SKILL_SYNTHESIS_MAX_TOKENS: u32 = 320;
const SKILL_SYNTHESIS_TEMPERATURE: f32 = 0.2;

/// Skill draft produced by [`extract_skill_drafts`].
///
/// `body` is a self-contained markdown document whose first line is
/// `---` (YAML frontmatter delimiter). Downstream consumers MAY parse
/// the frontmatter for metadata; pipelines that just want the prose
/// can skip the first `---`-bounded block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillDraft {
    /// Stable, kebab-case identifier suitable for filesystem use.
    pub name: String,
    /// One-line human description of when to use this skill.
    pub description: String,
    /// Full markdown body including YAML frontmatter.
    pub body: String,
    /// Indices into the original `messages` slice that contributed
    /// to this draft. Useful for trace UI / future reverse lookup.
    pub source_turns: Vec<usize>,
    /// Tool-name sequence that triggered the sedimentation.
    pub tool_sequence: Vec<String>,
}

const SYNTHESIS_SYSTEM_PROMPT: &str = concat!(
    "You write reusable skill cards for an AI agent. Given a tool-call ",
    "pattern observed multiple times in a successful conversation, output ",
    "ONE skill card in this exact format:\n",
    "---\n",
    "name: <kebab-case-name>\n",
    "description: <one-line when-to-use>\n",
    "---\n",
    "# <Title>\n\n",
    "## When to use\n<2-3 sentences>\n\n",
    "## Steps\n1. ...\n2. ...\n\n",
    "Reply with the markdown only. No preamble, no code fences.",
);

/// Extract zero or more skill drafts from a flat message stream.
///
/// Pure function (modulo `llm.complete` calls): same input + same
/// LLM responses → same output. Order of returned drafts mirrors
/// pattern-discovery order so callers can rely on it.
pub async fn extract_skill_drafts(
    messages: &[InputMessage],
    llm: &dyn UtilityLlm,
) -> Vec<SkillDraft> {
    if messages.is_empty() {
        return Vec::new();
    }
    let tool_seq = collect_tool_sequence(messages);
    if tool_seq.len() < MIN_PATTERN_LEN * MIN_REPEATS {
        return Vec::new();
    }

    let patterns = find_repeated_patterns(&tool_seq);
    if patterns.is_empty() {
        return Vec::new();
    }

    let mut drafts: Vec<SkillDraft> = Vec::with_capacity(patterns.len());
    for pattern in patterns {
        let user_prompt = render_synthesis_prompt(&pattern.tools, pattern.repeat_count);
        let body = match llm
            .complete(
                SYNTHESIS_SYSTEM_PROMPT,
                &user_prompt,
                SKILL_SYNTHESIS_MAX_TOKENS,
                SKILL_SYNTHESIS_TEMPERATURE,
            )
            .await
        {
            Ok(text) if !text.trim().is_empty() => text,
            Ok(_) => {
                tracing::debug!("[sedimentation] LLM returned empty body; skipping pattern");
                continue;
            }
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "[sedimentation] LLM error during skill synthesis; skipping pattern"
                );
                continue;
            }
        };

        let body = ensure_frontmatter(&body, &pattern.tools);
        let (name, description) =
            parse_frontmatter(&body).unwrap_or_else(|| default_metadata_for(&pattern.tools));
        if name.is_empty() || description.is_empty() {
            tracing::debug!("[sedimentation] empty name/description after parse; skipping");
            continue;
        }

        drafts.push(SkillDraft {
            name,
            description,
            body,
            source_turns: pattern.source_turn_indices.clone(),
            tool_sequence: pattern.tools.clone(),
        });
    }
    drafts
}

#[derive(Debug, Clone)]
struct RepeatedPattern {
    tools: Vec<String>,
    repeat_count: usize,
    /// Message indices where each occurrence STARTS.
    source_turn_indices: Vec<usize>,
}

fn collect_tool_sequence(messages: &[InputMessage]) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (idx, msg) in messages.iter().enumerate() {
        for block in &msg.content {
            if let InputContentBlock::ToolUse { name, .. } = block {
                out.push((idx, name.clone()));
            }
        }
    }
    out
}

fn find_repeated_patterns(seq: &[(usize, String)]) -> Vec<RepeatedPattern> {
    let names: Vec<&str> = seq.iter().map(|(_, n)| n.as_str()).collect();
    let mut found: Vec<RepeatedPattern> = Vec::new();

    let max_len = MAX_PATTERN_LEN.min(names.len() / MIN_REPEATS);
    for pat_len in MIN_PATTERN_LEN..=max_len {
        let mut seen_starts: Vec<usize> = Vec::new();
        let mut i = 0;
        while i + pat_len <= names.len() {
            let pattern = &names[i..i + pat_len];
            if seen_starts
                .iter()
                .any(|&s| s + pat_len <= names.len() && &names[s..s + pat_len] == pattern)
            {
                i += 1;
                continue;
            }

            let mut occurrences: Vec<usize> = Vec::new();
            let mut j = 0;
            while j + pat_len <= names.len() {
                if &names[j..j + pat_len] == pattern {
                    occurrences.push(j);
                    j += pat_len;
                } else {
                    j += 1;
                }
            }
            if occurrences.len() >= MIN_REPEATS {
                let source_turn_indices: Vec<usize> =
                    occurrences.iter().map(|&start| seq[start].0).collect();
                found.push(RepeatedPattern {
                    tools: pattern.iter().map(|s| s.to_string()).collect(),
                    repeat_count: occurrences.len(),
                    source_turn_indices,
                });
                seen_starts.push(i);
            }
            i += 1;
        }
    }

    found.sort_by_key(|p| (std::cmp::Reverse(p.repeat_count), p.tools.len()));
    if found.len() > 8 {
        found.truncate(8);
    }
    found
}

fn render_synthesis_prompt(tools: &[String], repeats: usize) -> String {
    format!(
        "Observed tool sequence: [{}]\nRepeated {} times across the session.\nWrite the skill card.",
        tools.join(" → "),
        repeats
    )
}

fn ensure_frontmatter(body: &str, tools: &[String]) -> String {
    let trimmed = body.trim_start();
    if trimmed.starts_with("---") {
        return trimmed.to_string();
    }
    let fallback = default_metadata_for(tools);
    format!(
        "---\nname: {}\ndescription: {}\n---\n{}",
        fallback.0, fallback.1, trimmed
    )
}

fn parse_frontmatter(body: &str) -> Option<(String, String)> {
    let body = body.trim_start();
    let rest = body.strip_prefix("---")?;
    let end_idx = rest.find("\n---")?;
    let block = &rest[..end_idx];
    let mut name = String::new();
    let mut description = String::new();
    for line in block.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("name:") {
            name = value.trim().trim_matches('"').to_string();
        } else if let Some(value) = line.strip_prefix("description:") {
            description = value.trim().trim_matches('"').to_string();
        }
    }
    Some((name, description))
}

fn default_metadata_for(tools: &[String]) -> (String, String) {
    let slug = tools
        .iter()
        .map(|t| t.replace('_', "-").to_lowercase())
        .collect::<Vec<_>>()
        .join("-then-");
    let name = format!("auto-{slug}");
    let description = format!(
        "Repeats the {} tool sequence observed during a successful task.",
        tools.join(" → ")
    );
    (name, description)
}
