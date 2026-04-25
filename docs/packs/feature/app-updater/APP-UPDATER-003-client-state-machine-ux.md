# APP-UPDATER-003: Client State Machine + UX

## Status
- State: active

## Goal
把设置页 updater 从一次性“检查/下载”按钮升级成产品级客户端状态机：后端发布 runtime state event，前端订阅并展示 checking / available / downloading / downloaded / installing / latest / error，提供自动检查偏好、可恢复错误和清晰 CTA。

## Spec (verifiable)
- 后端维护进程级 updater state，事件名 `app-updater://state`
- typed commands：
  - `app_updater_get_state`
  - `app_updater_check`
  - `app_updater_download_and_install`
  - `app_updater_check_manifest`
  - `app_updater_download_and_open`
  - `app_updater_set_preferences`
- 前端 updater API facade 不直接暴露 raw `invoke`
- About 页只保留一个“软件更新”操作中心，Hero 不再重复检查按钮
- CTA 按状态动态变化：检查更新、下载并安装、查看发布说明、复制诊断信息
- 下载中展示进度；无 total 时展示 indeterminate 进度
- 自动检查偏好持久化，后台检查尊重 `auto_check_enabled`

## Files (scope)
- `src-tauri/src/modules/updater/mod.rs`
- `src-tauri/src/commands/updater.rs`
- `src-tauri/src/commands/command_surface.rs`
- `src/api/updater.ts`
- `src/App.tsx`
- `src/modules/settings/pages/AboutSettingsPage.tsx`
- `src/modules/settings/pages/app-updater-state.ts`
- `src/modules/settings/pages/AboutSettingsPage.test.ts`
- `src/api/client.test.ts`

## Contract (review must check)
- UI 主文案面向用户，不直出 manifest/artifact 作为主要说明
- 诊断细节可见、可复制，但不吞掉错误
- 后端状态变更必须 emit；前端不能靠 toast 作为唯一状态
- `download_and_install` 使用 Tauri 签名安装链路；legacy download-open 仅保留为诊断/兼容路径

## Out of Scope
- 静默自动安装
- 多渠道切换 UI
- 全局导航 updater badge

## Verify
- `cargo test --manifest-path src-tauri/Cargo.toml updater::tests::`
- frontend targeted tests:
  `node --experimental-strip-types --import='data:text/javascript,import {register} from "node:module"; import {pathToFileURL} from "node:url"; register("./scripts/test-loader.mjs", pathToFileURL("./"))' --test src/api/client.test.ts src/modules/settings/pages/AboutSettingsPage.test.ts`
- `npm run build:web`

## Done
- 上面 verify PASS
- 设置页能从事件驱动状态更新，并清晰展示成功、下载、安装、失败恢复路径
