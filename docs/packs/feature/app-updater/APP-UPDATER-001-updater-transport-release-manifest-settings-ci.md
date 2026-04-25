# APP-UPDATER-001: App Updater Transport + Manifest + Settings State

## Status
- State: active

## Goal
把桌面应用更新能力从零散配置升级为可验证的产品闭环：后端 updater transport、release manifest 校验、设置页状态机、CI 发布约束一次纳入同一个 Pack。

## Spec (verifiable)
- updater transport 能读取 manifest、区分 no-update / update-available / failed → 测试 `updater::tests::manifest_status_matrix`
- release manifest 解析校验 version、platform、artifact url、signature/checksum → 测试 `updater::tests::release_manifest_validation_rejects_invalid_artifacts`
- 设置页展示 idle / checking / available / downloading / ready / failed 状态，并不阻塞其它设置项 → 测试 `settings_ui::tests::app_updater_state_machine_renders`
- 手动检查更新只走 typed API facade，不在 UI 中直接调用 raw `invoke` → 测试 `api::tests::updater_commands_use_typed_facade`
- CI 发布任务必须产出 manifest、artifact、checksum/signature，并防止未签名 release 进入 stable channel → e2e/CI step `release-ci::app_updater_manifest_gate`

## Files (scope)
- `src-tauri/src/modules/updater/**` (new)
- `src-tauri/src/commands/updater.rs` (new)
- `src-tauri/src/commands/command_surface.rs`
- `src/api/updater.ts` (new)
- `src/modules/settings/pages/AboutSettingsPage.tsx`
- `src/modules/settings/pages/UpdateSettingsPage.tsx` (new if needed)
- `.github/workflows/**` or existing release CI workflow
- `docs/packs/REGISTRY.md`

## Reads
- `src-tauri/tauri.conf.json`
- `src-tauri/tauri.conf.release.json` (if present)
- `src/modules/settings/pages/AboutSettingsPage.tsx`
- Existing release workflow files under `.github/workflows/`

## Contract (review must check)
- 不引入未声明的新 updater 服务商或发布渠道
- manifest schema 必须稳定、可版本化、可向后兼容
- update check / download / install 的错误要可诊断，不用泛化 toast 吞掉
- 设置页只展示状态与动作，不承担 release policy 判断
- CI gate 是 stable channel 的唯一发布约束真相

## Out of Scope
- 不做自动后台静默安装
- 不做多渠道订阅 UI
- 不重构整个 Settings shell
- 不改变现有 onboarding / provider 设置流

## Verify
- `./scripts/pack run APP-UPDATER-001`
- CI release dry-run / manifest validation job 通过

## Done
- 上面 verify 全 PASS
- 设置页能手动检查并呈现 updater 状态
- REGISTRY 状态改为 done
