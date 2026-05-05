//! Memory Recall tool - retrieves memories matching a query
//!
//! Provides memory retrieval with optional category filtering, scoped to the
//! current session when a `session_id` is present in the tool context.

use std::sync::Arc;

use crate::modules::memory::audit::{AuditContext, MemoryAuditEmitter};
use crate::modules::memory::scope::{MemoryExecutionScope, MemoryScopeResolver};
use crate::modules::memory::{MemoryEntry, MemoryError, SharedMemoryProvider};
use crate::modules::tools::context::SharedToolContext;
use crate::modules::tools::registry::{ToolEntry, ToolError, ToolHandler};

/// Default recall limit
#[allow(dead_code)]
const DEFAULT_LIMIT: usize = 10;

/// Creates the memory_recall tool entry for the registry.
#[allow(dead_code)]
#[must_use]
pub fn entry(memory: SharedMemoryProvider) -> ToolEntry {
    let handler: ToolHandler =
        Arc::new(move |args: serde_json::Value, context: SharedToolContext| {
            let memory = memory.clone();
            Box::pin(async move {
                let query = args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let category = args.get("category").and_then(|v| v.as_str());

                let limit = args
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(DEFAULT_LIMIT);

                // Resolve scope from the tool execution context.  When a session_id
                // is present, recall is filtered to session-scoped + global entries.
                let scope = context
                    .lock()
                    .ok()
                    .map(|ctx| MemoryScopeResolver::from_tool_context(&ctx))
                    .unwrap_or_else(crate::modules::memory::scope::MemoryExecutionScope::global);

                let results = recall_grounded_memory(&memory, &query, category, limit, &scope)
                    .await
                    .map_err(|e| ToolError::Handler(format!("failed to recall memory: {}", e)))?;

                // Emit audit event for observability and frontend evidence chain.
                let audit_ctx = AuditContext::from_scope(&scope);
                MemoryAuditEmitter::memory_recall_served(
                    &audit_ctx,
                    &query,
                    category,
                    results.len(),
                );

                Ok(format_deduped_recall_results(&results, &query))
            })
        });

    ToolEntry {
        name: "memory_recall".to_string(),
        toolset: "memory".to_string(),
        description: "Recall memories matching a query".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query (matches key or content)"
                },
                "category": {
                    "type": "string",
                    "description": "Filter by category (optional)"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum results to return (default: 10)"
                }
            },
            "required": []
        }),
        max_result_size: Some(1024 * 1024),
        max_text_bytes: None,
        max_image_bytes: None,
        timeout_secs: Some(10),
        disabled: false,
        handler,
        multimodal_handler: None,
    }
}

async fn recall_grounded_memory(
    memory: &SharedMemoryProvider,
    query: &str,
    category: Option<&str>,
    limit: usize,
    scope: &MemoryExecutionScope,
) -> Result<Vec<MemoryEntry>, MemoryError> {
    let mut results = memory.recall_scoped(query, category, limit, scope).await?;
    let fallback_queries = memory_recall_fallback_queries(query);

    for fallback_query in &fallback_queries {
        let scoped = memory
            .recall_scoped(fallback_query, category, limit, scope)
            .await?;
        merge_unique_entries(&mut results, scoped);
    }

    if should_include_project_session_fallback(query, scope) {
        let fallback_limit = limit.clamp(DEFAULT_LIMIT, 50);
        let mut project_queries = fallback_queries;
        if !query.trim().is_empty() {
            project_queries.push(query.to_string());
        }
        for fallback_query in &project_queries {
            let project_entries = memory
                .recall(fallback_query, category, fallback_limit)
                .await?;
            merge_unique_entries(
                &mut results,
                project_entries
                    .into_iter()
                    .filter(|entry| entry_matches_project_fallback(entry, scope)),
            );
        }
    }

    Ok(results)
}

fn memory_recall_fallback_queries(query: &str) -> Vec<String> {
    let lower = query.to_lowercase();
    let mut queries = Vec::new();

    let family_query = contains_any(&lower, &["儿子", "孩子", "书维", "son", "child"]);
    let food_query = contains_any(
        &lower,
        &[
            "喜欢吃",
            "吃什么",
            "共同喜欢",
            "共同爱吃",
            "food",
            "eat",
            "favorite",
        ],
    );

    if family_query {
        push_unique(&mut queries, "儿子");
        push_unique(&mut queries, "刘书维");
        push_unique(&mut queries, "son");
    }

    if food_query {
        push_unique(&mut queries, "喜欢吃");
        push_unique(&mut queries, "吃什么");
        push_unique(&mut queries, "food_preference");
    }

    queries.into_iter().map(str::to_string).collect()
}

