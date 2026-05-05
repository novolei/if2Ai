//! Memory recall assembler (Phase M3.5 skeleton).
//!
//! Composes per-turn recall + injection into the canonical
//! 6-section order required by the file-level plan §5.6 P5:
//!
//!   1. rules            (always; from memory injection rules section)
//!   2. pinned           (from PinnedStore via memory injection)
//!   3. critical facts   (M3.4 quality-gate-promoted; placeholder
//!                        section today — empty until quality
//!                        gate persistence wiring lands in M3-B+)
//!   4. preferences      (placeholder; empty today)
//!   5. compiled         (from compiled-memory file via memory injection)
//!   6. episodes         (from per-turn retrieval `# Relevant
//!                        Memories`)
//!
//! Honest scope:
//!
//! - The assembler today **wraps** [`super::memory_injection_service::prepare_memory_injection`]
//!   and re-orders its sections into the canonical 6-section
//!   shape.  It does **not** yet pull stable facts / preferences
//!   from a typed object store — those sections are emitted as
//!   empty placeholders so the canonical order is observable
//!   from day one.  M3.4+ persistence will populate them.
//! - The assembler returns a `RecallAssemblyResult` that carries
//!   typed `RecalledSection`s, the underlying
//!   [`MemoryInjectionArtifacts`] (so the prompt planner keeps
//!   working), and a `RecallDiagnostics` skeleton for the future
//!   "usefulness" metric mentioned in m3.5 acceptance criteria.

#![allow(dead_code)]

use super::memory_injection_service::{
    prepare_memory_injection, MemoryInjectionArtifacts, MemoryInjectionDeps,
    MemoryInjectionRequest, MemoryInjectionSectionKind, MemoryItemProjection,
};

/// Stable assembler version for harness eval pinning.
pub const MEMORY_RECALL_ASSEMBLER_VERSION: &str = "memory-recall-assembler@m3.5-skeleton";

/// Canonical 6-section recall slot ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecallSectionSlot {
    Rules,
    Pinned,
    CriticalFacts,
    Preferences,
    Compiled,
    Episodes,
}

impl RecallSectionSlot {
    /// Canonical iteration order (1..=6).
    pub fn canonical_order() -> [Self; 6] {
        [
            Self::Rules,
            Self::Pinned,
            Self::CriticalFacts,
            Self::Preferences,
            Self::Compiled,
            Self::Episodes,
        ]
    }

    /// Stable string label used by harness traces and the future
    /// M3.6 frontend projection.
    pub fn label(self) -> &'static str {
        match self {
            Self::Rules => "rules",
            Self::Pinned => "pinned",
            Self::CriticalFacts => "critical_facts",
            Self::Preferences => "preferences",
            Self::Compiled => "compiled",
            Self::Episodes => "episodes",
        }
    }
}

/// One recall section, ready for the prompt planner to consume.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RecalledSection {
    pub slot: RecallSectionSlot,
    pub content: String,
    /// `true` when the section is non-empty (had content to emit).
    /// Useful for diagnostics — empty placeholder slots can be
    /// counted without inspecting the string.
    pub present: bool,
}

/// First-cut diagnostics surface for the assembler.  Today only
/// counts sections present + memory-item count; M4 harness will
/// extend with usefulness scoring (per file-level plan §5.6 P5
/// "usefulness diagnostics skeleton").
#[derive(Debug, Clone, serde::Serialize)]
pub struct RecallDiagnostics {
    pub total_sections: usize,
    pub non_empty_sections: usize,
    pub memory_item_count: usize,
    pub assembler_version: String,
}

/// Composite assembler result.
pub struct RecallAssemblyResult {
    /// Canonical 6-slot ordered sections.
    pub sections: Vec<RecalledSection>,
    /// Underlying injection artefacts (the prompt planner still
    /// consumes this directly today; the canonical sections above
    /// are an additional explainability surface).
    pub artifacts: MemoryInjectionArtifacts,
    /// Per-turn memory items (mirrors `artifacts.memory_items`;
    /// re-exposed for `PreparedChatInputs.memory_items` callers).
    pub memory_items: Vec<MemoryItemProjection>,
    pub diagnostics: RecallDiagnostics,
}

