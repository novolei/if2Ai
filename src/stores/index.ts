// MIG-014 — `src/stores/*` barrel.
//
// Single import root for every chat / session / browser store.
// `import { useChatStore, sessionStore, ... } from '@/stores'`.

export * from './chat-store.ts'
export * from './session-store.ts'
// `browser-slice` has its own legacy entry point; we don't
// re-export it here to avoid name collisions (it predates the
// MIG-014 store stack).
