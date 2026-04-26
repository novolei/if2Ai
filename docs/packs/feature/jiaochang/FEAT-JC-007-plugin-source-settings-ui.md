# FEAT-JC-007: Plugin Source Settings UI

## Status
- State: done
- Completed: 2026-04-26

## Goal
把 FEAT-JC-006 的插件音源 sandbox 做成可操作设置界面：注册 manifest、查看插件列表、清理服务端 URL cache，并显示 worker/event bridge 日志。

## Spec (verifiable)
- 设置页新增「校场音源」入口
- Manifest 表单支持 plugin_id/name/version/enabled/allowed_hosts/resolver_template
- 注册前做前端校验，注册通过 typed Tauri command helper
- 列表读取后端 registered manifest，可点选回填表单
- Cache 面板读取 entries，并可触发 cache clear command
- 事件日志通过 typed helper 订阅 `jiaochang://audio/plugin-event`，页面不直接订阅 raw Tauri events
- UI 不执行插件代码，不散落 URL/cache 解析逻辑

## Files (scope)
- `src/modules/settings/**`
- `src/modules/jiaochang/audio/plugin-source-adapter.ts`
- `src/modules/jiaochang/audio/plugin-source-settings.ts`
- `src/modules/jiaochang/audio/plugin-source-settings.test.ts`
- `docs/packs/feature/jiaochang/FEAT-JC-007-plugin-source-settings-ui.md`
- `docs/packs/REGISTRY.md`

## Reads
- `docs/packs/feature/jiaochang/FEAT-JC-006-plugin-source-sandbox.md`
- `docs/product-specs/jiaochang-pixel-agent-board-prd.md` FEAT-JC-006/007 notes

## Contract (review must check)
- 不执行 renderer 插件代码
- 不内置第三方曲库
- 不绕过 `allowed_hosts` / Rust worker 校验
- 不新增 raw Tauri event subscription 到 UI 组件
- 不新增 dependency

## Out of Scope
- 插件市场、下载、签名、权限审批 UI
- 歌曲搜索/歌单/歌词配置
- 真实音源插件运行时脚本执行
- 缓存音频字节

## Verify
- `node --experimental-strip-types --import='data:text/javascript,import {register} from "node:module"; import {pathToFileURL} from "node:url"; register("./scripts/test-loader.mjs", pathToFileURL("./"))' --test 'src/modules/jiaochang/**/*.test.ts'`
- `npm run build:web`

## Done
- 上面 targeted verify PASS
- REGISTRY 状态改为 done
