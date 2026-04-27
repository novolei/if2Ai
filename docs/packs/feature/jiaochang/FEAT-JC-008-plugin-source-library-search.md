# FEAT-JC-008: Plugin Source Library Search

Status: done
Completed: 2026-04-26

## Goal

把校场插件音源从“只能注册和解析 URL”的设置能力，推进到可在校场音乐播放器内作为候选曲目加入曲库。继续遵守 CeruMusic 式边界：track metadata 不等于 playable URL，播放前仍必须通过 Rust-side worker 解析真实 URL。

## Spec

- 插件 manifest 注册后持久化到 `~/.if2ai/jiaochang/audio/plugin-sources.json`
- 插件 enabled 状态可持久化启停，重启后仍能通过 list 命令恢复
- 设置页补充 `source_url` / `signature` 字段，并显示来源可信提示
- 设置页展示权限说明：renderer 隔离、allowed_hosts 网络权限、签名/来源提示
- 校场音乐播放器增加曲库搜索框
- 搜索框可过滤当前 queue，也可基于启用的插件 manifest 生成 plugin candidate
- plugin candidate 加入曲库后只保存 metadata，不保存 playable URL
- 点击播放 plugin track 时仍走 `resolveTrackUrl` → `createPluginSourceAdapter` → Tauri command → Rust worker/cache

## Files

- `src-tauri/src/modules/jiaochang_audio/mod.rs`
- `src-tauri/src/commands/jiaochang_audio.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/commands/command_surface.rs`
- `src/modules/jiaochang/audio/plugin-source-adapter.ts`
- `src/modules/jiaochang/audio/plugin-source-settings.ts`
- `src/modules/jiaochang/audio/track-library.ts`
- `src/modules/jiaochang/audio/JiaochangAudioProvider.tsx`
- `src/modules/jiaochang/components/JiaochangMusicPlayer.tsx`
- `src/modules/jiaochang/i18n/*`
- `src/modules/settings/pages/JiaochangAudioSettingsPage.tsx`

## Out Of Scope

- 不内置第三方曲库
- 不绕过平台授权或 DRM
- 不在 renderer 执行插件代码
- 不实现完整插件市场、签名验证 PKI、远程 manifest 安装器

## Verify

- `node --experimental-strip-types --import='data:text/javascript,import {register} from "node:module"; import {pathToFileURL} from "node:url"; register("./scripts/test-loader.mjs", pathToFileURL("./"))' --test 'src/modules/jiaochang/**/*.test.ts'`
- `cargo test -p if2ai-backend modules::jiaochang_audio --lib`
- `npm run build:web`

## Result

- Jiaochang audio tests pass: 21/21
- Jiaochang Rust plugin tests pass: 4/4
- Web build passes
