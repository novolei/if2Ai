//! Memory injection service (Phase M1.4).
//!
//! Owns the responsibility of producing per-turn memory injection
//! artefacts that the prompt planner appends as `PromptBlock`s and
//! that the IPC adapter forwards to the frontend on
//! `stream_complete`.
//!
//! Replaces three previously command-resident pieces of logic:
//!
//! 1. `commands::agent::append_memory_injection_sections` (already
//!    moved into `prompt_planner` in M1.3 — moved out again here so
//!    `prompt_planner` no longer reaches `crate::modules::memory::*`
//!    directly).
//! 2. `commands::agent::retrieve_memory_context` (the M1.3 first cut
//!    still left this in `commands/agent.rs`; M1.4 moves it).
//! 3. `commands::agent::map_scored_memory_to_payload` (helper for the
//!    frontend payload shape).
//!
//! Hard rules (mirrored in
//! [`docs/staff-remediation/m0-god-file-responsibility-inventory.md`](../../../../../docs/staff-remediation/m0-god-file-responsibility-inventory.md)
//! §4.5 step 3):
//!
//! 1. This service MUST NOT import from `crate::commands::*`.
//! 2. M1.4 is a **boundary** slice — recall ordering, write policy
//!    and quality gating are unchanged. Those land in `M3`.
//! 3. The service does not own the frontend payload event; it only
//!    produces typed [`MemoryItemProjection`]s for the IPC adapter
//!    to embed in `StreamTokenPayload.memory_context`.
//! 4. Static memory injection (pinned / compiled / rules) and
//!    per-turn retrieval are kept as separately callable helpers
//!    so future slices can short-circuit one without touching the
//!    other.

use std::path::PathBuf;
use std::sync::Arc;

use crate::modules::memory::inject::CacheHint;
use crate::modules::memory::retrieval::{ActiveRetrievalManager, ScoredMemory};
use crate::modules::memory::{PinnedStore, SharedMemoryProvider};

// Re-export the canonical type from the runtime contracts so callers
// (`commands::agent`, `runtime::stream_emitter`) can import it from
// either module — the wire shape is owned by the runtime contract,
// not by this service. This enforces the layering rule that
// `runtime` must not depend on `application`.
pub use crate::modules::runtime::contracts::MemoryItemProjection;

/// Long-lived memory dependencies the service needs.
///
/// Constructed once per turn from `AppState` handles. Held by value
/// (with cheap `Arc` clones) so the service does not borrow the IPC
/// adapter's state lock for any non-trivial duration.
pub struct MemoryInjectionDeps {
    pub pinned_store: Arc<dyn PinnedStore>,
    pub memory_provider: SharedMemoryProvider,
    pub active_retrieval_manager: Option<Arc<ActiveRetrievalManager>>,
}

/// Per-turn input to [`prepare_memory_injection`].
pub struct MemoryInjectionRequest {
    /// Canonical session id (see canonical domain model §3.2).
    pub session_id: Option<String>,
    /// Canonical project id (see canonical domain model §3.3).
    pub project_id: Option<String>,
    /// Workdir path string for memory scope resolution.
    pub workdir: Option<String>,
    /// User message for this turn — used as the retrieval query.
    pub user_message: String,
    /// Caller tag for tracing (`"run_agent_turn"` /
    /// `"start_agent_stream"`).
    pub caller: &'static str,
}

/// Discriminator for the four canonical memory injection sections.
/// Order matches the legacy assembly order in `commands/agent.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryInjectionSectionKind {
    Pinned,
    Compiled,
    Procedural,
    Rules,
    Retrieved,
}

/// One memory section, ready for the prompt planner to wrap as a
/// `PromptBlock`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MemoryInjectionSection {
    pub kind: MemoryInjectionSectionKind,
    pub content: String,
    /// Cache hint propagated from [`crate::modules::memory::inject::MemoryCacheHints`].
    /// Defaults to [`CacheHint::None`] for sections without explicit hints
    /// (e.g. retrieved memory).
    #[serde(default)]
    pub cache_hint: CacheHint,
}

// `MemoryItemProjection` lives in
// `crate::modules::runtime::contracts::memory` (re-exported above).
// Phase M1.6 lifted it into the runtime contract so the
// stream emitter no longer reaches back into the application layer.

