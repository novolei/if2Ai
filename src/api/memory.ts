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
  daydreamStatus,
  daydreamTrigger,
  daydreamConfigGet,
  daydreamConfigSet,
  daydreamHistory,
  parseDayDreamState,
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
  type DayDreamState,
  type DayDreamStateKind,
  type DayDreamConfig,
  type DayDreamReport,
  type ConsolidationStrategy,
  type PruneReport,
  type MergeReport,
  type RefreshReport,
  type InsightCategory,
  type InsightDto,
  type TrajectoryDto,
  type ToolCallRecord,
  type TurnRecord,
  type DayDreamReflectionReport,
  memoryGraphTraverse,
  memoryGraphNeighborhood,
  memoryGraphDiscover,
  memoryGraphFull,
  type MemoryLinkDto,
  type GraphEntryDto,
  type GraphNodeDto,
  type GraphNeighborhoodDto,
  type FullGraphDto,
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
  daydreamStatus,
  daydreamTrigger,
  daydreamConfigGet,
  daydreamConfigSet,
  daydreamHistory,
  parseDayDreamState,
  memoryGraphTraverse,
  memoryGraphNeighborhood,
  memoryGraphDiscover,
  memoryGraphFull,
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
  DayDreamState,
  DayDreamStateKind,
  DayDreamConfig,
  DayDreamReport,
  ConsolidationStrategy,
  PruneReport,
  MergeReport,
  RefreshReport,
  InsightCategory,
  InsightDto,
  TrajectoryDto,
  ToolCallRecord,
  TurnRecord,
  DayDreamReflectionReport,
  MemoryLinkDto,
  GraphEntryDto,
  GraphNodeDto,
  GraphNeighborhoodDto,
  FullGraphDto,
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

// ─── DayDream hooks ─────────────────────────────────────────────────

export interface DayDreamStatusQuery {
  state: DayDreamState | null;
  parsed: { kind: DayDreamStateKind; startedAt?: string; finishedAt?: string } | null;
  loading: boolean;
  error: string | null;
  refetch: () => Promise<void>;
}

/**
 * Load + auto-refresh DayDream engine status.
 *
 * Polls once on mount and can be manually refetched. The `parsed`
 * field normalizes the Rust externally-tagged enum into a flat
 * `{ kind, startedAt?, finishedAt? }` shape for display convenience.
 */
