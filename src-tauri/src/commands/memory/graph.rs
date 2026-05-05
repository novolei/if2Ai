//! Memory Graph Tauri commands — expose graph traversal and link discovery
//! operations to the frontend via IPC.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;

use crate::commands::AppState;
use crate::modules::memory::graph::MemoryGraph;
use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::memory::{GraphNeighborhood, GraphNode, MemoryEntry, MemoryLink};

// ── DTO types ────────────────────────────────────────────────────────

/// Full graph DTO containing all memory nodes and their links.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullGraphDto {
    /// All memory nodes.
    pub nodes: Vec<GraphEntryDto>,
    /// All links between memory nodes.
    pub links: Vec<MemoryLinkDto>,
}

/// Serializable DTO for [`MemoryLink`] sent over IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryLinkDto {
    /// Source memory key.
    pub source_key: String,
    /// Target memory key.
    pub target_key: String,
    /// Link type string.
    pub link_type: String,
    /// ISO-8601 timestamp.
    pub created_at: String,
}

impl From<MemoryLink> for MemoryLinkDto {
    fn from(link: MemoryLink) -> Self {
        Self {
            source_key: link.source_key,
            target_key: link.target_key,
            link_type: link.link_type,
            created_at: link.created_at.to_rfc3339(),
        }
    }
}

/// Serializable DTO for [`GraphNode`] sent over IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNodeDto {
    /// Memory key.
    pub key: String,
    /// BFS depth from seed.
    pub depth: usize,
    /// Full entry content (if available).
    pub entry: Option<GraphEntryDto>,
    /// Links connected to this node.
    pub links: Vec<MemoryLinkDto>,
}

/// Minimal entry DTO for graph nodes (avoids duplicating full MemoryEntryDto).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEntryDto {
    /// Memory key.
    pub key: String,
    /// Memory content text.
    pub content: String,
    /// Category string.
    pub category: String,
    /// Importance score.
    pub importance: f64,
    /// Trust score.
    pub trust_score: f64,
    /// Created at ISO-8601.
    pub created_at: String,
    /// Cognitive layer integer.
    pub cognitive_layer: i32,
}

impl From<MemoryEntry> for GraphEntryDto {
    fn from(e: MemoryEntry) -> Self {
        Self {
            key: e.key,
            content: e.content,
            category: e.category.as_str().to_string(),
            importance: e.importance,
            trust_score: e.trust_score,
            created_at: e.created_at.to_rfc3339(),
            cognitive_layer: e.cognitive_layer.as_i32(),
        }
    }
}

impl From<GraphNode> for GraphNodeDto {
    fn from(node: GraphNode) -> Self {
        Self {
            key: node.key,
            depth: node.depth,
            entry: node.entry.map(GraphEntryDto::from),
            links: node.links.into_iter().map(MemoryLinkDto::from).collect(),
        }
    }
}

/// Serializable DTO for [`GraphNeighborhood`] sent over IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNeighborhoodDto {
    /// The center node entry.
    pub center: GraphEntryDto,
    /// Incoming links with source entries.
    pub incoming: Vec<(MemoryLinkDto, GraphEntryDto)>,
    /// Outgoing links with target entries.
    pub outgoing: Vec<(MemoryLinkDto, GraphEntryDto)>,
    /// Links grouped by type → related keys.
    pub by_link_type: HashMap<String, Vec<String>>,
}

impl From<GraphNeighborhood> for GraphNeighborhoodDto {
    fn from(n: GraphNeighborhood) -> Self {
        Self {
            center: GraphEntryDto::from(n.center),
            incoming: n
                .incoming
                .into_iter()
                .map(|(l, e)| (MemoryLinkDto::from(l), GraphEntryDto::from(e)))
                .collect(),
            outgoing: n
                .outgoing
                .into_iter()
                .map(|(l, e)| (MemoryLinkDto::from(l), GraphEntryDto::from(e)))
                .collect(),
            by_link_type: n.by_link_type,
        }
    }
}

// ── Tauri Commands ───────────────────────────────────────────────────

/// Traverse the memory graph starting from a seed key using BFS.
#[tauri::command]
pub async fn memory_graph_traverse(
    seed_key: String,
    depth: Option<usize>,
    link_types: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<Vec<GraphNodeDto>, String> {
    let graph = MemoryGraph::new(Arc::clone(&state.memory_provider));

    let link_type_refs: Option<Vec<&str>> = link_types
        .as_ref()
        .map(|v| v.iter().map(|s| s.as_str()).collect());

    let nodes = graph
        .graph_recall(&seed_key, depth, link_type_refs.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    Ok(nodes.into_iter().map(GraphNodeDto::from).collect())
}

/// Get the structured neighborhood of a memory node.
#[tauri::command]
pub async fn memory_graph_neighborhood(
    key: String,
    radius: Option<usize>,
    state: State<'_, AppState>,
) -> Result<GraphNeighborhoodDto, String> {
    let graph = MemoryGraph::new(Arc::clone(&state.memory_provider));
    let neighborhood = graph
        .get_neighborhood(&key, radius.unwrap_or(1))
        .await
        .map_err(|e| e.to_string())?;
    Ok(GraphNeighborhoodDto::from(neighborhood))
}

/// Discover new links between memories in a scope.
#[tauri::command]
pub async fn memory_graph_discover(
    _scope_kind: Option<String>,
    session_id: Option<String>,
    project_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<MemoryLinkDto>, String> {
    let scope = MemoryExecutionScope {
        session_id,
        project_id,
        workdir: None,
    };

    let graph = MemoryGraph::new(Arc::clone(&state.memory_provider));
    let new_links = graph
        .discover_links(&scope)
        .await
        .map_err(|e| e.to_string())?;

    Ok(new_links.into_iter().map(MemoryLinkDto::from).collect())
}

/// Load the full memory graph — all nodes and all links.
///
/// Fetches every memory entry via `provider.export(None)` and every link
/// via `provider.get_all_links()`, returning them as a flat
/// `FullGraphDto` for the force-graph visualization.
#[tauri::command]
pub async fn memory_graph_full(state: State<'_, AppState>) -> Result<FullGraphDto, String> {
    let provider = Arc::clone(&state.memory_provider);

    // 1. Fetch all entries
    let entries = provider.export(None).await.map_err(|e| e.to_string())?;

    // 2. Fetch all links in one query
    let links = provider.get_all_links().await.map_err(|e| e.to_string())?;

    // 3. Convert to DTOs
    let nodes: Vec<GraphEntryDto> = entries.into_iter().map(GraphEntryDto::from).collect();
    let link_dtos: Vec<MemoryLinkDto> = links.into_iter().map(MemoryLinkDto::from).collect();

    Ok(FullGraphDto {
        nodes,
        links: link_dtos,
    })
}
