import { useState } from "react";
import { ChevronLeft, ChevronRight, Circle, CheckCircle } from "lucide-react";

type Phase = {
  id: string;
  name: string;
  start: number; // week offset from Jan 1
  duration: number; // weeks
  status: "complete" | "active" | "pending";
  team: string;
};

type GanttProject = {
  id: string;
  name: string;
  client: string;
  color: string;
  phases: Phase[];
};

const PROJECTS: GanttProject[] = [
  {
    id: "1",
    name: "Kensington Penthouse",
    client: "A. Harrington",
    color: "bg-olive",
    phases: [
      { id: "p1", name: "Brief & Survey", start: 0, duration: 2, status: "complete", team: "SC" },
      { id: "p2", name: "Concept Design", start: 2, duration: 5, status: "complete", team: "SC + JL" },
      { id: "p3", name: "Proposal", start: 6, duration: 3, status: "active", team: "SC" },
      { id: "p4", name: "Technical Design", start: 9, duration: 6, status: "pending", team: "JL + MR" },
      { id: "p5", name: "Procurement", start: 14, duration: 8, status: "pending", team: "MR" },
      { id: "p6", name: "Installation", start: 22, duration: 6, status: "pending", team: "All" },
    ],
  },
  {
    id: "2",
    name: "Maison Rivière",
    client: "F. Beaumont",
    color: "bg-gold",
    phases: [
      { id: "p1", name: "Brief", start: 8, duration: 2, status: "complete", team: "SC" },
      { id: "p2", name: "Concept", start: 10, duration: 4, status: "active", team: "SC + TR" },
      { id: "p3", name: "Design Dev.", start: 14, duration: 6, status: "pending", team: "SC + TR" },
      { id: "p4", name: "Documentation", start: 20, duration: 5, status: "pending", team: "TR" },
      { id: "p5", name: "Handover", start: 25, duration: 2, status: "pending", team: "All" },
    ],
  },
  {
    id: "3",
    name: "The Aldgate Loft",
    client: "N. Okafor",
    color: "bg-terracotta",
    phases: [
      { id: "p1", name: "Survey", start: 0, duration: 1, status: "complete", team: "JL" },
      { id: "p2", name: "Concept", start: 1, duration: 3, status: "complete", team: "JL + TD" },
      { id: "p3", name: "Technical", start: 4, duration: 5, status: "complete", team: "JL + MR" },
      { id: "p4", name: "Site Works", start: 9, duration: 10, status: "active", team: "TD" },
      { id: "p5", name: "Snagging", start: 19, duration: 2, status: "pending", team: "JL" },
    ],
  },
  {
    id: "4",
    name: "Villa Almería",
    client: "C. Morales",
    color: "bg-status-complete",
    phases: [
      { id: "p1", name: "Brief", start: 4, duration: 2, status: "complete", team: "SC" },
      { id: "p2", name: "Concept", start: 6, duration: 6, status: "active", team: "SC" },
      { id: "p3", name: "Design Dev.", start: 12, duration: 8, status: "pending", team: "SC + TR" },
      { id: "p4", name: "Documentation", start: 20, duration: 6, status: "pending", team: "TR" },
      { id: "p5", name: "Procurement", start: 26, duration: 10, status: "pending", team: "MR" },
      { id: "p6", name: "Install & Handover", start: 36, duration: 8, status: "pending", team: "All" },
    ],
  },
  {
    id: "5",
    name: "Cotswolds Cottage",
    client: "H. Whitmore",
    color: "bg-muted-foreground",
    phases: [
      { id: "p1", name: "Brief", start: 18, duration: 1, status: "pending", team: "TR" },
      { id: "p2", name: "Concept", start: 19, duration: 4, status: "pending", team: "TR + MR" },
      { id: "p3", name: "Design Dev.", start: 23, duration: 6, status: "pending", team: "TR" },
    ],
  },
];

