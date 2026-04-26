# FEAT-SB-004: Agentic Browser + Cloud Escalation

## Status

- State: `draft`
- Owner: `@executor`
- Depends On: `FEAT-SB-003`
- Last Updated: `2026-04-25`

## Goal
Enable high-level browser-use agent retries and optional cloud escalation with explicit user approval, strong risk gates, and visible Smart Browser timeline state.

## Spec
- `retry_with_browser_use_agent` is available only through Smart Browser policy, not as a raw MCP tool → 测试 `modules::smart_browser::agentic::tests::hides_raw_agent_tool`
- Cloud escalation requires explicit user approval and records reason/backend transition → 测试 `modules::smart_browser::policy::tests::cloud_escalation_requires_approval`
- Sensitive actions are blocked or require approval: login, payment, personal data submit, file upload, cookie/profile sync → 测试 `modules::smart_browser::policy::tests::sensitive_actions_are_gated`
- UI shows escalation state and human takeover options → e2e step in `e2e/smart-browser-cockpit.spec.ts`

## Files (scope)
- `src-tauri/src/modules/smart_browser/agentic.rs` (new)
- `src-tauri/src/modules/smart_browser/cloud.rs` (new)
- `src-tauri/src/modules/smart_browser/policy.rs`
- `src-tauri/src/modules/runtime/permissions.rs`
- `src/components/browser/BrowserCard.tsx`
- `src/modules/browser-viewer/BrowserViewerPage.tsx`
- `src/modules/smart-browser/SmartBrowserCockpit.tsx` (new)
- `src/lib/tauri.ts`

## Reads
- `docs/design-docs/smart-browser-architecture.md`
- `src-tauri/src/modules/runtime/pending_permission.rs`
- `src-tauri/src/modules/runtime/permissions.rs`
- `src/components/browser/BrowserCard.tsx`
- `https://github.com/browser-use/browser-use/blob/main/AGENTS.md`
- `https://github.com/browser-use/browser-use/blob/main/README.md`

## Contract
- Never sync cookies/profile to browser-use cloud without explicit user action
- Never auto-approve form submission, file upload, payment, or account mutation
- Preserve existing user takeover pause semantics
- All agentic/cloud steps must appear in Smart Browser timeline

## Out Of Scope
- ❌ 不实现 scheduler/cron browser tasks
- ❌ 不实现 remote browser fleet management
- ❌ 不替换 local Rust CDP backend

## Verify
- ./scripts/pack run FEAT-SB-004
- cargo test --manifest-path src-tauri/Cargo.toml smart_browser
- npm run build
- manual Tauri check: CAPTCHA/blocked page shows approval/escalation state, not blind retry

## Done
- 上面 verify 全 PASS
- REGISTRY 状态改为 done
