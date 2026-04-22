import { useMemo, useState } from "react";
import {
  ChevronDown,
  ChevronRight,
  Layers3,
  ListTree,
  Sparkles,
  Workflow,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type { PromptDiagnosticsSummary } from "@/lib/tauri";
import type { PromptDiagnosticsSnapshot } from "../storage";

interface PromptDiagnosticsPanelProps {
  snapshot: PromptDiagnosticsSnapshot | null;
  title?: string;
  description?: string;
  collapsible?: boolean;
  defaultExpanded?: boolean;
  compact?: boolean;
  className?: string;
}

type LaneStatus = "active" | "suppressed";

function formatTimestamp(value: number | null | undefined): string {
  if (!value) return "未记录";
  try {
    return new Date(value).toLocaleString("zh-CN", { hour12: false });
  } catch {
    return String(value);
  }
}

function shortenTraceId(traceId: string): string {
  if (traceId.length <= 18) return traceId;
  return `${traceId.slice(0, 10)}…${traceId.slice(-6)}`;
}

function humanizeLane(lane: string): string {
  return lane
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function laneDotTone(status: LaneStatus): string {
  return status === "active" ? "bg-jade" : "bg-black/15";
}

function laneBadgeTone(status: LaneStatus): string {
  return status === "active"
    ? "border-jade/20 bg-jade/8 text-jade"
    : "border-black/[0.07] bg-black/[0.03] text-muted-foreground";
}

function laneRowTone(status: LaneStatus, selected: boolean): string {
  if (selected && status === "active") {
    return "border-jade/25 bg-jade/[0.065] shadow-[0_1px_4px_rgba(0,0,0,0.04)]";
  }
  if (selected) {
    return "border-black/[0.09] bg-black/[0.05] shadow-[0_1px_4px_rgba(0,0,0,0.03)]";
  }
  return "border-black/[0.06] bg-black/[0.018] hover:bg-black/[0.028]";
}

function reasonTone(reason: string): string {
  if (reason.includes("custom_identity_pack")) {
    return "border-orange-200/70 bg-orange-50 text-orange-800";
  }
  if (reason.includes("persona") || reason.includes("identity")) {
    return "border-amber-200/70 bg-amber-50 text-amber-800";
  }
  if (reason.includes("tool")) {
    return "border-sky-200/70 bg-sky-50 text-sky-800";
  }
  if (reason.includes("memory")) {
    return "border-emerald-200/70 bg-emerald-50 text-emerald-800";
  }
  if (
    reason.includes("scenario") ||
    reason.includes("planning") ||
    reason.includes("review")
  ) {
    return "border-blue-200/70 bg-blue-50 text-blue-800";
  }
  return "border-black/[0.07] bg-black/[0.03] text-foreground/75";
}

function sourceTone(source: string): string {
  if (source.includes("custom_identity_pack"))
    return "border-orange-200/70 bg-orange-50 text-orange-800";
  if (source.includes("request_intelligence"))
    return "border-blue-200/70 bg-blue-50 text-blue-800";
  if (source.includes("tool_registry"))
    return "border-sky-200/70 bg-sky-50 text-sky-800";
  if (source.includes("memory"))
    return "border-emerald-200/70 bg-emerald-50 text-emerald-800";
  if (source.includes("identity"))
    return "border-amber-200/70 bg-amber-50 text-amber-800";
  if (source.includes("execution_mode"))
    return "border-violet-200/70 bg-violet-50 text-violet-800";
  return "border-black/[0.07] bg-black/[0.03] text-foreground/75";
}

function MetricTile(props: {
  label: string;
  value: string | number;
  compact?: boolean;
}) {
  return (
    <div
      className={cn(
        "rounded-[18px] border border-black/[0.06] bg-black/[0.018]",
        props.compact ? "px-3 py-2" : "px-3.5 py-3",
      )}
    >
      <div className="text-[10px] uppercase tracking-[0.16em] text-muted-foreground/65">
        {props.label}
      </div>
      <div
        className={cn(
          "mt-1 font-semibold tracking-[-0.02em] text-foreground/90",
          props.compact ? "text-[14px]" : "text-[16px]",
        )}
      >
        {props.value}
      </div>
    </div>
  );
}

function SectionTitle(props: { icon: typeof Layers3; label: string }) {
  const Icon = props.icon;
  return (
    <div className="mb-2.5 flex items-center gap-2 text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground/72">
      <Icon className="h-3.5 w-3.5" aria-hidden />
      {props.label}
    </div>
  );
}

function EmptyState(props: { compact?: boolean }) {
  return (
    <div
      className={cn(
        "rounded-[20px] border border-dashed border-black/[0.08] bg-black/[0.02] text-muted-foreground",
        props.compact ? "px-4 py-5 text-[12px]" : "px-5 py-8 text-[13px]",
      )}
    >
      完成一次 assistant turn 后，这里会显示最新的 prompt lane 决策摘要。
    </div>
  );
}

function SummarySentence(props: {
  summary: PromptDiagnosticsSummary;
  selectedLane: string | null;
}) {
  const activeNames = props.summary.lane_summaries
    .filter((lane) => lane.status === "active")
    .map((lane) => humanizeLane(lane.lane));

  const focus = props.selectedLane ? humanizeLane(props.selectedLane) : null;
  const focusText = focus
    ? `当前聚焦 ${focus} lane。`
    : "当前显示本轮整体 prompt 装配概况。";
  const activeText =
    activeNames.length > 0
      ? `活跃层包括 ${activeNames.join("、")}。`
      : "当前没有活跃 lane。";

  return (
    <div className="rounded-[20px] border border-black/[0.06] bg-black/[0.018] px-4 py-3 text-[12.5px] leading-6 text-foreground/74">
      <span className="font-medium text-foreground/84">{focusText}</span>{" "}
      <span>{activeText}</span>{" "}
      <span className="text-muted-foreground">
        本轮共组装 {props.summary.block_count} 个 blocks，记录了{" "}
        {props.summary.activation_reason_codes.length} 个 activation reasons。
      </span>
    </div>
  );
}

function EntryList(props: {
  title: string;
  count: number;
  entries: Array<{ id: string; meta?: string; badge?: string }>;
  tone: "active" | "suppressed";
}) {
  const toneClass =
    props.tone === "active"
      ? "border-jade/12 bg-jade/[0.045] text-foreground/88"
      : "border-black/[0.06] bg-black/[0.028] text-foreground/72";

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between gap-3">
        <div className="text-[11px] font-medium uppercase tracking-[0.12em] text-muted-foreground/70">
          {props.title}
        </div>
        <div className="text-[11px] tabular-nums text-muted-foreground/65">
          {props.count}
        </div>
      </div>
      {props.entries.length === 0 ? (
        <div className="rounded-2xl border border-dashed border-black/[0.07] px-3 py-3 text-[12px] text-muted-foreground/80">
          无
        </div>
      ) : (
        <div className="space-y-1.5">
          {props.entries.map((entry) => (
            <div
              key={entry.id}
              className={cn(
                "rounded-[14px] border px-3 py-2 text-[11.5px] leading-5",
                toneClass,
              )}
            >
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <div>{entry.id}</div>
                  {entry.meta ? (
                    <div className="mt-1 text-[10.5px] text-muted-foreground">
                      {entry.meta}
                    </div>
                  ) : null}
                </div>
                {entry.badge ? (
                  <span
                    className={cn(
                      "shrink-0 rounded-full border px-2 py-0.5 text-[10px] font-medium",
                      props.tone === "active"
                        ? sourceTone(entry.badge)
                        : reasonTone(entry.badge),
                    )}
                  >
                    {entry.badge}
                  </span>
                ) : null}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function LaneTable(props: {
  lanes: PromptDiagnosticsSummary["lane_summaries"];
  selectedLane: string | null;
  onSelectLane: (lane: string | null) => void;
}) {
  return (
    <div className="space-y-2">
      {props.lanes.map((lane) => {
        const isSelected = props.selectedLane === lane.lane;
        return (
          <button
            key={lane.lane}
            type="button"
            onClick={() => props.onSelectLane(isSelected ? null : lane.lane)}
            className={cn(
              "flex w-full items-center justify-between gap-3 rounded-[16px] border px-3.5 py-3 text-left transition-all",
              laneRowTone(lane.status, isSelected),
            )}
          >
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <span
                  className={cn(
                    "h-2 w-2 rounded-full",
                    laneDotTone(lane.status),
                  )}
                />
                <span className="text-[13px] font-medium tracking-tight text-foreground/90">
                  {humanizeLane(lane.lane)}
                </span>
              </div>
              <div className="mt-1 pl-4 text-[11.5px] text-muted-foreground">
                {lane.entry_count} entries selected
              </div>
            </div>
            <div className="flex items-center gap-2">
              {isSelected && (
                <span className="text-[10px] uppercase tracking-[0.12em] text-muted-foreground/75">
                  Focus
                </span>
              )}
              <span
                className={cn(
                  "rounded-full border px-2.5 py-1 text-[10px] font-medium uppercase tracking-[0.12em]",
                  laneBadgeTone(lane.status),
                )}
              >
                {lane.status}
              </span>
            </div>
          </button>
        );
      })}
    </div>
  );
}

function renderBody(
  summary: PromptDiagnosticsSummary,
  compact: boolean,
  selectedLane: string | null,
  onSelectLane: (lane: string | null) => void,
) {
  const filteredActivated =
    selectedLane === null
      ? summary.activated_entries
      : summary.activated_entries.filter(
          (entry) => entry.lane === selectedLane,
        );
  const filteredSuppressed =
    selectedLane === null
      ? summary.suppressed_entries
      : summary.suppressed_entries.filter(
          (entry) => entry.lane === selectedLane,
        );
  const filteredReasons =
    selectedLane === null
      ? summary.activation_reasons
      : summary.activation_reasons.filter(
          (reason) => reason.lane === selectedLane,
        );

  return (
    <div className={cn("space-y-4", compact && "space-y-3.5")}>
      <div
        className={cn(
          "grid gap-2.5",
          compact ? "grid-cols-2" : "grid-cols-2 xl:grid-cols-4",
        )}
      >
        <MetricTile
          label="trace"
          value={shortenTraceId(summary.trace_id)}
          compact={compact}
        />
        <MetricTile
          label="blocks"
          value={summary.block_count}
          compact={compact}
        />
        <MetricTile
          label="active lanes"
          value={summary.active_lane_count}
          compact={compact}
        />
        <MetricTile
          label="reasons"
          value={summary.activation_reason_codes.length}
          compact={compact}
        />
      </div>

      <SummarySentence summary={summary} selectedLane={selectedLane} />

      <div
        className={cn(
          "grid gap-4",
          compact
            ? "grid-cols-1"
            : "grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)]",
        )}
      >
        <section className="rounded-[22px] border border-black/[0.06] bg-white px-4 py-4 shadow-[0_1px_8px_rgba(0,0,0,0.04)]">
          <SectionTitle icon={Layers3} label="Lane Summary" />
          <LaneTable
            lanes={summary.lane_summaries}
            selectedLane={selectedLane}
            onSelectLane={onSelectLane}
          />
        </section>

        <div className="space-y-4">
          <section className="rounded-[22px] border border-black/[0.06] bg-white px-4 py-4 shadow-[0_1px_8px_rgba(0,0,0,0.04)]">
            <SectionTitle icon={Sparkles} label="Activation Reasons" />
            {filteredReasons.length === 0 ? (
              <div className="text-[12px] text-muted-foreground">
                本轮没有激活原因记录。
              </div>
            ) : (
              <div className="space-y-2">
                {filteredReasons.map((reason) => (
                  <div
                    key={`${reason.entry_id}-${reason.reason_code}`}
                    className="rounded-2xl border border-black/[0.06] bg-black/[0.02] px-3 py-2.5"
                  >
                    <div className="flex flex-wrap items-center gap-2">
                      <span
                        className={cn(
                          "rounded-full border px-2.5 py-1 text-[11px] font-medium",
                          reasonTone(reason.reason_code),
                        )}
                      >
                        {reason.reason_code}
                      </span>
                      <span className="text-[10.5px] text-muted-foreground">
                        {reason.entry_id}
                      </span>
                    </div>
                    <div className="mt-1.5 text-[11.5px] leading-5 text-foreground/78">
                      {reason.detail}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </section>

          <section className="rounded-[22px] border border-black/[0.06] bg-white px-4 py-4 shadow-[0_1px_8px_rgba(0,0,0,0.04)]">
            <SectionTitle icon={Workflow} label="Entries" />
            <div className="grid gap-3 xl:grid-cols-2">
              <EntryList
                title="Activated"
                count={filteredActivated.length}
                entries={filteredActivated.map((entry) => ({
                  id: entry.entry_id,
                  meta: `lane: ${entry.lane}`,
                  badge: entry.source,
                }))}
                tone="active"
              />
              <EntryList
                title="Suppressed"
                count={filteredSuppressed.length}
                entries={filteredSuppressed.map((entry) => ({
                  id: entry.entry_id,
                  meta: `lane: ${entry.lane}`,
                  badge: entry.reason_code,
                }))}
                tone="suppressed"
              />
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}

export function PromptDiagnosticsPanel({
  snapshot,
  title = "Prompt Diagnostics",
  description = "只读展示当前 prompt control plane 的 lane / entry / reason 摘要，不暴露原始 prompt 文本。",
  collapsible = false,
  defaultExpanded = true,
  compact = false,
  className,
}: PromptDiagnosticsPanelProps) {
  const [expanded, setExpanded] = useState(defaultExpanded);
  const [selectedLane, setSelectedLane] = useState<string | null>(null);

  const headerMeta = useMemo(() => {
    if (!snapshot) return null;
    return {
      updatedAt: formatTimestamp(snapshot.updatedAt),
      sessionId: snapshot.sessionId,
      traceId: shortenTraceId(snapshot.summary.trace_id),
    };
  }, [snapshot]);

  return (
    <section
      className={cn(
        "overflow-hidden rounded-[26px] border border-black/[0.07] bg-white",
        className,
      )}
      style={{
        boxShadow: "0 1px 10px rgba(0,0,0,0.05), 0 0 0 0.5px rgba(0,0,0,0.03)",
      }}
    >
      <div className={cn("relative", compact ? "px-4 py-4" : "px-5 py-5")}>
        <div className="pointer-events-none absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-black/8 to-transparent" />
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0">
            <div className="mb-2 inline-flex items-center gap-2 rounded-full border border-black/[0.06] bg-black/[0.02] px-2.5 py-1 text-[10px] font-medium uppercase tracking-[0.14em] text-muted-foreground/78">
              <ListTree className="h-3.5 w-3.5 text-jade" aria-hidden />
              Prompt Control Plane
            </div>
            <div className="text-[17px] font-semibold tracking-[-0.03em] text-foreground/92">
              {title}
            </div>
            <p
              className={cn(
                "mt-1 max-w-3xl text-muted-foreground",
                compact ? "text-[12px] leading-5" : "text-[12.5px] leading-6",
              )}
            >
              {description}
            </p>

            {headerMeta && (
              <div className="mt-3 flex flex-wrap gap-2">
                {[
                  { label: "Session", value: headerMeta.sessionId },
                  { label: "Trace", value: headerMeta.traceId },
                  { label: "Updated", value: headerMeta.updatedAt },
                ].map((item) => (
                  <div
                    key={item.label}
                    className="inline-flex items-center gap-2 rounded-full border border-black/[0.06] bg-black/[0.018] px-2.5 py-1 text-[11px] text-foreground/72"
                  >
                    <span className="text-muted-foreground/75">
                      {item.label}
                    </span>
                    <code className="font-mono text-[10.5px] text-foreground/78">
                      {item.value}
                    </code>
                  </div>
                ))}
              </div>
            )}
          </div>

          {collapsible && (
            <button
              type="button"
              onClick={() => setExpanded((prev) => !prev)}
              className="inline-flex items-center gap-1.5 rounded-full border border-black/[0.07] bg-white px-3 py-1.5 text-[11px] font-medium text-foreground/78 transition-colors hover:bg-black/[0.03]"
            >
              {expanded ? (
                <ChevronDown className="h-3.5 w-3.5" />
              ) : (
                <ChevronRight className="h-3.5 w-3.5" />
              )}
              {expanded ? "收起" : "展开"}
            </button>
          )}
        </div>
      </div>

      {(!collapsible || expanded) && (
        <div
          className={cn(
            "border-t border-black/[0.06]",
            compact ? "px-4 py-4" : "px-5 py-5",
          )}
        >
          {!snapshot ? (
            <EmptyState compact={compact} />
          ) : (
            renderBody(snapshot.summary, compact, selectedLane, setSelectedLane)
          )}
        </div>
      )}
    </section>
  );
}
