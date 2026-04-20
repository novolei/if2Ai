//! If2Ai canonical runtime contracts (Phase M0.3 + M0.4 + M0.5 skeleton).
//!
//! This module is the single canonical home for runtime-facing contract
//! types shared between the Rust backend and the TypeScript frontend
//! (mirrored in [`src/transport/contracts.ts`](../../../../../src/transport/contracts.ts)).
//!
//! Scope of this skeleton:
//! - Establish stable canonical type names with `serde` JSON shapes.
//! - Provide a single canonical event envelope (`RuntimeEventEnvelope`)
//!   so future emitters and the frontend translator can be wired
//!   incrementally without re-shaping payloads.
//! - Stay independent of any concrete `commands::*` implementation —
//!   no runtime logic lives here.
//!
//! Out of scope (deferred to later phases):
//! - Real `ConversationRuntime` event production wiring (M1).
//! - Real activation state-machine implementation (M1
//!   `activation_service`).
//! - Real request classifier producing `ExecutionModeDecision` (M1
//!   `request_intelligence_service`).
//! - Frontend translator/reducer/store wiring (M2).
//!
//! The companion docs are
//! [`docs/staff-remediation/if2ai-canonical-domain-model.md`](../../../../../docs/staff-remediation/if2ai-canonical-domain-model.md)
//! and
//! [`docs/staff-remediation/if2ai-workflow-truth.md`](../../../../../docs/staff-remediation/if2ai-workflow-truth.md).

#![allow(dead_code)]
// M0.3 skeleton: re-exports exist so M1+ wiring does not need to
// touch this module. They are unused by design until then.
#![allow(unused_imports)]

pub mod activation;
pub mod common;
pub mod execution_mode;
pub mod memory;

pub use activation::{
    ActivationAction, ActivationActionKind, ActivationFailureReason, ActivationLicense,
    ActivationSnapshot, ActivationStatus, ActivationStatusKind,
};
pub use common::{
    CorrelationIds, RuntimeEventEnvelope, RuntimeEventPayloadFamily, RuntimeEventType,
    SchemaVersion, CONTRACTS_SCHEMA_VERSION,
};
pub use execution_mode::{
    ClassifierEvidence, ComplexityLevel, ExecutionMode, ExecutionModeDecision, ReasonCode,
    RiskLevel, RouteHint, ScenarioProfileHint,
};
pub use memory::{
    MemoryDecision, MemoryDecisionVerdict, MemoryItemProjection, MemoryKind, MemoryProjection,
    MemoryScope,
};
