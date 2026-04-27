import { Activity, AlertTriangle, CheckCircle2, Clock3, Lock, Route, Wrench } from "lucide-react";
import { useMemo, type ReactNode } from "react";

import { useRuntimeProjection } from "@/runtime-projection";
import { cn } from "@/lib/utils";
import type { FinalRunReport } from "@/transport/contracts";

interface RunInspectorPanelProps {
  sessionId: string | null;
  className?: string;
}

function label(value: string | undefined): string {
  return value ? value.replace(/_/g, " ") : "pending";
}

export function RunInspectorPanel({
  sessionId,
  className,
}: RunInspectorPanelProps) {
  const snapshot = useRuntimeProjection();
  const state = useMemo(() => {
    if (!sessionId) return null;
    const runs = Object.values(snapshot.runs)
      .filter((candidate) => candidate.sessionId === sessionId)
      .sort((left, right) => right.lastUpdatedAt - left.lastUpdatedAt);
    const run = runs[0] ?? null;
    return run ? { run, approval: snapshot.approvals[sessionId] } : null;
  }, [sessionId, snapshot]);

  if (!state) return null;

  const { run, approval } = state;
  const tools = Object.values(run.toolCalls);
  const completedTools = tools.filter((tool) => tool.status === "completed").length;
  const failedTools = tools.filter(
    (tool) =>
      tool.status === "failed" ||
      tool.status === "error" ||
      tool.status === "blocked" ||
      tool.status === "cancelled",
  ).length;
  const report = run.finalRunReport;
  const approvalTool = approval
    ? tools.find((tool) => tool.toolName === approval.toolName && tool.status !== "completed")
      ?? tools.find((tool) => tool.toolName === approval.toolName)
    : undefined;
  const approvalRisk =
    report?.failedItems[0] ??
    report?.terminalStatus ??
    approval?.message;
  const loadedSkills =
    report?.loadedSkills.length
      ? report.loadedSkills
      : run.skillResolution?.loadedSkillNames ?? [];
  const blockedSkills =
    report?.blockedSkills.length
      ? report.blockedSkills
      : run.skillResolution?.blockedSkillNames ?? [];
  const candidateSkills =
    run.skillResolution?.candidates
      .filter((skill) => !loadedSkills.includes(skill.name) && !blockedSkills.includes(skill.name))
      .map((skill) => skill.name) ?? [];
  const outcomeMeta = report ? reportMeta(report) : null;

  return (
    <aside
      className={cn(
        "pointer-events-auto absolute right-4 top-4 z-30 hidden w-[320px] rounded-lg border border-border/70 bg-background/92 p-3 shadow-[0_18px_50px_rgba(15,23,42,0.14)] backdrop-blur-xl xl:block",
        className,
      )}
      aria-label="Run inspector"
    >
      <div className="flex items-center justify-between gap-2">
        <div className="inline-flex min-w-0 items-center gap-2">
          <Activity className="h-3.5 w-3.5 text-jade" />
          <span className="truncate text-[12.5px] font-semibold text-foreground">
            Run Inspector
          </span>
        </div>
        <span className="rounded-full bg-muted px-2 py-0.5 text-[10.5px] font-medium text-muted-foreground">
          {label(run.status)}
        </span>
      </div>

      <div className="mt-3 grid grid-cols-2 gap-2 text-[11.5px]">
        <div className="rounded-md bg-muted/60 px-2.5 py-2">
          <div className="inline-flex items-center gap-1.5 text-muted-foreground">
            <Route className="h-3 w-3" />
            Loop
          </div>
          <div className="mt-1 truncate font-medium text-foreground">
            {label(run.workLoop?.loopKind ?? report?.loopKind)}
          </div>
        </div>
        <div className="rounded-md bg-muted/60 px-2.5 py-2">
          <div className="inline-flex items-center gap-1.5 text-muted-foreground">
            <Wrench className="h-3 w-3" />
            Tools
          </div>
          <div className="mt-1 font-medium text-foreground">
            {completedTools} ok · {failedTools} failed
          </div>
        </div>
        <div className="rounded-md bg-muted/60 px-2.5 py-2">
          <div className="inline-flex items-center gap-1.5 text-muted-foreground">
            {outcomeMeta?.icon ?? <Clock3 className="h-3 w-3" />}
            Outcome
          </div>
          <div className="mt-1 truncate font-medium text-foreground">
            {label(report?.outcome ?? run.taskOutcome)}
          </div>
        </div>
        <div className="rounded-md bg-muted/60 px-2.5 py-2">
          <div className="text-muted-foreground">Terminal</div>
          <div className="mt-1 truncate font-medium text-foreground">
            {label(report?.terminalStatus ?? run.degradedReason)}
          </div>
        </div>
      </div>

      {approval ? (
        <div className="mt-2 rounded-md border border-amber-500/35 bg-amber-500/10 px-2.5 py-2">
          <div className="inline-flex items-center gap-1.5 text-[11px] font-semibold text-amber-500">
            <Lock className="h-3 w-3" />
            Approval pending
          </div>
          <div className="mt-1 grid gap-1 text-[11px] leading-4 text-foreground/74">
            <div className="truncate">Operation: {approval.toolName}</div>
            <div className="truncate">Mode: {approval.currentMode} → {approval.permissionMode}</div>
            <div className="break-words text-muted-foreground">State: pending user decision</div>
            {approvalRisk ? (
              <div className="break-words text-muted-foreground">Risk: {approvalRisk}</div>
            ) : null}
            {approvalTool?.effectiveWorkdir ? (
              <div className="truncate text-muted-foreground">
                Workdir: {approvalTool.effectiveWorkdir}
              </div>
            ) : null}
            {approvalTool?.toolArgs ? (
              <div className="break-words rounded-md bg-background/45 px-2 py-1 font-mono text-[10.5px] leading-4 text-muted-foreground">
                Params: {formatToolArgs(approvalTool.toolArgs)}
              </div>
            ) : null}
            <div className="break-words text-muted-foreground">{approval.message}</div>
          </div>
        </div>
      ) : null}

      <SkillChips title="Loaded skills" skills={loadedSkills} tone="loaded" />
      <SkillChips title="Blocked skills" skills={blockedSkills} tone="blocked" />
      <SkillChips title="Candidate skills" skills={candidateSkills} tone="candidate" />
      <ReportSection title="Skill warnings" items={report?.skillWarnings ?? run.skillResolution?.loadWarnings ?? []} tone="warning" />

      {report ? (
        <div className={cn("mt-2 rounded-md border px-2.5 py-2", outcomeMeta?.containerClass)}>
          <div className="inline-flex min-w-0 items-center gap-1.5 text-[11px] font-semibold text-foreground">
            {outcomeMeta?.icon}
            <span className="truncate">{outcomeMeta?.title ?? label(report.outcome)}</span>
          </div>
          <div className="mt-1 grid grid-cols-2 gap-1 text-[10.5px] text-muted-foreground">
            <span className="truncate">Task: {label(report.taskOutcome)}</span>
            <span className="truncate">Resume: {report.resumeAvailable ? "yes" : "no"}</span>
          </div>
          <ReportSection title="Completed" items={report.completedItems} />
          <ReportSection title="Failed / blocked" items={report.failedItems} tone="danger" />
          <ReportSection title="Next steps" items={report.userNextSteps} tone="next" />
        </div>
      ) : null}
    </aside>
  );
}

