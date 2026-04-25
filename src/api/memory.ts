/**
 * Memory API facade — single entry point for everything memory.
 *
 * Why this module exists (Memory System Audit P1 #6):
 *
 *   Before: each surface (MemoryBrowser / MemorySettings / TelemetryDrawer
 *   / chat MemoryChip / pinned editor / compiled viewer / narrative viewer)
 *   imported memory IPCs straight from `@/lib/tauri`, kept its own
 *   loading/error/state logic, and had to subscribe to the
 *   `memory.invalidationVersion` selector by hand to stay fresh.
 *
 *   After: components import from `@/api/memory`. The raw IPCs are
 *   still re-exported (no breaking change), but a small set of hooks
 *   wraps the most common patterns:
 *
 *     - `useMemoryEntries(args)`  — load+refetch entries with the right
 *                                   scope hint, automatically refetch
 *                                   on `memory_invalidated` events
 *                                   whose `scope_kind` matches.
 *     - `useMemoryPromotionCandidates()` — same pattern for promotion
 *                                   recommendations.
 *
 * Goals satisfied:
 *
 *   1. Single import path for memory in the React layer.
 *   2. No per-component invalidation boilerplate (every consumer used
 *      to copy-paste the same 8 lines).
 *   3. Future cache / dedup / batching can live here without touching
 *      consumers — they only see the hook signature.
 *
 * Non-goals:
 *
 *   - Not a Zustand store. Each consumer keeps its own copy of the
 *     entries it asked for; concurrent overlapping queries from
 *     different surfaces still hit the IPC twice. If that becomes a
 *     real perf problem, swap the hook body for a SWR/React Query
 *     adapter — the public surface stays stable.
 */

import { useCallback, useEffect, useRef, useState } from "react";

import { useRuntimeProjectionSelector } from "@/runtime-projection";
import {
  memoryClearAll,
  memoryCompiledClear,
  memoryCompiledRead,
  memoryCompileNow,
  memoryDelete,
  memoryDemote,
  memoryExport,
  memoryPromote,
  memoryPromotionCandidates,
  memoryRecall,
  memorySummariesList,
  pinnedGet,
  type CompileReport,
  type CompileResult,
  type CompileSkipReason,
  type CompiledMemoryDto,
  type MemoryEntryDto,
  type MemoryPromotionCandidateDto,
  type MemoryScopeArgs,
  type MemoryScopeKind,
  type PinnedItemDto,
  type SessionSummaryDto,
} from "@/lib/tauri";

// ─── Re-exports ──────────────────────────────────────────────────────
//
// Components should `import { ... } from "@/api/memory"` from here on
// out; the lib/tauri path stays available for non-React callers.

export {
  memoryClearAll,
  memoryCompiledClear,
  memoryCompiledRead,
  memoryCompileNow,
  memoryDelete,
  memoryDemote,
  memoryExport,
  memoryPromote,
  memoryPromotionCandidates,
  memoryRecall,
  memorySummariesList,
  pinnedGet,
};
export type {
  CompileReport,
  CompileResult,
  CompileSkipReason,
  CompiledMemoryDto,
  MemoryEntryDto,
  MemoryPromotionCandidateDto,
  MemoryScopeArgs,
  MemoryScopeKind,
  PinnedItemDto,
  SessionSummaryDto,
};

// ─── Invalidation helper ────────────────────────────────────────────

/**
 * Backend `memory_invalidated` events carry a coarse `scope_kind` hint
 * indicating *what kind of data changed*. This helper turns that hint
 * into a stable key for `useEffect` dependency arrays so unrelated
 * invalidations (e.g. a pinned add) don't trigger an entries refetch.
 *
 * Returns 0 when the last invalidation was for a different scope, or
 * the monotonic counter value when it was relevant. Components can
 * just `useEffect(..., [..., key])` and React handles the rest.
 */