// 52-week year, display 26 weeks at a time
const WEEK_LABELS = Array.from({ length: 52 }, (_, i) => {
  const m = Math.floor(i / 4.33);
  const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
  return i % Math.round(52 / 12) === 0 ? months[m] : "";
});

const MILESTONES: { week: number; label: string }[] = [
  { week: 6, label: "Q2 Review" },
  { week: 13, label: "Mid-Year" },
  { week: 24, label: "Q3 Review" },
];

export default function Timeline() {
  const [offset, setOffset] = useState(0); // scroll in weeks
  const VISIBLE = 26;

  const visibleStart = offset;
  const visibleEnd = offset + VISIBLE;
  const colWidth = 28; // px per week

  return (
    <div data-cmp="Timeline" className="flex-1 overflow-hidden flex flex-col bg-background">
      {/* Toolbar */}
      <div className="px-6 py-4 border-b border-border bg-surface flex items-center gap-4">
        <h2 className="font-serif text-[16px] font-semibold text-foreground">Project Timeline</h2>
        <span className="text-muted-foreground text-[12px] font-sans">2025 — Gantt View</span>
        <div className="flex items-center gap-1 ml-auto">
          {["Week", "Month", "Quarter"].map((v) => (
            <button
              key={v}
              className={`px-3 py-1 rounded-md text-[11px] font-sans transition-all ${
                v === "Month"
                  ? "bg-charcoal text-primary-foreground"
                  : "text-muted-foreground hover:text-foreground hover:bg-muted"
              }`}
            >
              {v}
            </button>
          ))}
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={() => setOffset(Math.max(0, offset - 4))}
            className="w-7 h-7 flex items-center justify-center rounded-md border border-border text-muted-foreground hover:bg-muted hover:text-foreground transition-all"
          >
            <ChevronLeft size={14} />
          </button>
          <button
            onClick={() => setOffset(Math.min(26, offset + 4))}
            className="w-7 h-7 flex items-center justify-center rounded-md border border-border text-muted-foreground hover:bg-muted hover:text-foreground transition-all"
          >
            <ChevronRight size={14} />
          </button>
        </div>
      </div>

      {/* Gantt area */}
      <div className="flex-1 overflow-auto scrollbar-thin">
        <div className="flex min-w-max">
          {/* Row headers */}
          <div className="w-52 shrink-0 border-r border-border sticky left-0 bg-surface z-20">
            {/* Header spacer */}
            <div className="h-10 border-b border-border px-4 flex items-center">
              <span className="text-muted-foreground text-[10px] font-sans uppercase tracking-wide">Project</span>
            </div>
            {PROJECTS.map((p) => (
              <div key={p.id}>
                {/* Project row */}
                <div className="h-10 border-b border-border px-4 flex items-center">
                  <div className="min-w-0">
                    <p className="text-[12px] font-sans font-semibold text-foreground truncate">{p.name}</p>
                    <p className="text-muted-foreground text-[10px] font-sans truncate">{p.client}</p>
                  </div>
                </div>
                {/* Phase rows */}
                {p.phases.map((ph) => (
                  <div key={ph.id} className="h-8 border-b border-border/50 px-4 flex items-center">
                    <div className="flex items-center gap-1.5">
                      {ph.status === "complete"
                        ? <CheckCircle size={9} className="text-olive shrink-0" />
                        : <Circle size={9} className="text-muted-foreground shrink-0" />
                      }
                      <span className="text-[10px] font-sans text-muted-foreground truncate">{ph.name}</span>
                    </div>
                  </div>
                ))}
              </div>
            ))}
          </div>

          {/* Gantt chart area */}
          <div className="flex-1 overflow-x-auto">
            {/* Week header */}
            <div
              className="h-10 border-b border-border flex items-end pb-1 sticky top-0 bg-surface z-10"
              style={{ width: `${VISIBLE * colWidth}px` }}
            >
              {Array.from({ length: VISIBLE }, (_, i) => {
                const week = visibleStart + i;
                const label = WEEK_LABELS[week % 52];
                const milestone = MILESTONES.find((m) => m.week === week);
                return (
                  <div
                    key={i}
                    className="shrink-0 relative flex items-end justify-center"
                    style={{ width: `${colWidth}px` }}
                  >
                    {label && (
                      <span className="text-[9px] font-sans text-muted-foreground absolute top-1 left-1">
                        {label}
                      </span>
                    )}
                    {milestone && (
                      <div className="absolute -bottom-1 left-1/2 -translate-x-1/2 w-1.5 h-1.5 rounded-full bg-gold z-10" title={milestone.label} />
                    )}
                    <span className="text-[8px] font-sans text-muted-foreground/50">W{week + 1}</span>
                  </div>
                );
              })}
            </div>

            {/* Rows */}
            {PROJECTS.map((p) => (
              <div key={p.id}>
                {/* Project summary row */}
                <div
                  className="h-10 border-b border-border bg-muted/30 relative"
                  style={{ width: `${VISIBLE * colWidth}px` }}
                >
                  {/* Today line */}
                  <div
                    className="absolute top-0 bottom-0 w-px bg-terracotta/40 z-10"
                    style={{ left: `${(23 - visibleStart) * colWidth}px` }}
                  />
                </div>
                {/* Phase rows */}
                {p.phases.map((ph) => {
                  const barStart = Math.max(0, ph.start - visibleStart);
                  const barEnd = Math.min(VISIBLE, ph.start + ph.duration - visibleStart);
                  const visible = barEnd > barStart && ph.start < visibleEnd && ph.start + ph.duration > visibleStart;

                  return (
                    <div
                      key={ph.id}
                      className="h-8 border-b border-border/50 relative"
                      style={{ width: `${VISIBLE * colWidth}px` }}
                    >
                      {/* Today line */}
                      <div
                        className="absolute top-0 bottom-0 w-px bg-terracotta/30 z-10"
                        style={{ left: `${(23 - visibleStart) * colWidth}px` }}
                      />
                      {/* Phase bar */}
                      {visible && (
                        <div
                          className={`absolute top-1.5 h-5 rounded-md flex items-center px-2 ${p.color} ${
                            ph.status === "complete" ? "opacity-60" : ph.status === "active" ? "opacity-100" : "opacity-30"
                          } transition-all cursor-pointer hover:opacity-80`}
                          style={{
                            left: `${barStart * colWidth + 2}px`,
                            width: `${(barEnd - barStart) * colWidth - 4}px`,
                          }}
                          title={`${ph.name} — ${ph.team}`}
                        >
                          <span className="text-[9px] font-sans font-semibold text-primary-foreground truncate">
                            {ph.name}
                          </span>
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            ))}
          </div>
        </div>

        {/* Legend */}
        <div className="sticky left-0 px-6 py-3 border-t border-border bg-surface flex items-center gap-6">
          {[
            { cls: "bg-olive opacity-100", label: "Active" },
            { cls: "bg-muted-foreground opacity-60", label: "Complete" },
            { cls: "bg-muted-foreground opacity-30", label: "Planned" },
          ].map((l) => (
            <div key={l.label} className="flex items-center gap-1.5">
              <div className={`w-8 h-2.5 rounded-sm ${l.cls}`} />
              <span className="text-[11px] font-sans text-muted-foreground">{l.label}</span>
            </div>
          ))}
          <div className="flex items-center gap-1.5">
            <div className="w-px h-3 bg-terracotta/40" />
            <span className="text-[11px] font-sans text-muted-foreground">Today</span>
          </div>
          <div className="flex items-center gap-1.5">
            <div className="w-2 h-2 rounded-full bg-gold" />
            <span className="text-[11px] font-sans text-muted-foreground">Milestone</span>
          </div>
        </div>
      </div>
    </div>
  );
}