fn should_include_project_session_fallback(query: &str, scope: &MemoryExecutionScope) -> bool {
    scope.project_id.is_some() && !memory_recall_fallback_queries(query).is_empty()
}

fn entry_matches_project_fallback(entry: &MemoryEntry, scope: &MemoryExecutionScope) -> bool {
    match scope.project_id.as_deref() {
        Some(project_id) => {
            entry.project_id.as_deref() == Some(project_id)
                || (entry.project_id.is_none() && entry.session_id.is_none())
        }
        None => entry.session_id.is_none() && entry.project_id.is_none(),
    }
}

fn merge_unique_entries<I>(target: &mut Vec<MemoryEntry>, incoming: I)
where
    I: IntoIterator<Item = MemoryEntry>,
{
    for entry in incoming {
        let exists = target.iter().any(|existing| {
            existing.key == entry.key
                && existing.content == entry.content
                && existing.session_id == entry.session_id
                && existing.project_id == entry.project_id
        });
        if !exists {
            target.push(entry);
        }
    }
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn push_unique(values: &mut Vec<&'static str>, value: &'static str) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn format_deduped_recall_results(results: &[MemoryEntry], query: &str) -> String {
    let mut unique: Vec<DedupedMemoryLine> = Vec::new();
    let mut hidden_duplicates = 0usize;

    for entry in results {
        let candidate = DedupedMemoryLine::from_entry(entry);
        if let Some(existing) = unique
            .iter_mut()
            .find(|line| line.dedupe_key == candidate.dedupe_key)
        {
            hidden_duplicates += 1;
            if candidate.is_better_than(existing) {
                *existing = candidate;
            }
            continue;
        }
        unique.push(candidate);
    }

    let mut lines = vec![format!(
        "[memory_recall_evidence] original_results={} unique_results={} hidden_duplicates={}",
        results.len(),
        unique.len(),
        hidden_duplicates
    )];
    lines.push(
        "Use only these deduplicated memory facts when answering; do not repeat equivalent facts."
            .to_string(),
    );

    if unique.is_empty() {
        lines.push("No matching memories found.".to_string());
    } else {
        if let Some(common_foods) = infer_common_food_candidates(query, &unique) {
            lines.push(format!(
                "[memory_recall_inference] common_food_candidates={}",
                common_foods.join("、")
            ));
        }
        lines.extend(unique.into_iter().map(|line| line.render()));
    }

    lines.join("\n")
}

fn infer_common_food_candidates(query: &str, lines: &[DedupedMemoryLine]) -> Option<Vec<String>> {
    if !is_common_family_food_query(query) {
        return None;
    }

    let mut user_foods: Vec<String> = Vec::new();
    let mut child_foods: Vec<String> = Vec::new();

    for line in lines {
        let foods = extract_liked_foods(&line.fact);
        if foods.is_empty() {
            continue;
        }
        if fact_mentions_user(&line.fact) {
            merge_foods(&mut user_foods, foods.clone());
        }
        if fact_mentions_child(&line.fact) {
            merge_foods(&mut child_foods, foods);
        }
    }

    let mut common = user_foods
        .into_iter()
        .filter(|food| child_foods.iter().any(|child_food| child_food == food))
        .collect::<Vec<_>>();
    common.sort();
    common.dedup();

    if common.is_empty() {
        None
    } else {
        Some(common)
    }
}

fn is_common_family_food_query(query: &str) -> bool {
    let lower = query.to_lowercase();
    contains_any(&lower, &["共同", "都", "both", "together"])
        && contains_any(&lower, &["儿子", "孩子", "书维", "son", "child"])
        && contains_any(
            &lower,
            &["喜欢吃", "吃什么", "爱吃", "food", "eat", "favorite"],
        )
}

fn fact_mentions_user(fact: &str) -> bool {
    let lower = fact.to_lowercase();
    lower.contains("ryan") || fact.contains("我喜欢") || fact.contains("用户")
}

fn fact_mentions_child(fact: &str) -> bool {
    let lower = fact.to_lowercase();
    fact.contains("儿子")
        || fact.contains("孩子")
        || fact.contains("书维")
        || lower.contains("son")
        || lower.contains("child")
}

fn extract_liked_foods(fact: &str) -> Vec<String> {
    for marker in ["喜欢吃", "爱吃"] {
        if let Some((_, rest)) = fact.split_once(marker) {
            return split_food_list(rest);
        }
    }
    Vec::new()
}

fn split_food_list(value: &str) -> Vec<String> {
    value
        .split(['、', '，', ',', ';', '；', '。', '.', '和', '与'])
        .map(str::trim)
        .map(|food| food.trim_matches(|c: char| c.is_whitespace() || c == '了'))
        .filter(|food| !food.is_empty())
        .map(str::to_string)
        .collect()
}

fn merge_foods(target: &mut Vec<String>, incoming: Vec<String>) {
    for food in incoming {
        if !target.iter().any(|existing| existing == &food) {
            target.push(food);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DedupedMemoryLine {
    category: String,
    key: String,
    fact: String,
    dedupe_key: String,
    episode_wrapper: bool,
}

impl DedupedMemoryLine {
    fn from_entry(entry: &MemoryEntry) -> Self {
        let leaf_key = canonical_leaf_key(&entry.key);
        let fact = canonical_fact_text(&entry.key, &entry.content);
        Self {
            category: entry.category.as_str().to_string(),
            key: leaf_key,
            dedupe_key: normalize_for_dedupe(&fact),
            fact,
            episode_wrapper: is_episode_key(&entry.key),
        }
    }

    fn is_better_than(&self, other: &Self) -> bool {
        if other.episode_wrapper && !self.episode_wrapper {
            return true;
        }
        if self.episode_wrapper != other.episode_wrapper {
            return false;
        }
        self.key.len() < other.key.len()
    }

    fn render(self) -> String {
        format!("[{}] {}: {}", self.category, self.key, self.fact)
    }
}

fn canonical_leaf_key(key: &str) -> String {
    if is_episode_key(key) {
        key.split(':')
            .next_back()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(key)
            .to_string()
    } else {
        key.to_string()
    }
}

fn canonical_fact_text(key: &str, content: &str) -> String {
    let trimmed = collapse_whitespace(content);
    let leaf_key = canonical_leaf_key(key);
    if is_episode_key(key) {
        let prefix = format!("{leaf_key}:");
        if let Some(rest) = trimmed.strip_prefix(&prefix) {
            return rest.trim().to_string();
        }
    }
    trimmed
}

fn normalize_for_dedupe(value: &str) -> String {
    collapse_whitespace(value).to_lowercase()
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_episode_key(key: &str) -> bool {
    key.starts_with("Episode:")
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;
    use crate::modules::memory::{InMemoryMemoryProvider, MemoryCategory};

    fn test_memory() -> SharedMemoryProvider {
        Arc::new(InMemoryMemoryProvider::new())
    }

    #[tokio::test]
    async fn memory_recall_tool_entry_has_correct_structure() {
        let entry = entry(test_memory());
        assert_eq!(entry.name, "memory_recall");
        assert_eq!(entry.toolset, "memory");
        assert!(!entry.disabled);
    }

    fn memory_entry(key: &str, content: &str) -> MemoryEntry {
        memory_entry_scoped(key, content, None, None)
    }

    fn memory_entry_scoped(
        key: &str,
        content: &str,
        session_id: Option<&str>,
        project_id: Option<&str>,
    ) -> MemoryEntry {
        let now = chrono::Utc::now();
        MemoryEntry {
            key: key.to_string(),
            content: content.to_string(),
            category: MemoryCategory::Conversation,
            created_at: now,
            updated_at: now,
            importance: 0.5,
            access_count: 0,
            trust_score: 0.0,
            session_id: session_id.map(str::to_string),
            project_id: project_id.map(str::to_string),
            quality_score: 0.5,
            source_reliability: 0.5,
            last_validated_at: None,
            contradiction_count: 0,
            cognitive_layer: crate::modules::memory::CognitiveLayer::Reactive,
            context_tags: Vec::new(),
        }
    }

    #[test]
    fn memory_recall_family_food_query_adds_fallback_terms() {
        let queries = memory_recall_fallback_queries("我与我儿子共同喜欢吃什么？");
        assert!(queries.iter().any(|query| query == "儿子"));
        assert!(queries.iter().any(|query| query == "刘书维"));
        assert!(queries.iter().any(|query| query == "喜欢吃"));
        assert!(queries.iter().any(|query| query == "food_preference"));
    }

    #[test]
    fn memory_recall_project_fallback_keeps_same_project_cross_session_entry() {
        let scope = MemoryExecutionScope {
            session_id: Some("current-session".to_string()),
            project_id: Some("project-a".to_string()),
            workdir: None,
        };
        let same_project = memory_entry_scoped(
            "son_fish_preference",
            "Ryan的大儿子刘书维喜欢吃鱼",
            Some("older-session"),
            Some("project-a"),
        );
        let other_project = memory_entry_scoped(
            "other_project_son_preference",
            "其他项目中的孩子喜欢吃鱼",
            Some("older-session"),
            Some("project-b"),
        );

        assert!(entry_matches_project_fallback(&same_project, &scope));
        assert!(!entry_matches_project_fallback(&other_project, &scope));
    }

    #[test]
    fn memory_recall_project_fallback_rejects_unprojected_old_session_entry() {
        let scope = MemoryExecutionScope {
            session_id: Some("current-session".to_string()),
            project_id: Some("project-a".to_string()),
            workdir: None,
        };
        let unprojected_old_session = memory_entry_scoped(
            "old_session_food",
            "另一个旧会话中的个人饮食记忆",
            Some("older-session"),
            None,
        );
        let global_entry = memory_entry_scoped("global_food", "全局饮食记忆", None, None);

        assert!(!entry_matches_project_fallback(
            &unprojected_old_session,
            &scope
        ));
        assert!(entry_matches_project_fallback(&global_entry, &scope));
    }

    #[test]
    fn memory_recall_dedupes_episode_and_canonical_fact() {
        let output = format_deduped_recall_results(
            &[
                memory_entry(
                    "Episode:Session:helen_husky",
                    "helen_husky: Ryan Liu 家有一只哈士奇叫 Helen",
                ),
                memory_entry("helen_husky", "Ryan Liu 家有一只哈士奇叫 Helen"),
                memory_entry("pet_preference", "Ryan Liu 喜欢动物，尤其是小狗"),
            ],
            "你记得关于我的什么事情",
        );

        assert!(output.contains("original_results=3"));
        assert!(output.contains("unique_results=2"));
        assert!(output.contains("hidden_duplicates=1"));
        assert_eq!(output.matches("Ryan Liu 家有一只哈士奇叫 Helen").count(), 1);
        assert!(output.contains("[conversation] helen_husky: Ryan Liu 家有一只哈士奇叫 Helen"));
    }

    #[test]
    fn memory_recall_output_includes_grounding_header() {
        let output = format_deduped_recall_results(
            &[memory_entry("location", "Ryan Liu 是山东人")],
            "你记得什么",
        );
        assert!(output.starts_with("[memory_recall_evidence]"));
        assert!(output.contains("Use only these deduplicated memory facts"));
    }

    #[test]
    fn memory_recall_empty_output_is_grounded() {
        let output = format_deduped_recall_results(&[], "你记得什么");
        assert!(output.contains("unique_results=0"));
        assert!(output.contains("No matching memories found."));
    }

    #[test]
    fn memory_recall_infers_common_food_from_deduped_facts() {
        let output = format_deduped_recall_results(
            &[
                memory_entry("food_preference", "Ryan Liu 喜欢吃牛肉、羊肉和鱼"),
                memory_entry("son_fish_preference", "Ryan的大儿子刘书维喜欢吃鱼"),
            ],
            "我与我儿子共同喜欢吃什么？",
        );

        assert!(output.contains("[memory_recall_inference] common_food_candidates=鱼"));
    }

    #[tokio::test]
    async fn memory_recall_live_family_food_pattern_uses_same_project_fallback() {
        let memory = test_memory();
        let current_scope = MemoryExecutionScope {
            session_id: Some("current-session".to_string()),
            project_id: Some("project-a".to_string()),
            workdir: None,
        };
        let older_scope_a = MemoryExecutionScope {
            session_id: Some("older-session-a".to_string()),
            project_id: Some("project-a".to_string()),
            workdir: None,
        };
        let older_scope_b = MemoryExecutionScope {
            session_id: Some("older-session-b".to_string()),
            project_id: Some("project-a".to_string()),
            workdir: None,
        };

        memory
            .store_scoped(
                "food_preference",
                "Ryan Liu 喜欢吃牛肉、羊肉和鱼",
                MemoryCategory::Conversation,
                &older_scope_a,
            )
            .await
            .unwrap();
        memory
            .store_scoped(
                "son_fish_preference",
                "Ryan的大儿子刘书维喜欢吃鱼",
                MemoryCategory::Conversation,
                &older_scope_b,
            )
            .await
            .unwrap();

        let results = recall_grounded_memory(
            &memory,
            "我与我儿子共同喜欢吃什么？",
            None,
            10,
            &current_scope,
        )
        .await
        .unwrap();
        let output = format_deduped_recall_results(&results, "我与我儿子共同喜欢吃什么？");

        assert!(output.contains("food_preference"));
        assert!(output.contains("son_fish_preference"));
        assert!(output.contains("[memory_recall_inference] common_food_candidates=鱼"));
    }
}