export function useDayDreamStatus(): DayDreamStatusQuery {
  const [state, setState] = useState<DayDreamState | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const reqIdRef = useRef(0);

  const refetch = useCallback(async () => {
    const reqId = ++reqIdRef.current;
    setLoading(true);
    setError(null);
    try {
      const result = await daydreamStatus();
      if (reqId === reqIdRef.current) setState(result);
    } catch (e) {
      if (reqId === reqIdRef.current) setError(String(e));
    } finally {
      if (reqId === reqIdRef.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refetch();
  }, [refetch]);

  return {
    state,
    parsed: state ? parseDayDreamState(state) : null,
    loading,
    error,
    refetch,
  };
}

export interface DayDreamConfigQuery {
  config: DayDreamConfig | null;
  loading: boolean;
  error: string | null;
  update: (config: DayDreamConfig) => Promise<void>;
  refetch: () => Promise<void>;
}

/**
 * Load, update, and auto-refresh DayDream configuration.
 */
export function useDayDreamConfig(): DayDreamConfigQuery {
  const [config, setConfig] = useState<DayDreamConfig | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const reqIdRef = useRef(0);

  const refetch = useCallback(async () => {
    const reqId = ++reqIdRef.current;
    setLoading(true);
    setError(null);
    try {
      const result = await daydreamConfigGet();
      if (reqId === reqIdRef.current) setConfig(result);
    } catch (e) {
      if (reqId === reqIdRef.current) setError(String(e));
    } finally {
      if (reqId === reqIdRef.current) setLoading(false);
    }
  }, []);

  const update = useCallback(async (newConfig: DayDreamConfig) => {
    setError(null);
    try {
      await daydreamConfigSet(newConfig);
      setConfig(newConfig);
    } catch (e) {
      setError(String(e));
      throw e;
    }
  }, []);

  useEffect(() => {
    void refetch();
  }, [refetch]);

  return { config, loading, error, update, refetch };
}

export interface DayDreamHistoryQuery {
  reports: DayDreamReport[];
  loading: boolean;
  error: string | null;
  refetch: () => Promise<void>;
}

/**
 * Load DayDream history reports.
 */
export function useDayDreamHistory(): DayDreamHistoryQuery {
  const [reports, setReports] = useState<DayDreamReport[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const reqIdRef = useRef(0);

  const refetch = useCallback(async () => {
    const reqId = ++reqIdRef.current;
    setLoading(true);
    setError(null);
    try {
      const result = await daydreamHistory();
      if (reqId === reqIdRef.current) setReports(result);
    } catch (e) {
      if (reqId === reqIdRef.current) setError(String(e));
    } finally {
      if (reqId === reqIdRef.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refetch();
  }, [refetch]);

  return { reports, loading, error, refetch };
}

// ─── Evolution hooks ──────────────────────────────────────────────────────

export interface ProceduralMemoriesQuery {
  procedures: MemoryEntryDto[];
  loading: boolean;
  error: string | null;
  refetch: () => Promise<void>;
}

/**
 * Load procedural memories (category = "Procedural").
 *
 * These are rules and patterns the agent has learned from experience
 * via the SelfReflector → ProceduralMemoryManager pipeline.
 */
export function useProceduralMemories(): ProceduralMemoriesQuery {
  const { entries, loading, error, refetch } = useMemoryEntries({
    category: "Procedural",
  });
  return { procedures: entries, loading, error, refetch };
}

export interface InsightMemoriesQuery {
  insights: MemoryEntryDto[];
  loading: boolean;
  error: string | null;
  refetch: () => Promise<void>;
}

/**
 * Load insight memories (category = "Reflection").
 *
 * These are raw insights extracted by the SelfReflector before they
 * are promoted to procedural memory.
 */
export function useInsightMemories(): InsightMemoriesQuery {
  const { entries, loading, error, refetch } = useMemoryEntries({
    category: "Reflection",
  });
  return { insights: entries, loading, error, refetch };
}

/**
 * Load all evolution-related memories (Procedural + Reflection).
 *
 * Merges both categories and sorts by updated_at descending for
 * timeline display.
 */
export interface EvolutionMemoriesQuery {
  entries: MemoryEntryDto[];
  procedureCount: number;
  insightCount: number;
  loading: boolean;
  error: string | null;
  refetch: () => Promise<void>;
}

export function useEvolutionMemories(): EvolutionMemoriesQuery {
  const proc = useProceduralMemories();
  const ins = useInsightMemories();

  const entries = [...proc.procedures, ...ins.insights].sort((a, b) =>
    b.updated_at.localeCompare(a.updated_at),
  );

  const refetch = useCallback(async () => {
    await Promise.all([proc.refetch(), ins.refetch()]);
  }, [proc.refetch, ins.refetch]);

  return {
    entries,
    procedureCount: proc.procedures.length,
    insightCount: ins.insights.length,
    loading: proc.loading || ins.loading,
    error: proc.error || ins.error,
    refetch,
  };
}

// ─── Memory Graph hooks ───────────────────────────────────────────────────

export interface MemoryGraphQuery {
  nodes: GraphNodeDto[];
  loading: boolean;
  error: string | null;
  refetch: () => Promise<void>;
}

/**
 * BFS graph traversal from a seed key.
 * Returns nodes with hydrated entries and connected links.
 */
export function useMemoryGraph(
  seedKey: string | null,
  depth = 2,
): MemoryGraphQuery {
  const [nodes, setNodes] = useState<GraphNodeDto[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const reqIdRef = useRef(0);

  const refetch = useCallback(async () => {
    if (!seedKey) return;
    const reqId = ++reqIdRef.current;
    setLoading(true);
    setError(null);
    try {
      const result = await memoryGraphTraverse(seedKey, depth);
      if (reqId === reqIdRef.current) setNodes(result);
    } catch (e) {
      if (reqId === reqIdRef.current) setError(String(e));
    } finally {
      if (reqId === reqIdRef.current) setLoading(false);
    }
  }, [seedKey, depth]);

  useEffect(() => {
    void refetch();
  }, [refetch]);

  return { nodes, loading, error, refetch };
}

export interface MemoryNeighborhoodQuery {
  neighborhood: GraphNeighborhoodDto | null;
  loading: boolean;
  error: string | null;
  refetch: () => Promise<void>;
}

/**
 * Load the structured neighborhood of a memory node.
 */
export function useMemoryNeighborhood(
  key: string | null,
): MemoryNeighborhoodQuery {
  const [neighborhood, setNeighborhood] =
    useState<GraphNeighborhoodDto | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const reqIdRef = useRef(0);

  const refetch = useCallback(async () => {
    if (!key) return;
    const reqId = ++reqIdRef.current;
    setLoading(true);
    setError(null);
    try {
      const result = await memoryGraphNeighborhood(key);
      if (reqId === reqIdRef.current) setNeighborhood(result);
    } catch (e) {
      if (reqId === reqIdRef.current) setError(String(e));
    } finally {
      if (reqId === reqIdRef.current) setLoading(false);
    }
  }, [key]);

  useEffect(() => {
    void refetch();
  }, [refetch]);

  return { neighborhood, loading, error, refetch };
}

// ─── Full Memory Graph hook ─────────────────────────────────────────────────

export interface FullMemoryGraphQuery {
  graph: FullGraphDto | null;
  loading: boolean;
  error: string | null;
  refetch: () => Promise<void>;
}

/**
 * Load the full memory graph — all nodes and all links.
 *
 * Used by the MemoryGraphPanel for default full-graph visualization.
 */
export function useFullMemoryGraph(): FullMemoryGraphQuery {
  const [graph, setGraph] = useState<FullGraphDto | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const reqIdRef = useRef(0);

  const invalidationKey = useMemoryInvalidationKey("entries");

  const refetch = useCallback(async () => {
    const reqId = ++reqIdRef.current;
    setLoading(true);
    setError(null);
    try {
      const result = await memoryGraphFull();
      if (reqId === reqIdRef.current) setGraph(result);
    } catch (e) {
      if (reqId === reqIdRef.current) setError(String(e));
    } finally {
      if (reqId === reqIdRef.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refetch();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refetch, invalidationKey]);

  return { graph, loading, error, refetch };
}
