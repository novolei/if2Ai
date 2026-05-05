/**
 * GF-01 PR-09 — DT-03 cutover shim.
 *
 * This file used to be the 1,100-line `ChatUI` orchestrator. PR-09
 * moved the orchestrator to `@/components/chat/chat-ui/ChatUI` and
 * replaced its `messages: Message[]` prop with an internal
 * `useRuntimeProjectionSelector` + `useConversation(sessionId)`
 * subscription so the chat surface owns its own data plane.
 *
 * This file is now a re-export shim. It remains in place so the
 * many child modules that import the `Message` and
 * `ComposerDropItem` types from this path keep compiling without a
 * mass rename. A follow-up PR may relocate those types and delete
 * the shim entirely.
 */
export {
  ChatUI,
  type Message,
  type ChatUIProps,
  type ComposerDropItem,
} from "@/components/chat/chat-ui/ChatUI"