/// Assemble per-turn recall + injection into the canonical
/// 6-section order.
///
/// Wraps [`prepare_memory_injection`] and re-arranges its output.
/// The legacy 4-section order produced by the underlying service
/// (Pinned → Compiled → Rules → Retrieved) is preserved inside
/// `artifacts.prompt_sections` for byte-compatibility with the
/// prompt planner; the canonical 6-slot view here is a parallel
/// projection.
pub async fn assemble_recall(
    deps: &MemoryInjectionDeps,
    request: MemoryInjectionRequest,
) -> RecallAssemblyResult {
    // MIG-005: Save request fields before moving request into prepare_memory_injection
    let session_id = request.session_id.clone();
    let project_id = request.project_id.clone();
    let workdir = request.workdir.clone();

    let artifacts = prepare_memory_injection(deps, request).await;
    let memory_items = artifacts.memory_items.clone();

    // Index the 5 legacy section kinds for O(1) lookup.
    let mut pinned: Option<String> = None;
    let mut compiled: Option<String> = None;
    // Procedural rules are injected into the system prompt via PromptBlock::MemoryInjectionProcedural.
    // They are intentionally excluded from RecallArtifacts to avoid double-injection.
    let mut _procedural: Option<String> = None;
    let mut rules: Option<String> = None;
    let mut retrieved: Option<String> = None;
    for s in &artifacts.prompt_sections {
        match s.kind {
            MemoryInjectionSectionKind::Pinned => pinned = Some(s.content.clone()),
            MemoryInjectionSectionKind::Compiled => compiled = Some(s.content.clone()),
            MemoryInjectionSectionKind::Procedural => _procedural = Some(s.content.clone()),
            MemoryInjectionSectionKind::Rules => rules = Some(s.content.clone()),
            MemoryInjectionSectionKind::Retrieved => retrieved = Some(s.content.clone()),
        }
    }

    // Canonical 6-slot ordering. Critical facts and preferences are
    // now recalled from the typed object store (MIG-005).

    // MIG-005: Recall critical facts from memory provider
    let critical_facts = {
        let scope = crate::modules::memory::scope::MemoryExecutionScope {
            session_id: session_id.clone(),
            project_id: project_id.clone(),
            workdir: workdir.clone(),
        };
        match deps
            .memory_provider
            .recall_scoped("", Some("core"), 10, &scope)
            .await
        {
            Ok(entries) => {
                let facts: Vec<String> = entries
                    .into_iter()
                    .filter(|e| e.key.starts_with("Fact:"))
                    .map(|e| e.content)
                    .collect();
                if facts.is_empty() {
                    None
                } else {
                    Some(format!("# Critical Facts\n\n{}", facts.join("\n")))
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to recall critical facts");
                None
            }
        }
    };

    // MIG-005: Recall preferences from memory provider
    let preferences = {
        let scope = crate::modules::memory::scope::MemoryExecutionScope {
            session_id,
            project_id,
            workdir,
        };
        match deps
            .memory_provider
            .recall_scoped("", Some("core"), 10, &scope)
            .await
        {
            Ok(entries) => {
                let prefs: Vec<String> = entries
                    .into_iter()
                    .filter(|e| e.key.starts_with("Preference:"))
                    .map(|e| e.content)
                    .collect();
                if prefs.is_empty() {
                    None
                } else {
                    Some(format!("# Preferences\n\n{}", prefs.join("\n")))
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to recall preferences");
                None
            }
        }
    };

    let sections: Vec<RecalledSection> = RecallSectionSlot::canonical_order()
        .into_iter()
        .map(|slot| {
            let content = match slot {
                RecallSectionSlot::Rules => rules.clone(),
                RecallSectionSlot::Pinned => pinned.clone(),
                RecallSectionSlot::CriticalFacts => critical_facts.clone(),
                RecallSectionSlot::Preferences => preferences.clone(),
                RecallSectionSlot::Compiled => compiled.clone(),
                RecallSectionSlot::Episodes => retrieved.clone(),
            };
            let present = content.is_some();
            RecalledSection {
                slot,
                content: content.unwrap_or_default(),
                present,
            }
        })
        .collect();

    let total = sections.len();
    let non_empty = sections.iter().filter(|s| s.present).count();
    let diagnostics = RecallDiagnostics {
        total_sections: total,
        non_empty_sections: non_empty,
        memory_item_count: memory_items.len(),
        assembler_version: MEMORY_RECALL_ASSEMBLER_VERSION.to_string(),
    };

    RecallAssemblyResult {
        sections,
        artifacts,
        memory_items,
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_order_is_six_slots_in_documented_sequence() {
        let order = RecallSectionSlot::canonical_order();
        assert_eq!(order.len(), 6);
        assert_eq!(order[0], RecallSectionSlot::Rules);
        assert_eq!(order[1], RecallSectionSlot::Pinned);
        assert_eq!(order[2], RecallSectionSlot::CriticalFacts);
        assert_eq!(order[3], RecallSectionSlot::Preferences);
        assert_eq!(order[4], RecallSectionSlot::Compiled);
        assert_eq!(order[5], RecallSectionSlot::Episodes);
    }

    #[test]
    fn slot_labels_are_stable() {
        assert_eq!(RecallSectionSlot::Rules.label(), "rules");
        assert_eq!(RecallSectionSlot::Pinned.label(), "pinned");
        assert_eq!(RecallSectionSlot::CriticalFacts.label(), "critical_facts");
        assert_eq!(RecallSectionSlot::Preferences.label(), "preferences");
        assert_eq!(RecallSectionSlot::Compiled.label(), "compiled");
        assert_eq!(RecallSectionSlot::Episodes.label(), "episodes");
    }
}
