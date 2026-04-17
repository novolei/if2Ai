import { TrendingUp, TrendingDown, FolderOpen, Users, DollarSign, Clock } from "lucide-react";

type StatCard = {
  label: string;
  value: string;
  delta: string;
  deltaUp: boolean;
  icon: React.ComponentType<{ size?: number; className?: string }>;
  accent: string;
};

const STATS: StatCard[] = [
  {
    label: "Active Projects",
    value: "24",
    delta: "+3 this month",
    deltaUp: true,
    icon: FolderOpen,
    accent: "bg-olive-light text-olive",
  },
  {
    label: "Active Clients",
    value: "18",
    delta: "+2 this quarter",
    deltaUp: true,
    icon: Users,
    accent: "bg-terracotta-light text-terracotta",
  },
  {
    label: "Revenue YTD",
    value: "$2.4M",
    delta: "+14% vs last year",
    deltaUp: true,
    icon: DollarSign,
    accent: "bg-gold-light text-gold",
  },
  {
    label: "Pending Reviews",
    value: "7",
    delta: "−2 from last week",
    deltaUp: false,
    icon: Clock,
    accent: "bg-secondary text-secondary-foreground",
  },
];

export default function StatCards() {
  return (
    <div data-cmp="StatCards" className="flex gap-3">
      {STATS.map((s) => {
        const Icon = s.icon;
        return (
          <div
            key={s.label}
            className="flex-1 bg-surface border border-border rounded-xl p-4 shadow-custom"
          >
            <div className="flex items-start justify-between mb-3">
              <div className={`w-8 h-8 rounded-lg flex items-center justify-center ${s.accent}`}>
                <Icon size={15} />
              </div>
              <div className={`flex items-center gap-1 text-[10px] font-sans font-medium ${s.deltaUp ? "text-olive" : "text-terracotta"}`}>
                {s.deltaUp ? <TrendingUp size={11} /> : <TrendingDown size={11} />}
                {s.delta}
              </div>
            </div>
            <p className="font-serif text-[26px] font-semibold text-foreground leading-none">
              {s.value}
            </p>
            <p className="text-muted-foreground text-[11px] font-sans mt-1">
              {s.label}
            </p>
          </div>
        );
      })}
    </div>
  );
}
