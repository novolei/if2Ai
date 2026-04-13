# Skills Control Plane v1

## 1. Background

if2Ai currently supports local skill discovery (`skill`, `skill_search`) from project/user directories, but does not yet provide a governed Skills platform for:

- app bundled skills
- remote distribution (e.g. `skills.sh`)
- frontend authoring and lifecycle management
- enable/disable policy control
- prompt/security review pipeline
- agent-authored skill proposals

At the same time, ADR-010 explicitly rejects an open marketplace-style Skills Hub due to security and trust risks.

The target of this document is to define a **controlled, auditable, policy-first Skills platform** that preserves ADR-010 security posture while enabling future product capabilities.

## 2. Scope

### In Scope

- SkillsControlPlane architecture
- Trust model and source precedence
- Bundled skills runtime strategy
- Security review gates (prompt + skill)
- Frontend management UX boundaries
- Agent-authored skill lifecycle
- Metrics and operational governance

### Out of Scope

- Any direct adoption of ungoverned external marketplace model
- Direct runtime permission escalation by skill metadata
- Autonomous publish-to-active without review

## 3. Design Principles

1. **Policy before capability**: no feature (download/edit/agent-create) may bypass control-plane gates.
2. **Deterministic source precedence**: same input must produce same active skill set.
3. **Audit by default**: every install/enable/disable/review/execute event must be traceable.
4. **Fail closed on trust**: unsigned or unreviewed skills are non-runnable by default.
5. **Human-in-the-loop for activation**: generated or downloaded skills enter draft/quarantine first.

## 4. Trust Model and Source Precedence

Skill sources in v1:

- `builtin` (app-bundled, read-only baseline)
- `workspace` (team/project managed)
- `user` (local personal)
- `remote` (downloaded artifacts, quarantined until verified)

Precedence (v1 recommendation):

1. `workspace`
2. `user`
3. `builtin`
4. `remote` (never auto-active; promoted only after validation/review)

Notes:

- `remote` acts as an acquisition channel, not an active runtime tier.
- conflicts are surfaced in UI and audit logs as deterministic shadowing decisions.

## 5. Bundled Skills Strategy

### 5.1 Source of truth (initial)

Existing bundled skill corpus is currently maintained at:

- `docs/references/bundled-skills`

### 5.2 App packaging strategy (v1)

Use Tauri bundle resources to package this corpus into app artifacts.

v1 decision:

- keep current source path unchanged to avoid risky bulk move
- include it through `src-tauri/tauri.conf.json` bundle resources
- treat bundled corpus as read-only at runtime

### 5.3 Future optimization

Introduce sync pipeline:

- source: `docs/references/bundled-skills`
- packaged target: `src-tauri/resources/bundled-skills` (generated mirror)
- checksum manifest validation during CI

## 6. Skill Manifest Contract

Each skill should have:

- `SKILL.md` (human-readable instruction spec)
- `skill.json` (machine policy manifest)

`skill.json` minimal fields:

- `id`, `name`, `version`, `apiVersion`
- `source`, `origin`, `minAppVersion`
- `capabilities`, `requiredTools`
- `integrity` (`sha256`, optional `signature`)
- `review` (`status`, `riskLevel`, `lastReviewedAt`)

## 7. Security and Review Pipeline

## 7.1 Prompt Security Gate

Detect and block patterns such as:

- covert instruction override
- hidden tool invocation directives
- data exfiltration intent templates
- persistent jailbreak payloads

## 7.2 Skill Security Gate

Check:

- tool allowlist consistency
- suspicious command patterns
- unsupported capability declarations
- missing integrity metadata

## 7.3 Review states

- `draft`
- `quarantine`
- `review_passed`
- `active`
- `disabled`
- `revoked`

Transition to `active` requires review pass + policy allow + user confirmation.

## 8. Frontend Product Capabilities

v1 management UI capabilities:

- list/search/filter skill inventory
- view source/version/risk/status/shadowing
- create/edit in draft mode
- enable/disable with explicit confirmation
- review report display and remediation hints
- conflict diagnostics (why current skill is shadowed)

Explicit non-goal:

- one-click bypass for blocked security review

## 9. Agent-Authored Skill Lifecycle

Agent can propose skill drafts, but cannot directly activate.

Flow:

1. agent generates draft skill package
2. policy/security review runs
3. reviewer (human/user) approves or rejects
4. approved draft is promoted to `active`
5. post-deploy telemetry monitored for regressions

Mandatory constraint:

- skill proposals never bypass `ToolExecutionBroker + PermissionPolicy + AuditEmitter`.

## 10. skills.sh Distribution Model

`skills.sh` integration is introduced as controlled channel:

- fetched artifacts are quarantined by default
- signature/hash verification required before import
- compatibility checks (`minAppVersion`, `apiVersion`) required
- policy review required before enable

No direct install-to-active path is allowed.

## 11. Observability and Ops Metrics

Baseline metrics:

- `skill_install_success_rate`
- `skill_review_block_rate`
- `skill_enable_success_rate`
- `skill_runtime_error_rate`
- `skill_rollback_rate`
- `skill_source_distribution`
- `skill_version_skew_count`
- `agent_generated_skill_adoption_rate` (phase-later)

Dimension tags:

- `source`, `workdir`, `session_size`, `model`, `skill_id`, `risk_level`

## 12. Priority Roadmap (P0 -> P3)

## P0 (blocking foundation)

- SkillsControlPlane core
- trust model + precedence engine
- manifest parser/validator
- integrity/signature framework
- prompt/skill security gates
- audit event model

## P1 (user governance UX)

- frontend skill management page
- create/edit draft workflow
- enable/disable policy path
- review report and conflict diagnostics
- compatibility quarantine UX

## P2 (distribution)

- `skills.sh` channel integration
- stable/canary channel policy
- artifact rollback and provenance tracing

## P3 (agent autonomy)

- agent-authored draft generation
- assisted remediation suggestions
- approval workflow automation hooks
- adoption and safety optimization loop

## 13. Relation to ADR-010

This design does not reintroduce an untrusted open Skills Hub.

It reframes ADR-010 into:

- reject ungoverned marketplace-style ingestion
- allow governed, policy-first skill lifecycle with strict safety controls

## 14. Definition of Done (v1 program-level)

- deterministic precedence behavior is tested
- no unsigned/unreviewed skill reaches active state
- frontend supports end-to-end draft->review->enable flow
- bundled skills packaged and discoverable in app runtime
- `skills.sh` artifacts are quarantined until verified
- audit and metrics available for operations
