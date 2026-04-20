// Runtime projection pipeline barrel (Phase M2.2 + M2.3).
//
// Single canonical import point for callers that wire the pipeline
// (M2.4 stores, future dev inspector). Re-exports the type / event
// alphabet plus the translator + queue + reducer.

export * from './types'
export * from './runtime-event-translator'
export * from './runtime-event-queue'
export * from './runtime-event-reducer'
export * from './runtime-projection-store'
export * from './runtime-projection-bridge'
export * from './use-runtime-projection'
export * from './use-execution-mode-preview'
