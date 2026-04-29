// Transport boundary entry point (Phase M2.1).
//
// Re-exports the canonical contracts so consumers can write
// `import { ... } from '@/transport'` instead of reaching into
// `@/transport/contracts` directly. The `tauri.ts` thin bridge
// (`@/lib/tauri`) continues to own `invoke(...)` / `listen(...)`
// helpers; this barrel only covers transport-level **types**.

export * from './contracts'
export * from './gateway'
export * from './runtime-event-payloads'
export * from './runtime-event-translator'
export * from './runtime-event-reducer'