function SkillChips({
  title,
  skills,
  tone,
}: {
  title: string;
  skills: string[];
  tone: "loaded" | "blocked" | "candidate";
}) {
  const visible = skills.filter(Boolean);
  if (visible.length === 0) return null;
  return (
    <div className="mt-2 rounded-md bg-muted/40 px-2.5 py-2">
      <div className="text-[10.5px] font-semibold text-muted-foreground">{title}</div>
      <div className="mt-1 flex flex-wrap gap-1">
        {visible.slice(0, 6).map((skill) => (
          <span
            key={`${title}-${skill}`}
            className={cn(
              "max-w-full truncate rounded-full px-2 py-0.5 text-[10.5px] font-medium",
              tone === "loaded" && "bg-jade/10 text-jade",
              tone === "blocked" && "bg-rose-500/10 text-rose-500",
              tone === "candidate" && "bg-muted text-muted-foreground",
            )}
          >
            {skill}
          </span>
        ))}
      </div>
    </div>
  );
}

function ReportSection({
  title,
  items,
  tone = "neutral",
}: {
  title: string;
  items: string[];
  tone?: "neutral" | "danger" | "warning" | "next";
}) {
  const visible = items.filter((item) => item.trim().length > 0);
  if (visible.length === 0) return null;
  return (
    <div className="mt-2">
      <div
        className={cn(
          "text-[10.5px] font-semibold",
          tone === "danger" && "text-rose-500",
          tone === "warning" && "text-amber-500",
          tone === "next" && "text-sky-500",
          tone === "neutral" && "text-muted-foreground",
        )}
      >
        {title}
      </div>
      <ul className="mt-1 space-y-1">
        {visible.slice(0, 3).map((item, index) => (
          <li key={`${title}-${index}`} className="break-words text-[11px] leading-4 text-foreground/74">
            {item}
          </li>
        ))}
      </ul>
    </div>
  );
}

function reportMeta(report: FinalRunReport): {
  title: string;
  icon: ReactNode;
  containerClass: string;
} {
  if (report.outcome === "completed") {
    return {
      title: "Done",
      icon: <CheckCircle2 className="h-3 w-3 text-jade" />,
      containerClass: "border-jade/35 bg-jade/8",
    };
  }
  if (report.outcome === "needs_approval") {
    return {
      title: "Waiting for approval",
      icon: <Lock className="h-3 w-3 text-amber-500" />,
      containerClass: "border-amber-500/35 bg-amber-500/10",
    };
  }
  if (report.outcome === "exhausted_with_summary") {
    return {
      title: "Stopped after limits",
      icon: <Clock3 className="h-3 w-3 text-sky-500" />,
      containerClass: "border-sky-500/35 bg-sky-500/10",
    };
  }
  return {
    title: "Could not finish",
    icon: <AlertTriangle className="h-3 w-3 text-rose-500" />,
    containerClass: "border-rose-500/35 bg-rose-500/10",
  };
}

function formatToolArgs(args: Record<string, unknown>): string {
  const text = JSON.stringify(args);
  if (!text) return "{}";
  return text.length > 220 ? `${text.slice(0, 220)}...` : text;
}
