import { MessageSquare, Upload, CheckCircle, Clock, Edit3, Star } from "lucide-react";
import type { LucideIcon } from "lucide-react";

type Activity = {
  id: string;
  type: "comment" | "upload" | "approved" | "deadline" | "revision" | "milestone";
  user: string;
  initials: string;
  action: string;
  target: string;
  time: string;
};

const ICON_MAP: Record<string, LucideIcon> = {
  comment: MessageSquare,
  upload: Upload,
  approved: CheckCircle,
  deadline: Clock,
  revision: Edit3,
  milestone: Star,
};

const ICON_COLOR: Record<string, string> = {
  comment: "bg-secondary text-secondary-foreground",
  upload: "bg-olive-light text-olive",
  approved: "bg-olive-light text-olive",
  deadline: "bg-gold-light text-gold",
  revision: "bg-terracotta-light text-terracotta",
  milestone: "bg-gold-light text-gold",
};

const ACTIVITY: Activity[] = [
  { id: "1", type: "approved", user: "Sara Chen", initials: "SC", action: "approved concept board for", target: "Kensington Penthouse", time: "2m ago" },
  { id: "2", type: "upload", user: "James Lee", initials: "JL", action: "uploaded 12 material samples to", target: "Material Library", time: "18m ago" },
  { id: "3", type: "comment", user: "Tara Roy", initials: "TR", action: "left a comment on proposal", target: "Maison Rivière — Sec.3", time: "1h ago" },
  { id: "4", type: "milestone", user: "System", initials: "SY", action: "milestone reached:", target: "Aldgate Loft — 85% complete", time: "2h ago" },
  { id: "5", type: "revision", user: "Marcus R.", initials: "MR", action: "requested revision on", target: "Villa Almería — Floor Plan", time: "3h ago" },
  { id: "6", type: "deadline", user: "System", initials: "SY", action: "deadline in 5 days:", target: "Aldgate Loft — Client Presentation", time: "5h ago" },
];

export default function RecentActivity() {
  return (
    <div data-cmp="RecentActivity" className="bg-surface border border-border rounded-xl shadow-custom">
      <div className="px-5 py-3.5 border-b border-border">
        <h2 className="font-serif text-[15px] font-semibold text-foreground">
          Recent Activity
        </h2>
        <p className="text-muted-foreground text-[10px] font-sans mt-0.5">
          Studio-wide updates
        </p>
      </div>
      <div className="divide-y divide-border">
        {ACTIVITY.map((a) => {
          const Icon = ICON_MAP[a.type];
          return (
            <div key={a.id} className="flex items-start gap-3 px-5 py-2.5 hover:bg-muted/30 transition-all">
              <div className={`w-6 h-6 rounded-full shrink-0 flex items-center justify-center mt-0.5 ${ICON_COLOR[a.type]}`}>
                <Icon size={10} />
              </div>
              <div className="flex-1 min-w-0">
                <p className="text-[12px] font-sans text-foreground leading-snug">
                  <span className="font-semibold">{a.user}</span>{" "}
                  <span className="text-muted-foreground">{a.action}</span>{" "}
                  <span className="font-medium text-charcoal">{a.target}</span>
                </p>
              </div>
              <span className="text-muted-foreground text-[10px] font-sans shrink-0 mt-0.5">
                {a.time}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
