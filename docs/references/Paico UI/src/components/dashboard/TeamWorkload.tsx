type Member = {
  initials: string;
  name: string;
  role: string;
  projects: number;
  load: number;
  status: "available" | "busy" | "away";
};

const TEAM: Member[] = [
  { initials: "SC", name: "Sara Chen", role: "Lead Designer", projects: 5, load: 82, status: "busy" },
  { initials: "JL", name: "James Lee", role: "Interior Arch.", projects: 4, load: 65, status: "available" },
  { initials: "TR", name: "Tara Roy", role: "Jr. Designer", projects: 3, load: 50, status: "available" },
  { initials: "MR", name: "Marcus R.", role: "3D Visualist", projects: 4, load: 90, status: "busy" },
  { initials: "TD", name: "Thea D.", role: "Project Mgr.", projects: 6, load: 75, status: "available" },
];

const STATUS_DOT: Record<string, string> = {
  available: "bg-olive",
  busy: "bg-terracotta",
  away: "bg-gold",
};

const LOAD_COLOR = (load: number) => {
  if (load >= 85) return "bg-terracotta";
  if (load >= 70) return "bg-gold";
  return "bg-olive";
};

export default function TeamWorkload() {
  return (
    <div data-cmp="TeamWorkload" className="bg-surface border border-border rounded-xl shadow-custom">
      <div className="px-4 py-3.5 border-b border-border">
        <h2 className="font-serif text-[15px] font-semibold text-foreground">
          Team Workload
        </h2>
        <p className="text-muted-foreground text-[10px] font-sans mt-0.5">
          Current sprint capacity
        </p>
      </div>
      <div className="divide-y divide-border">
        {TEAM.map((m) => (
          <div key={m.initials} className="flex items-center gap-3 px-4 py-2.5">
            <div className="relative shrink-0">
              <div className="w-7 h-7 rounded-full bg-warm-beige-dark flex items-center justify-center text-[10px] font-semibold text-charcoal">
                {m.initials}
              </div>
              <span
                className={`absolute bottom-0 right-0 w-2 h-2 rounded-full border border-surface ${STATUS_DOT[m.status]}`}
              />
            </div>
            <div className="flex-1 min-w-0">
              <div className="flex items-center justify-between mb-1">
                <p className="text-[12px] font-sans font-semibold text-foreground truncate">
                  {m.name}
                </p>
                <span className="text-muted-foreground text-[10px] font-sans ml-2 shrink-0">
                  {m.projects} proj
                </span>
              </div>
              <div className="h-1 bg-muted rounded-full overflow-hidden">
                <div
                  className={`h-full rounded-full ${LOAD_COLOR(m.load)}`}
                  style={{ width: `${m.load}%` }}
                />
              </div>
            </div>
            <span className={`text-[11px] font-sans font-semibold shrink-0 w-8 text-right ${
              m.load >= 85 ? "text-terracotta" : m.load >= 70 ? "text-gold" : "text-olive"
            }`}>
              {m.load}%
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
