// MIG-013 — `src/app/*` barrel.
//
// Single import root for the canonical AppShell / ContentRouter
// containers. App.tsx is the only caller today; future tests
// and specialized-surface stubs mount these directly.

export * from './AppShell.tsx'
export * from './ContentRouter.tsx'