export function useMemoryInvalidationKey(
  ...interestedScopes: Array<"entries" | "pinned" | "compiled" | "summaries" | "all">
): number {
  const version = useRuntimeProjectionSelector(
    (s) => s.memory.invalidationVersion,
  );
  const lastScope = useRuntimeProjectionSelector(
    (s) => s.memory.lastInvalidationScope,
  );
  // "all" is treated as a wildcard — every consumer reacts to wipe-all.
  if (lastScope === "all") return version;
  if (lastScope && interestedScopes.includes(lastScope as never)) {
    return version;
  }
  return 0;
}

// ─── useMemoryEntries hook ──────────────────────────────────────────

export interface UseMemoryEntriesArgs {
  /** Backend memory category filter; pass null for "all". */
  category?: string | null;
  /** Optional scope filter (session / project / global). */
  scope?: MemoryScopeArgs;
  /** Suspend fetching when false (e.g. tab not visible). */
  enabled?: boolean;
}

export interface MemoryEntriesQuery {
  entries: MemoryEntryDto[];
  loading: boolean;
  error: string | null;
  refetch: () => Promise<void>;
}

/**
 * Load + auto-refresh memory entries for one (category, scope) pair.
 *
 * Refetches on:
 *   - mount
 *   - `category` / `scope` arg changes
 *   - any backend `memory_invalidated` event with a matching scope hint
 *     (entries / all)
 *
 * Prevents stale-state races: a fast second invalidation supersedes
 * the previous in-flight request via a request-id ref.
 */
export function useMemoryEntries(
  args: UseMemoryEntriesArgs = {},
): MemoryEntriesQuery {
  const { category = null, scope, enabled = true } = args;
  const [entries, setEntries] = useState<MemoryEntryDto[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const reqIdRef = useRef(0);

  const invalidationKey = useMemoryInvalidationKey("entries");

  const refetch = useCallback(async () => {
    if (!enabled) return;
    const reqId = ++reqIdRef.current;
    setLoading(true);
    setError(null);
    try {
      const result = await memoryExport({ category, scope });
      // Drop late results — only the most recent request wins.
      if (reqId === reqIdRef.current) setEntries(result);
    } catch (e) {
      if (reqId === reqIdRef.current) setError(String(e));
    } finally {
      if (reqId === reqIdRef.current) setLoading(false);
    }
  }, [category, scope, enabled]);

  useEffect(() => {
    void refetch();
    // refetch is stable per (category, scope, enabled); invalidationKey
    // bumps when an "entries" invalidation arrives.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refetch, invalidationKey]);

  return { entries, loading, error, refetch };
}

// ─── usePromotionCandidates hook ────────────────────────────────────

export interface PromotionCandidatesQuery {
  candidates: MemoryPromotionCandidateDto[];
  loading: boolean;
  error: string | null;
  refetch: () => Promise<void>;
}

/**
 * Load + auto-refresh promotion recommendations.
 *
 * Same invalidation contract as `useMemoryEntries` — promotion
 * candidate state is a function of entries, so any "entries" or "all"
 * invalidation triggers a refresh.
 */
export function useMemoryPromotionCandidates(
  enabled = true,
): PromotionCandidatesQuery {
  const [candidates, setCandidates] = useState<MemoryPromotionCandidateDto[]>(
    [],
  );
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const reqIdRef = useRef(0);

  const invalidationKey = useMemoryInvalidationKey("entries");

  const refetch = useCallback(async () => {
    if (!enabled) return;
    const reqId = ++reqIdRef.current;
    setLoading(true);
    setError(null);
    try {
      const result = await memoryPromotionCandidates();
      if (reqId === reqIdRef.current) setCandidates(result);
    } catch (e) {
      if (reqId === reqIdRef.current) setError(String(e));
    } finally {
      if (reqId === reqIdRef.current) setLoading(false);
    }
  }, [enabled]);

  useEffect(() => {
    void refetch();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refetch, invalidationKey]);

  return { candidates, loading, error, refetch };
}
