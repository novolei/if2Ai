//! Common runtime contract primitives (Phase M0.3 skeleton).
//!
//! This file defines the canonical envelope and identifying primitives
//! that every runtime event family will share. It deliberately stays
//! payload-agnostic: concrete payload families (conversation / tool /
//! permission / memory / activation / execution_mode) are introduced
//! as they get wired up in M1+, but each will fit into
//! [`RuntimeEventEnvelope`] without re-shaping.
//!
//! See the companion TS skeleton at
//! [`src/transport/contracts.ts`](../../../../../src/transport/contracts.ts)
//! and the design source at
//! [`docs/staff-remediation/runtime-contracts-and-event-projection-design.md`](../../../../../docs/staff-remediation/runtime-contracts-and-event-projection-design.md).

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Current canonical contracts schema version.
///
/// Bump on any backwards-incompatible change to the envelope or to
/// any payload family. Minor field additions are allowed without
/// bumping.
pub const CONTRACTS_SCHEMA_VERSION: &str = "1.0.0-skeleton";

/// Canonical schema version marker for a single envelope.
///
/// Mirrors [`CONTRACTS_SCHEMA_VERSION`] but lives on every envelope
/// instance so receivers can pin a minimum version without parsing
/// the payload.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SchemaVersion(pub String);

impl SchemaVersion {
    /// Build the current canonical version marker.
    #[must_use]
    pub fn current() -> Self {
        Self(CONTRACTS_SCHEMA_VERSION.to_string())
    }
}

impl Default for SchemaVersion {
    fn default() -> Self {
        Self::current()
    }
}

/// Canonical runtime event type discriminator.
///
/// Every emitter must classify its event into exactly one of these
/// families so the frontend translator can route to the correct
/// reducer. New families MUST be appended here before they are
/// emitted in production code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEventType {
    /// Conversation-shaped events: prompt accepted, response delta,
    /// response completed, run finished.
    Conversation,
    /// Tool-shaped events: tool requested, tool result, tool error.
    Tool,
    /// Permission-shaped events: prompt opened, decision recorded.
    Permission,
    /// Memory-shaped events: capture, recall, write decision,
    /// promotion / demotion / rejection.
    Memory,
    /// Activation lifecycle events: status transitions, license
    /// refresh, revoke, deactivate.
    Activation,
    /// Execution-mode classifier events: decision produced, evidence
    /// snapshot, escalation triggered.
    ExecutionMode,
    /// Harness governance events (recording, replay, eval).
    Harness,
    /// System-level events (boot phase changed, recovery actions).
    System,
    // ── FEAT-INT-001: Agent Evolution event families (10 new variants).
    // Each one is paired with a `runtime-event-payloads.ts` interface
    // on the frontend; serde `rename_all = "snake_case"` makes the
    // Rust enum name → wire string match exactly.
    /// FEAT-SH-001~003 — daemon health transition (DaemonState change,
    /// recovery outcome, MCP / browser liveness probe result).
    DaemonHealth,
    /// FEAT-SE-001 — a new SkillDraft was sedimented from a session.
    SkillSedimented,
    /// FEAT-TE-001..004 — a context-compression / digest pass ran
    /// (kept_tokens, dropped, passthrough flag).
    CompressionEvent,
    /// FEAT-SE-003 — a constitution rule fired against a draft skill
    /// or self-edit proposal.
    ConstitutionViolation,
    /// FEAT-AE-001~003 — a self-edit proposal was generated /
    /// verified / promoted.
    SelfEditProposal,
    /// FEAT-SH-003 / FEAT-BR-002 — browser session health / coordinate
    /// strategy decision.
    BrowserHealth,
    /// FEAT-DK-001/003 — a domain-knowledge entry was looked up or
    /// auto-contributed.
    DomainKnowledge,
    /// FEAT-DK-002 — a working checkpoint was extracted or injected.
    CheckpointUpdated,
    /// FEAT-AE-002 — a verification verdict (pass/fail with gates).
    VerificationDecision,
    /// FEAT-BR-001 — a SimplifiedContent payload was produced for a
    /// browser web_scan call.
    ContentSimplified,
}

/// Canonical payload family marker, used as a free-form tag inside
/// the envelope when a richer enum is not yet defined.
///
/// This exists so M0.3 consumers can already round-trip envelopes
/// without committing to a closed payload enum. M1 wiring slices
/// will replace ad-hoc strings with proper enums per family.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RuntimeEventPayloadFamily(pub String);

impl RuntimeEventPayloadFamily {
    /// Construct from a static or owned string tag.
    pub fn new(tag: impl Into<String>) -> Self {
        Self(tag.into())
    }
}