/// Composite output of [`prepare_memory_injection`].
///
/// `prompt_sections` holds both the static (pinned / compiled /
/// rules) and per-turn (retrieved) sections in canonical order so
/// the prompt planner can append them without re-deciding ordering.
/// `memory_items` is the structured frontend payload — the IPC
/// adapter embeds it into `StreamTokenPayload.memory_context` on
/// `stream_complete`.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct MemoryInjectionArtifacts {
    pub prompt_sections: Vec<MemoryInjectionSection>,
    pub memory_items: Vec<MemoryItemProjection>,
}

/// Compose static memory injection + per-turn retrieval into a
/// single typed result.
///
/// Failure handling preserves the legacy semantics:
/// - Pinned-store / compiled-memory failures are logged at WARN and
///   the corresponding sections are simply absent from the result.
/// - Retrieval failures yield an empty `memory_items` and no
///   retrieved section — never propagate to the caller.
pub async fn prepare_memory_injection(
    deps: &MemoryInjectionDeps,
    req: MemoryInjectionRequest,
) -> MemoryInjectionArtifacts {
    let mut prompt_sections: Vec<MemoryInjectionSection> = Vec::new();

    append_static_sections(
        &mut prompt_sections,
        deps.pinned_store.clone(),
        deps.memory_provider.clone(),
        req.session_id.as_deref(),
        req.project_id.as_deref(),
        req.workdir.as_deref(),
        req.caller,
    )
    .await;

    let retrieved = retrieve_memory_for_turn(deps, &req.user_message).await;
    if !retrieved.prompt_fragment.is_empty() {
        prompt_sections.push(MemoryInjectionSection {
            kind: MemoryInjectionSectionKind::Retrieved,
            content: retrieved.prompt_fragment,
            cache_hint: CacheHint::None, // retrieved memory is turn-specific
        });
    }

    MemoryInjectionArtifacts {
        prompt_sections,
        memory_items: retrieved.items,
    }
}

/// Per-turn retrieval result used internally by
/// [`prepare_memory_injection`] and exposed for callers that already
/// have a static section path.
#[derive(Debug, Clone, Default)]
pub struct RetrievedMemory {
    pub prompt_fragment: String,
    pub items: Vec<MemoryItemProjection>,
}

/// Run the active retrieval manager and format the result.
///
/// Behaviour preserved from the legacy
/// `commands::agent::retrieve_memory_context`:
/// - Uses the shared `ActiveRetrievalManager` when present, falls
///   back to a one-shot manager with defaults otherwise.
/// - Empty / failed retrieval yields an empty struct (never
///   propagates the error).
/// - The `# Relevant Memories` markdown header and per-item bullet
///   format are byte-identical to the legacy output so the model
///   sees the same prompt.
pub async fn retrieve_memory_for_turn(
    deps: &MemoryInjectionDeps,
    user_message: &str,
) -> RetrievedMemory {
    let result = if let Some(mgr) = &deps.active_retrieval_manager {
        mgr.retrieve(user_message, &*deps.memory_provider).await
    } else {
        ActiveRetrievalManager::with_defaults()
            .retrieve(user_message, &*deps.memory_provider)
            .await
    };

    let scored = match result {
        Ok(scored) => scored,
        Err(e) => {
            tracing::warn!(
                "[memory_injection_service] Retrieval failed, proceeding without memory context: {e}"
            );
            return RetrievedMemory::default();
        }
    };

    if scored.is_empty() {
        return RetrievedMemory::default();
    }

    let mut prompt_fragment = String::from("# Relevant Memories\n\n");
    for sm in &scored {
        prompt_fragment.push_str(&format!(
            "- [{}] (score: {:.3}): {}\n",
            sm.entry.category.as_str(),
            sm.score,
            sm.entry.content
        ));
    }

    let items: Vec<MemoryItemProjection> =
        scored.iter().map(map_scored_memory_to_projection).collect();

    RetrievedMemory {
        prompt_fragment,
        items,
    }
}

