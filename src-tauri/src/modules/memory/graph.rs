//! Memory knowledge graph engine — provides graph traversal, relationship
//! discovery, and graph-enhanced retrieval over the `memory_links` table.
//!
//! # Architecture
//!
//! [`MemoryGraph`] is a thin coordination layer that sits *above* the
//! [`MemoryProvider`] trait.  It does not own storage; instead it composes
//! the provider's `get_outgoing_links` / `get_incoming_links` /
//! `graph_traverse` / `recall` / `create_link` primitives into higher-level
//! graph operations:
//!
//! - **graph_recall** — BFS traversal from a seed key with depth & type filters.
//! - **find_related** — semantic recall + link hydration (graph-enhanced search).
//! - **discover_links** — LLM- or rule-based relationship mining.
//! - **get_neighborhood** — structured 1-hop view of a memory node.

use crate::modules::memory::llm::UtilityLlm;
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::{
    GraphNeighborhood, GraphNode, MemoryEntry, MemoryError, MemoryLink, SharedMemoryProvider,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tracing::{debug, warn};

/// Maximum allowed traversal depth — prevents runaway queries.
const MAX_DEPTH: usize = 5;

/// Default traversal depth when none is specified.
const DEFAULT_DEPTH: usize = 2;

/// Bigram similarity threshold for rule-based link discovery.
const BIGRAM_SIMILARITY_THRESHOLD: f64 = 0.6;

/// 记忆知识图谱引擎 — 提供图遍历、关系发现和图增强检索。
///
/// Holds a shared reference to the underlying [`MemoryProvider`] and an
/// optional [`UtilityLlm`] for LLM-assisted relationship discovery.
pub struct MemoryGraph {
    provider: SharedMemoryProvider,
    llm: Option<Arc<dyn UtilityLlm>>,
}

impl MemoryGraph {
    /// Create a new graph engine with the given provider.
    #[must_use]
    pub fn new(provider: SharedMemoryProvider) -> Self {
        Self {
            provider,
            llm: None,
        }
    }

    /// Attach a [`UtilityLlm`] for LLM-assisted link discovery.
    ///
    /// When set, [`Self::discover_links`] uses LLM to judge potential
    /// relationships between memory pairs.  When `None`, it falls back
    /// to bigram similarity.
    #[must_use]
    pub fn with_llm(mut self, llm: Arc<dyn UtilityLlm>) -> Self {
        self.llm = Some(llm);
        self
    }

    /// BFS graph traversal from a seed memory key.
    ///
    /// Explores up to `depth` hops (clamped to [`MAX_DEPTH`]).  When
    /// `link_types` is non-empty, only those link types are followed.
    /// Each discovered node is hydrated with its full [`MemoryEntry`].
    /// Already-visited keys are never re-enqueued (cycle-safe).
    pub async fn graph_recall(
        &self,
        seed_key: &str,
        depth: Option<usize>,
        link_types: Option<&[&str]>,
    ) -> Result<Vec<GraphNode>, MemoryError> {
        let depth = depth.unwrap_or(DEFAULT_DEPTH).min(MAX_DEPTH);
        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();
        let mut result: Vec<GraphNode> = Vec::new();

        queue.push_back((seed_key.to_string(), 0));

        while let Some((key, d)) = queue.pop_front() {
            if d > depth || visited.contains(&key) {
                continue;
            }
            visited.insert(key.clone());

            // Gather outgoing and incoming links
            let outgoing = self.provider.get_outgoing_links(&key, None).await?;
            let incoming = self.provider.get_incoming_links(&key, None).await?;

            // Filter by link_types if specified
            let filter_link = |link: &MemoryLink| -> bool {
                match link_types {
                    Some(types) => types.contains(&link.link_type.as_str()),
                    None => true,
                }
            };

            let mut node_links: Vec<MemoryLink> = Vec::new();
            for link in &outgoing {
                if filter_link(link) {
                    node_links.push(link.clone());
                    if d < depth && !visited.contains(&link.target_key) {
                        queue.push_back((link.target_key.clone(), d + 1));
                    }
                }
            }
            for link in &incoming {
                if filter_link(link) {
                    node_links.push(link.clone());
                    if d < depth && !visited.contains(&link.source_key) {
                        queue.push_back((link.source_key.clone(), d + 1));
                    }
                }
            }

            // Hydrate the entry
            let entry = self.provider.get_by_key(&key).await?;

            result.push(GraphNode {
                key,
                depth: d,
                entry,
                links: node_links,
            });
        }

        Ok(result)
    }

    /// Graph-enhanced recall: semantic search + link hydration.
    ///
    /// 1. Runs `provider.recall_scoped()` to find the top-`limit` entries
    ///    matching `query`.
    /// 2. For each result, fetches all outgoing and incoming links.
    /// 3. Returns `(MemoryEntry, Vec<MemoryLink>)` tuples.
    pub async fn find_related(
        &self,
        query: &str,
        scope: &MemoryExecutionScope,
        limit: usize,
    ) -> Result<Vec<(MemoryEntry, Vec<MemoryLink>)>, MemoryError> {
        let entries = self
            .provider
            .recall_scoped(query, None, limit, scope)
            .await?;
        let mut results = Vec::with_capacity(entries.len());

        for entry in entries {
            let mut links = self.provider.get_outgoing_links(&entry.key, None).await?;
            let incoming = self.provider.get_incoming_links(&entry.key, None).await?;
            links.extend(incoming);
            results.push((entry, links));
        }

        Ok(results)
    }

    /// Discover potential relationships between memories in a scope.
    ///
    /// When an [`UtilityLlm`] is available, pairs of memories are sent to
    /// the LLM for relationship judgement.  Otherwise a rule-based bigram
    /// similarity check is used (threshold: [`BIGRAM_SIMILARITY_THRESHOLD`]).
    ///
    /// Newly discovered links are persisted via `provider.create_link()`
    /// and returned to the caller.
    pub async fn discover_links(
        &self,
        scope: &MemoryExecutionScope,
    ) -> Result<Vec<MemoryLink>, MemoryError> {
        // Fetch all entries in scope (limited to avoid blowup)
        let entries = self.provider.export_scoped(None, scope).await?;

        if entries.len() < 2 {
            return Ok(vec![]);
        }

        // Cap the number of pairs to avoid O(n²) explosion
        let max_entries = entries.len().min(50);
        let entries = &entries[..max_entries];

        let new_links = if let Some(ref llm) = self.llm {
            // LLM-assisted discovery — process in small batches
            self.discover_links_llm(llm, entries).await?
        } else {
            // Rule-based: bigram similarity
            self.discover_links_bigram(entries).await?
        };

        Ok(new_links)
    }

    /// Get the structured neighborhood of a memory node.
    ///
    /// Returns the center entry, all incoming/outgoing links with their
    /// connected entries hydrated, and a by-type grouping map.
    pub async fn get_neighborhood(
        &self,
        key: &str,
        _radius: usize,
    ) -> Result<GraphNeighborhood, MemoryError> {
        let center = self
            .provider
            .get_by_key(key)
            .await?
            .ok_or_else(|| MemoryError::KeyNotFound(key.to_string()))?;

        let outgoing_links = self.provider.get_outgoing_links(key, None).await?;
        let incoming_links = self.provider.get_incoming_links(key, None).await?;

        let mut outgoing: Vec<(MemoryLink, MemoryEntry)> = Vec::new();
        for link in &outgoing_links {
            if let Some(entry) = self.provider.get_by_key(&link.target_key).await? {
                outgoing.push((link.clone(), entry));
            }
        }

        let mut incoming: Vec<(MemoryLink, MemoryEntry)> = Vec::new();
        for link in &incoming_links {
            if let Some(entry) = self.provider.get_by_key(&link.source_key).await? {
                incoming.push((link.clone(), entry));
            }
        }

        // Build by_link_type map
        let mut by_link_type: HashMap<String, Vec<String>> = HashMap::new();
        for link in outgoing_links.iter().chain(incoming_links.iter()) {
            let other_key = if link.source_key == key {
                &link.target_key
            } else {
                &link.source_key
            };
            by_link_type
                .entry(link.link_type.clone())
                .or_default()
                .push(other_key.clone());
        }

        Ok(GraphNeighborhood {
            center,
            incoming,
            outgoing,
            by_link_type,
        })
    }

    // ── Private helpers ─────────────────────────────────────────────

    /// LLM-based link discovery between memory pairs.
    async fn discover_links_llm(
        &self,
        llm: &Arc<dyn UtilityLlm>,
        entries: &[MemoryEntry],
    ) -> Result<Vec<MemoryLink>, MemoryError> {
        let mut new_links: Vec<MemoryLink> = Vec::new();

        // Process pairs — limit to avoid excessive LLM calls
        let max_pairs = 20usize;
        let mut pair_count = 0usize;

        for i in 0..entries.len() {
            if pair_count >= max_pairs {
                break;
            }
            for j in (i + 1)..entries.len() {
                if pair_count >= max_pairs {
                    break;
                }

                let a = &entries[i];
                let b = &entries[j];

                let prompt = format!(
                    "分析以下两条记忆，判断它们之间是否存在关系。\n\
                     如果存在，返回一行：link_type（只能是 related_to / supersedes / contradicts / evidence_for 之一）。\n\
                     如果不存在关系，返回 none。\n\n\
                     记忆 A [{}]: {}\n\
                     记忆 B [{}]: {}\n\n\
                     只返回 link_type 或 none，不要其他文字。",
                    a.key, a.content, b.key, b.content,
                );

                match llm
                    .complete("你是记忆关系分析助手。", &prompt, 20, 0.1)
                    .await
                {
                    Ok(response) => {
                        let link_type = response.trim().to_lowercase();
                        if matches!(
                            link_type.as_str(),
                            "related_to" | "supersedes" | "contradicts" | "evidence_for"
                        ) {
                            if let Err(e) =
                                self.provider.create_link(&a.key, &b.key, &link_type).await
                            {
                                warn!(
                                    source = %a.key, target = %b.key,
                                    link_type = %link_type,
                                    "failed to create discovered link: {e}"
                                );
                            } else {
                                debug!(
                                    source = %a.key, target = %b.key,
                                    link_type = %link_type,
                                    "discovered new link via LLM"
                                );
                                new_links.push(MemoryLink {
                                    source_key: a.key.clone(),
                                    target_key: b.key.clone(),
                                    link_type,
                                    created_at: chrono::Utc::now(),
                                });
                            }
                        }
                    }
                    Err(e) => {
                        warn!(
                            "LLM link discovery failed for pair ({}, {}): {e}",
                            a.key, b.key
                        );
                    }
                }

                pair_count += 1;
            }
        }

        Ok(new_links)
    }

    /// Rule-based link discovery using bigram similarity.
    async fn discover_links_bigram(
        &self,
        entries: &[MemoryEntry],
    ) -> Result<Vec<MemoryLink>, MemoryError> {
        let mut new_links: Vec<MemoryLink> = Vec::new();

        for i in 0..entries.len() {
            for j in (i + 1)..entries.len() {
                let a = &entries[i];
                let b = &entries[j];
                let sim = bigram_similarity(&a.content, &b.content);
                if sim > BIGRAM_SIMILARITY_THRESHOLD {
                    if let Err(e) = self
                        .provider
                        .create_link(&a.key, &b.key, "related_to")
                        .await
                    {
                        warn!(
                            source = %a.key, target = %b.key,
                            similarity = %sim,
                            "failed to create bigram-discovered link: {e}"
                        );
                    } else {
                        debug!(
                            source = %a.key, target = %b.key,
                            similarity = %sim,
                            "discovered link via bigram similarity"
                        );
                        new_links.push(MemoryLink {
                            source_key: a.key.clone(),
                            target_key: b.key.clone(),
                            link_type: "related_to".to_string(),
                            created_at: chrono::Utc::now(),
                        });
                    }
                }
            }
        }

        Ok(new_links)
    }
}

/// Compute the Dice coefficient of two strings' character bigrams.
///
/// Returns a value in `[0.0, 1.0]` where 1.0 means identical bigram sets.
fn bigram_similarity(a: &str, b: &str) -> f64 {
    let bigrams = |s: &str| -> HashSet<(char, char)> {
        let chars: Vec<char> = s.chars().collect();
        chars.windows(2).map(|w| (w[0], w[1])).collect()
    };

    let a_bigrams = bigrams(a);
    let b_bigrams = bigrams(b);

    if a_bigrams.is_empty() && b_bigrams.is_empty() {
        return 1.0;
    }
    if a_bigrams.is_empty() || b_bigrams.is_empty() {
        return 0.0;
    }

    let intersection = a_bigrams.intersection(&b_bigrams).count();
    (2.0 * intersection as f64) / (a_bigrams.len() + b_bigrams.len()) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bigram_similarity_identical() {
        assert!((bigram_similarity("hello world", "hello world") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bigram_similarity_empty() {
        assert!((bigram_similarity("", "") - 1.0).abs() < f64::EPSILON);
        assert!((bigram_similarity("hello", "")).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bigram_similarity_partial() {
        let sim = bigram_similarity("hello world", "hello there");
        assert!(sim > 0.0 && sim < 1.0);
    }

    #[test]
    fn test_bigram_similarity_disjoint() {
        let sim = bigram_similarity("abc", "xyz");
        assert!(sim.abs() < f64::EPSILON);
    }
}