/// Cross-cutting correlation identifiers carried by every envelope.
///
/// All ids are optional in the skeleton because not every event
/// produced before M1 will have a `run_id` yet (e.g. `system` boot
/// events happen before any run exists). Consumers MUST treat absent
/// ids as "not applicable", not as "unknown".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrelationIds {
    /// Canonical `session` id (see canonical domain model §3.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Canonical `project` id (see canonical domain model §3.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// Canonical `run` id (see canonical domain model §3.4).
    ///
    /// Note: present in the contract from M0.3 even though no
    /// `Run` entity exists in code yet — M1 will introduce it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    /// Stream id, used today by `start_agent_stream` /
    /// `stop_agent_stream`. Held for backwards compatibility during
    /// the M1 cut-over; new code should prefer `run_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_id: Option<String>,
    /// Optional turn / step counter inside a run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_index: Option<u32>,
    /// Optional tool-attempt identifier inside a run.
    ///
    /// Uniquely identifies a single tool invocation attempt within a
    /// run. When a tool call is retried, each attempt gets a distinct
    /// `attempt_id` (monotonically increasing `attempt_no` is tracked
    /// separately in the attempt ledger, T-013).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt_id: Option<String>,
    // ── Agents Teams (ARCHITECTURE.md §9.3): optional until `team` bounded context ships.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delegation_id: Option<String>,
}

/// Canonical envelope wrapping every runtime-emitted event.
///
/// Frontend listeners MUST consume only this envelope shape; the
/// translator layer (M2) is responsible for unwrapping `payload`
/// into typed family payloads.
///
/// `payload` is `serde_json::Value` in the M0.3 skeleton so emitters
/// can land incrementally without forcing a closed enum migration in
/// the same slice. M1 will progressively narrow this to typed enums
/// per family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEventEnvelope {
    /// Schema version marker. Defaults to [`SchemaVersion::current`].
    #[serde(default)]
    pub schema_version: SchemaVersion,
    /// Canonical event type family (see [`RuntimeEventType`]).
    pub event_type: RuntimeEventType,
    /// Free-form payload-family tag, narrows `event_type` (e.g.
    /// `event_type = Conversation`, `payload_family = "delta"`).
    pub payload_family: RuntimeEventPayloadFamily,
    /// ISO-8601 / RFC3339 timestamp at emit time. String to keep the
    /// contract serializer-agnostic; concrete emitters should use
    /// `chrono::Utc::now().to_rfc3339()`.
    pub emitted_at: String,
    /// Cross-cutting correlation ids.
    #[serde(default)]
    pub correlation: CorrelationIds,
    /// Family-specific payload as raw JSON. M1+ will narrow this per
    /// family.
    pub payload: serde_json::Value,
}

impl RuntimeEventEnvelope {
    /// Construct a new envelope with the current schema version.
    pub fn new(
        event_type: RuntimeEventType,
        payload_family: impl Into<String>,
        correlation: CorrelationIds,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            schema_version: SchemaVersion::current(),
            event_type,
            payload_family: RuntimeEventPayloadFamily::new(payload_family),
            emitted_at: chrono::Utc::now().to_rfc3339(),
            correlation,
            payload,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_round_trips_through_json() {
        let env = RuntimeEventEnvelope::new(
            RuntimeEventType::Conversation,
            "delta",
            CorrelationIds {
                session_id: Some("s1".into()),
                run_id: Some("r1".into()),
                ..CorrelationIds::default()
            },
            serde_json::json!({ "text": "hello" }),
        );
        let s = serde_json::to_string(&env).unwrap();
        let back: RuntimeEventEnvelope = serde_json::from_str(&s).unwrap();
        assert_eq!(env, back);
        assert_eq!(back.event_type, RuntimeEventType::Conversation);
        assert_eq!(back.payload_family.0, "delta");
        assert_eq!(back.correlation.run_id.as_deref(), Some("r1"));
    }

    #[test]
    fn schema_version_defaults_to_current() {
        let v = SchemaVersion::default();
        assert_eq!(v.0, CONTRACTS_SCHEMA_VERSION);
    }

    #[test]
    fn correlation_ids_attempt_id_round_trips() {
        let corr = CorrelationIds {
            session_id: Some("s1".into()),
            run_id: Some("r1".into()),
            stream_id: Some("st1".into()),
            attempt_id: Some("att-42".into()),
            ..CorrelationIds::default()
        };
        let json = serde_json::to_string(&corr).unwrap();
        let back: CorrelationIds = serde_json::from_str(&json).unwrap();
        assert_eq!(back.attempt_id.as_deref(), Some("att-42"));
        assert_eq!(back.run_id.as_deref(), Some("r1"));
    }

    #[test]
    fn correlation_ids_attempt_id_skipped_when_none() {
        let corr = CorrelationIds {
            session_id: Some("s1".into()),
            ..CorrelationIds::default()
        };
        let json = serde_json::to_string(&corr).unwrap();
        assert!(
            !json.contains("attemptId"),
            "attempt_id should be skipped when None: {json}"
        );
    }

    #[test]
    fn correlation_ids_team_fields_round_trip() {
        let corr = CorrelationIds {
            session_id: Some("s1".into()),
            run_id: Some("r1".into()),
            team_id: Some("t1".into()),
            member_id: Some("m1".into()),
            role_id: Some("planner".into()),
            parent_run_id: Some("r0".into()),
            delegation_id: Some("d1".into()),
            ..CorrelationIds::default()
        };
        let json = serde_json::to_string(&corr).unwrap();
        let back: CorrelationIds = serde_json::from_str(&json).unwrap();
        assert_eq!(back.team_id.as_deref(), Some("t1"));
        assert_eq!(back.delegation_id.as_deref(), Some("d1"));
    }
}