/// Map a backend `MemoryEntry` (+ scored fusion result) to the
/// IPC-bound projection. Scope is derived from the entry's optional
/// `session_id` / `project_id` bindings; entries without either are
/// treated as `"global"`.
fn map_scored_memory_to_projection(sm: &ScoredMemory) -> MemoryItemProjection {
    let scope = if sm.entry.session_id.is_some() {
        "session"
    } else if sm.entry.project_id.is_some() {
        "project"
    } else {
        "global"
    };
    MemoryItemProjection {
        id: sm.entry.key.clone(),
        content: sm.entry.content.clone(),
        scope: scope.to_string(),
        relevance_score: Some(sm.score),
        stored_at: Some(sm.entry.created_at.to_rfc3339()),
    }
}

/// Pre-fetch [`crate::modules::memory::MemoryInjection`] and append
/// it as up to four sections (pinned / compiled / procedural / rules).
/// Equivalent to the M1.3 `prompt_planner::append_memory_injection_blocks`
/// helper, now relocated to its proper service home.
async fn append_static_sections(
    sections: &mut Vec<MemoryInjectionSection>,
    pinned_store: Arc<dyn PinnedStore>,
    memory_provider: SharedMemoryProvider,
    session_id: Option<&str>,
    project_id: Option<&str>,
    workdir: Option<&str>,
    caller: &'static str,
) {
    let memory_cfg = crate::modules::runtime::config::current().memory();
    if !memory_cfg.inject_to_prompt() {
        return;
    }
    let max_tokens = memory_cfg.max_inject_tokens() as usize;

    let scope = crate::modules::memory::scope::MemoryScopeResolver::resolve(
        session_id, project_id, workdir,
    );

    // MEM-MOD-PATH-FIX — single root via if2ai_data_root().
    let memory_root = crate::modules::config::store::if2ai_data_root().join("memory");
    let compiled_path = memory_root.join("memory.md");

    let is_zh = crate::modules::runtime::locale::is_zh();

    match crate::modules::memory::build_memory_injection(
        pinned_store,
        &scope,
        &compiled_path,
        is_zh,
        max_tokens,
        Some(memory_provider),
    )
    .await
    {
        Ok(injection) => {
            if let Some(section) = injection.pinned_section {
                sections.push(MemoryInjectionSection {
                    kind: MemoryInjectionSectionKind::Pinned,
                    content: section,
                    cache_hint: injection.cache_hints.pinned,
                });
            }
            if let Some(section) = injection.compiled_section {
                sections.push(MemoryInjectionSection {
                    kind: MemoryInjectionSectionKind::Compiled,
                    content: section,
                    cache_hint: injection.cache_hints.compiled,
                });
            }
            if let Some(section) = injection.procedural_section {
                sections.push(MemoryInjectionSection {
                    kind: MemoryInjectionSectionKind::Procedural,
                    content: section,
                    cache_hint: injection.cache_hints.procedural,
                });
            }
            sections.push(MemoryInjectionSection {
                kind: MemoryInjectionSectionKind::Rules,
                content: injection.rules_section,
                cache_hint: injection.cache_hints.rules,
            });
            tracing::debug!(
                caller = caller,
                tokens_estimate = injection.total_tokens_estimate,
                "[memory_injection_service] static injection appended"
            );
        }
        Err(e) => {
            tracing::warn!(
                caller = caller,
                error = %e,
                "[memory_injection_service] static injection failed; continuing without"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifacts_default_is_empty() {
        let a = MemoryInjectionArtifacts::default();
        assert!(a.prompt_sections.is_empty());
        assert!(a.memory_items.is_empty());
    }

    #[test]
    fn projection_serialises_with_legacy_shape() {
        let p = MemoryItemProjection {
            id: "m-1".into(),
            content: "hello".into(),
            scope: "session".into(),
            relevance_score: Some(0.42),
            stored_at: Some("2026-04-20T00:00:00+00:00".into()),
        };
        let s = serde_json::to_value(&p).unwrap();
        // Field names match the legacy MemoryContextItemPayload wire
        // shape so the frontend MemoryChip stays compatible.
        assert_eq!(s["id"], "m-1");
        assert_eq!(s["scope"], "session");
        assert!(s["relevance_score"].is_number());
    }
}
