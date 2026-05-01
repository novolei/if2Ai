// MIG-012 — frontend API facade barrel.
//
// Single import root for every domain-scoped entry point.
// Pages / stores / shells SHOULD consume this barrel rather
// than reach into `src/api/*` submodules one-at-a-time, and
// MUST NOT reach into `@/lib/tauri` for business helpers.
//
// The legacy `@/lib/tauri` module is retained only as a
// transport bridge (invoke / listen primitives + wire DTO
// re-exports) until post-MIG-013 / MIG-014 when the business
// helpers that still live there can be retired. See the
// MIG-012 pack Cutover section.

export * from "./client";
export * from "./conversations";
export * from "./gateway-re-export";
export * from "./identity";
export * from "./onboarding";
export * from "./projects";
export * from "./sessions";
export * from "./slash";
export * from "./streaming";
export * from "./models";
export * from "./updater";
export * from "./window";
