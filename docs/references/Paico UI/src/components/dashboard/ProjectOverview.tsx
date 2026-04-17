import { ArrowRight, MapPin } from "lucide-react";

type Project = {
  id: string;
  name: string;
  client: string;
  location: string;
  phase: string;
  status: "active" | "review" | "hold" | "complete";
  progress: number;
  budget: string;
  dueDate: string;
  image: string;
  team: string[];
};

const PROJECTS: Project[] = [
  {
    id: "1",
    name: "Kensington Penthouse",
    client: "A. Harrington",
    location: "London, UK",
    phase: "Concept Development",
    status: "active",
    progress: 68,
    budget: "$420K",
    dueDate: "Aug 2025",
    image: "https://images.unsplash.com/photo-1600607687939-ce8a6c25118c?w=400&h=240&fit=crop",
    team: ["SC", "JL", "MR"],
  },
  {
    id: "2",
    name: "Maison Rivière",
    client: "F. Beaumont",
    location: "Paris, FR",
    phase: "Proposal Review",
    status: "review",
    progress: 42,
    budget: "$890K",
    dueDate: "Nov 2025",
    image: "https://images.unsplash.com/photo-1586023492125-27b2c045efd7?w=400&h=240&fit=crop",
    team: ["SC", "TR"],
  },
  {
    id: "3",
    name: "The Aldgate Loft",
    client: "N. Okafor",
    location: "London, UK",
    phase: "Construction Doc.",
    status: "active",
    progress: 85,
    budget: "$210K",
    dueDate: "Jul 2025",
    image: "https://images.unsplash.com/photo-1618221195710-dd6b41faaea6?w=400&h=240&fit=crop",
    team: ["JL", "MR", "TD"],
  },
  {
    id: "4",
    name: "Villa Almería",
    client: "C. Morales",
    location: "Almería, ES",
    phase: "On Hold",
    status: "hold",
    progress: 30,
    budget: "$1.2M",
    dueDate: "Jan 2026",
    image: "https://images.unsplash.com/photo-1512917774080-9991f1c4c750?w=400&h=240&fit=crop",
    team: ["SC"],
  },
];

const STATUS_MAP: Record<string, string> = {
  active: "status-active",
  review: "status-review",
  hold: "status-hold",
  complete: "status-complete",
};

const STATUS_LABEL: Record<string, string> = {
  active: "Active",
  review: "In Review",
  hold: "On Hold",
  complete: "Complete",
};

const PROGRESS_COLOR: Record<string, string> = {
  active: "bg-olive",
  review: "bg-gold",
  hold: "bg-terracotta",
  complete: "bg-status-complete",
};

interface ProjectOverviewProps {
  onSelectProject?: (id: string) => void;
}

export default function ProjectOverview({
  onSelectProject = () => {},
}: ProjectOverviewProps) {
  return (
    <div data-cmp="ProjectOverview" className="bg-surface border border-border rounded-xl shadow-custom overflow-hidden">
      <div className="flex items-center justify-between px-5 py-3.5 border-b border-border">
        <div>
          <h2 className="font-serif text-[15px] font-semibold text-foreground">
            Active Projects
          </h2>
          <p className="text-muted-foreground text-[10px] font-sans mt-0.5">
            24 total · 4 requiring attention
          </p>
        </div>
        <button className="flex items-center gap-1 text-muted-foreground hover:text-foreground text-[11px] font-sans transition-all">
          View All <ArrowRight size={12} />
        </button>
      </div>

      <div className="divide-y divide-border">
        {PROJECTS.map((p) => (
          <button
            key={p.id}
            onClick={() => onSelectProject(p.id)}
            className="w-full flex items-center gap-3 px-5 py-3 hover:bg-muted/50 transition-all text-left group"
          >
            {/* Thumbnail */}
            <div
              className="w-14 h-10 rounded-md image-card shrink-0 border border-border"
              style={{ backgroundImage: `url(${p.image})` }}
            />

            {/* Info */}
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2 mb-0.5">
                <span className="font-sans text-[13px] font-semibold text-foreground truncate">
                  {p.name}
                </span>
                <span className={`tag-pill ${STATUS_MAP[p.status]}`}>
                  {STATUS_LABEL[p.status]}
                </span>
              </div>
              <div className="flex items-center gap-2 text-muted-foreground text-[11px] font-sans">
                <span className="truncate">{p.client}</span>
                <span>·</span>
                <MapPin size={9} />
                <span className="truncate">{p.location}</span>
                <span>·</span>
                <span className="truncate">{p.phase}</span>
              </div>
            </div>

            {/* Progress */}
            <div className="w-28 shrink-0">
              <div className="flex items-center justify-between text-[10px] font-sans text-muted-foreground mb-1">
                <span>Progress</span>
                <span className="font-medium text-foreground">{p.progress}%</span>
              </div>
              <div className="h-1 bg-muted rounded-full overflow-hidden">
                <div
                  className={`h-full rounded-full ${PROGRESS_COLOR[p.status]}`}
                  style={{ width: `${p.progress}%` }}
                />
              </div>
            </div>

            {/* Budget */}
            <div className="w-16 text-right shrink-0">
              <p className="text-[12px] font-semibold font-sans text-foreground">
                {p.budget}
              </p>
              <p className="text-muted-foreground text-[10px] font-sans">
                {p.dueDate}
              </p>
            </div>

            {/* Team */}
            <div className="flex -space-x-1.5 shrink-0">
              {p.team.slice(0, 3).map((m) => (
                <div
                  key={m}
                  className="w-6 h-6 rounded-full bg-warm-beige-dark border border-border flex items-center justify-center text-[9px] font-semibold text-charcoal"
                >
                  {m}
                </div>
              ))}
            </div>

            <ArrowRight
              size={13}
              className="text-muted-foreground opacity-0 group-hover:opacity-100 transition-all shrink-0"
            />
          </button>
        ))}
      </div>
    </div>
  );
}
