//! If2Ai application service layer (Phase M1 skeleton).
//!
//! The application layer sits between the IPC `commands::*` adapters
//! and the lower-level `runtime` / `memory` / `tools` / `api` modules.
//! Its job is to own the **business orchestration** of a single agent
//! turn so the command layer can shrink back into a thin IPC adapter
//! and so M2 frontend projection has a stable service edge to consume.
//!
//! This module is intentionally introduced empty-of-coordination in
//! Phase M1 first round (`m1.1 + m1.2 + m1.3`):
//!
//! - `provider_service` owns provider/model resolving and runtime
//!   client construction (M1.2).
//! - `prompt_planner` owns prompt assembly (system + web-tools guide +
//!   memory injection sections + retrieved-memory fragment) and
//!   produces a structured [`prompt_planner::PromptPlan`] so M4 harness
//!   can trace prompt composition (M1.3).
//! - `turn_service` exposes the first orchestration seam that
//!   composes the two above into a single
//!   [`turn_service::PreparedChatInputs`] consumed by `commands::agent`
//!   (M1.1).
//!
//! Strict layering rules (enforced by review, not by compiler):
//! 1. `application::*` MUST NOT import from `crate::commands::*`.
//! 2. `application::*` MAY import from `crate::modules::api`,
//!    `crate::modules::config`, `crate::modules::memory`,
//!    `crate::modules::runtime`, `crate::modules::tools`.
//! 3. Future services (memory_injection / stream_emitter /
//!    request_intelligence / activation / license_lifecycle) land
//!    here in subsequent M1 slices; this `mod.rs` is the only place
//!    that wires them up.
//!
//! See:
//! - [`docs/staff-remediation/m0-god-file-responsibility-inventory.md`](../../../../../docs/staff-remediation/m0-god-file-responsibility-inventory.md)
//! - [`docs/exec-plans/active/phase-m1-executor-runbook.md`](../../../../../docs/exec-plans/active/phase-m1-executor-runbook.md)
//! - [`docs/exec-plans/active/phase-m1-initial-slices-file-level-plan.md`](../../../../../docs/exec-plans/active/phase-m1-initial-slices-file-level-plan.md)

#![allow(dead_code)]
// Re-exports below form the M1 service surface. Some are not yet
// consumed by any other module — that is intentional; later M1
// slices (m1.4 memory_injection, m1.5 stream_emitter, m1.6
// request_intelligence, m1.7 activation) will pick them up.
#![allow(unused_imports)]

pub mod activation_service;
pub mod license_lifecycle_service;
pub mod memory_candidate_extractor;
pub mod memory_conflict_resolution;
pub mod memory_coordinator;
pub mod memory_injection_service;
pub mod memory_quality_gate;
pub mod memory_recall_assembler;
pub mod memory_write_policy;
pub mod prompt_planner;
pub mod provider_service;
pub mod real_api_client;
pub mod request_intelligence_service;
pub mod turn_service;

pub use activation_service::{ActivationCeremonyResult, ActivationChecklist, ActivationService};
pub use license_lifecycle_service::{snapshot_with_kind, LicenseLifecycleService};
pub use memory_candidate_extractor::{
    extract_memory_store_tool_candidates, lookup_existing_records_for_candidates,
    SOURCE_MEMORY_STORE_TOOL,
};
pub use memory_conflict_resolution::{
    reason_codes as memory_conflict_reason_codes, resolve_conflict, ConflictResolution,
    ConflictResolutionOutcome, ExistingRecordRef, MEMORY_CONFLICT_RESOLVER_VERSION,
};
pub use memory_coordinator::{
    AfterTurnInput, AfterTurnOutput, MemoryCoordinator, PrepareContextInput, PrepareContextOutput,
};
pub use memory_injection_service::{
    prepare_memory_injection, retrieve_memory_for_turn, MemoryInjectionArtifacts,
    MemoryInjectionDeps, MemoryInjectionRequest, MemoryInjectionSection,
    MemoryInjectionSectionKind, MemoryItemProjection, RetrievedMemory,
};
pub use memory_quality_gate::{
    evaluate_quality_gate, reason_codes as memory_quality_reason_codes, QualityGateAccepted,
    QualityGateContext, QualityGateRejected, QualityGateResult, QualityGateWarning,
    MEMORY_QUALITY_GATE_VERSION,
};
pub use memory_recall_assembler::{
    assemble_recall, RecallAssemblyResult, RecallDiagnostics, RecallSectionSlot, RecalledSection,
    MEMORY_RECALL_ASSEMBLER_VERSION,
};
pub use memory_write_policy::{
    reason_codes as memory_write_reason_codes, DefaultMemoryWritePolicy, MemoryWritePolicy,
    MEMORY_WRITE_POLICY_VERSION,
};
pub use prompt_planner::{
    BuildPromptPlanRequest, PromptBlock, PromptBlockKind, PromptPlan, PromptPlanResult,
    PromptPlannerError,
};
pub use provider_service::{
    load_provider_transport_policy, resolve_chat_runtime_provider, RuntimeProviderResolution,
};
pub(crate) use real_api_client::RealApiClient;
pub use request_intelligence_service::{
    classify as classify_request_intelligence, RequestIntelligenceInput, RequestIntelligenceOutput,
};
pub use turn_service::{
    PrepareChatInputsRequest, PreparedChatInputs, TurnService, TurnServiceDeps, TurnServiceError,
};
