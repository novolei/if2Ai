/**
 * 跨窗口状态同步工具
 *
 * 背景：if2ai 的 Settings 是通过 `openSettingsWindow()` 打开的**独立 Tauri 窗口**。
 * - `localStorage` 在不同窗口同源，但 React state 不知道要重新读
 * - `window.dispatchEvent(new CustomEvent(...))` 只在当前窗口触发
 * - `window.addEventListener('storage', ...)` 在 Tauri WebView 中**不可靠**
 *
 * 解决方案：用 **Tauri events**（默认 emit 到所有窗口 + listen 在当前窗口）
 * 做跨窗口广播。所有需要"设置窗口改完后主窗口立即同步"的状态走这条通道。
 *
 * ## 使用约定
 *
 * 频道名采用 `cross:` 前缀，便于和后端命令事件区分：
 * - `cross:agent-voice-changed`     — Agent 语音 id 变更
 * - `cross:agent-voice-enabled`     — 自动 TTS 开关变更
 * - `cross:stt-settings-changed`    — STT provider/key 变更
 * - `cross:tts-voices-changed`      — 用户自定义 voice 列表变更（上传/删除/重命名）
 * - `cross:onboarding-reset`        — 重置 Onboarding，主窗口需要重新加载 app state
 *
 * ## 示例
 *
 * ```ts
 * // 设置窗口（写入端）
 * await broadcastChange('cross:agent-voice-changed', { id: 'zh_1' })
 *
 * // 主窗口（读取端）
 * useCrossWindowChange('cross:agent-voice-changed', (payload) => {
 *   setVoiceId(payload.id)
 * })
 * ```
 */

import { useEffect } from 'react'
import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event'

export type CrossWindowChannel =
  | 'cross:agent-voice-changed'
  | 'cross:agent-voice-enabled'
  | 'cross:stt-settings-changed'
  | 'cross:tts-voices-changed'
  | 'cross:onboarding-reset'

/**
 * 把变更广播到所有窗口（包括当前窗口）。
 *
 * Tauri 的 `emit()` 默认是 broadcast，所有窗口的 `listen()` 都会收到。
 * 这意味着调用方不需要再手动 dispatch 本地事件。
 */
export async function broadcastChange<T = unknown>(
  channel: CrossWindowChannel,
  payload?: T,
): Promise<void> {
  try {
    await emit(channel, payload)
  } catch (err) {
    // Tauri runtime 不可用时（例如本地测试 / Storybook）退化为本窗口 CustomEvent
    console.warn('[crossWindowSync] emit failed, fallback to local CustomEvent:', err)
    try {
      window.dispatchEvent(new CustomEvent(channel, { detail: payload }))
    } catch {
      /* ignore */
    }
  }
}

/**
 * React Hook：监听跨窗口变更。
 *
 * @param channel  跨窗口频道名（必须用 `cross:` 前缀的预定义类型）
 * @param handler  变更回调；接收 emit 时传入的 payload
 *
 * Hook 会自动管理 `listen` 的注册和清理。也兼听本窗口的 CustomEvent fallback，
 * 这样 broadcastChange 在 Tauri 不可用时退化路径也能 work。
 */
export function useCrossWindowChange<T = unknown>(
  channel: CrossWindowChannel,
  handler: (payload: T) => void,
): void {
  useEffect(() => {
    let unlisten: UnlistenFn | null = null
    let mounted = true

    void listen<T>(channel, (event) => {
      handler(event.payload)
    }).then((un) => {
      if (mounted) unlisten = un
      else un()
    }).catch((err) => {
      console.warn('[crossWindowSync] listen failed:', err)
    })

    // CustomEvent fallback（Tauri 不可用时的退化路径）
    const customHandler = (e: Event) => {
      const detail = (e as CustomEvent<T>).detail
      handler(detail)
    }
    window.addEventListener(channel, customHandler as EventListener)

    return () => {
      mounted = false
      unlisten?.()
      window.removeEventListener(channel, customHandler as EventListener)
    }
    // 故意省略 handler — 调用方应包成 useCallback 或允许任意闭包
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [channel])
}
