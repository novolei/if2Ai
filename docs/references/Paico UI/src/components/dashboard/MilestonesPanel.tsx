import { Flag, CheckCircle2, Circle } from "lucide-react";

type Milestone = {
  id: string;
  project: string;
  label: string;
  date: string;
  done: boolean;
  critical: boolean;
};

const MILESTONES: Milestone[] = [
  { id: "1", project: "Aldgate Loft", label: "Client Presentation", date: "Jun 17", done: false, critical: true },
  { id: "2", project: "Kensington", label: "Concept Sign-off", date: "Jun 22", done: false, critical: false },
  { id: "3", project: "Maison Rivière", label: "Proposal Submission", date: "Jul 5", done: false, critical: false },
  { id: "4", project: "Villa Almería", label: "Mood Board Draft", date: "Jul 12", done: false, critical: false },
  { id: "5", project: "Aldgate Loft", label: "Contractor Brief", date: "Jun 10", done: true, critical: false },
  { id: "6", project: "Kensington", label: "Site Survey", date: "Jun 2", done: true, critical: false },
];

export default function MilestonesPanel() {
  return (
    <div data-cmp="MilestonesPanel" className="bg-surface border border-border rounded-xl shadow-custom">
      <div className="px-4 py-3.5 border-b border-border flex items-center gap-2">
        <Flag size={13} className="text-terracotta" />
        <h2 className="font-serif text-[15px] font-semibold text-foreground">
          Milestones
        </h2>
      </div>
      <div className="divide-y divide-border">
        {MILESTONES.map((m) => (
          <div
            key={m.id}
            className={`flex items-center gap-3 px-4 py-2.5 ${m.done ? "opacity-50" : ""}`}
          >
            {m.done ? (
              <CheckCircle2 size={14} className="text-olive shrink-0" />
            ) : (
              <Circle size={14} className={`shrink-0 ${m.critical ? "text-terracotta" : "text-muted-foreground"}`} />
            )}
            <div className="flex-1 min-w-0">
              <p className={`text-[12px] font-sans font-medium truncate ${m.done ? "line-through" : "text-foreground"}`}>
                {m.label}
              </p>
              <p className="text-muted-foreground text-[10px] font-sans truncate">
                {m.project}
              </p>
            </div>
            <span className={`text-[10px] font-sans shrink-0 font-medium ${m.critical && !m.done ? "text-terracotta" : "text-muted-foreground"}`}>
              {m.date}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
