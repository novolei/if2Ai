import { useState } from "react";
import {
  Plus, Search, Filter, MapPin, Clock, DollarSign,
  ArrowRight, Users, CalendarDays, X
} from "lucide-react";

type Project = {
  id: string;
  name: string;
  client: string;
  location: string;
  phase: string;
  status: "active" | "review" | "hold" | "complete";
  progress: number;
  budget: string;
  spent: number;
  startDate: string;
  endDate: string;
  image: string;
  team: { initials: string; name: string }[];
  tags: string[];
  description: string;
  rooms: number;
  sqft: number;
};

const PROJECTS: Project[] = [
  {
    id: "1",
    name: "Kensington Penthouse",
    client: "Alexandra Harrington",
    location: "London, UK",
    phase: "Concept Development",
    status: "active",
    progress: 68,
    budget: "$420,000",
    spent: 60,
    startDate: "Jan 2025",
    endDate: "Aug 2025",
    image: "https://images.unsplash.com/photo-1600607687939-ce8a6c25118c?w=600&h=360&fit=crop",
    team: [{ initials: "SC", name: "Sara Chen" }, { initials: "JL", name: "James Lee" }, { initials: "MR", name: "Marcus R." }],
    tags: ["Residential", "Luxury", "Penthouse"],
    description: "Full-floor penthouse redesign with custom joinery, curated art collection integration, and panoramic terrace landscaping.",
    rooms: 7,
    sqft: 4200,
  },
  {
    id: "2",
    name: "Maison Rivière",
    client: "François Beaumont",
    location: "Paris, France",
    phase: "Proposal Review",
    status: "review",
    progress: 42,
    budget: "$890,000",
    spent: 30,
    startDate: "Mar 2025",
    endDate: "Nov 2025",
    image: "https://images.unsplash.com/photo-1586023492125-27b2c045efd7?w=600&h=360&fit=crop",
    team: [{ initials: "SC", name: "Sara Chen" }, { initials: "TR", name: "Tara Roy" }],
    tags: ["Residential", "Haussmann", "Heritage"],
    description: "Sensitive renovation of a historic Haussmann apartment, blending period features with contemporary comfort and bespoke furniture.",
    rooms: 9,
    sqft: 3600,
  },
  {
    id: "3",
    name: "The Aldgate Loft",
    client: "Ngozi Okafor",
    location: "London, UK",
    phase: "Construction Documents",
    status: "active",
    progress: 85,
    budget: "$210,000",
    spent: 75,
    startDate: "Oct 2024",
    endDate: "Jul 2025",
    image: "https://images.unsplash.com/photo-1618221195710-dd6b41faaea6?w=600&h=360&fit=crop",
    team: [{ initials: "JL", name: "James Lee" }, { initials: "MR", name: "Marcus R." }, { initials: "TD", name: "Thea D." }],
    tags: ["Residential", "Industrial", "Loft"],
    description: "Industrial loft conversion featuring exposed structural elements, custom steel partitions, and a curated palette of warm raw materials.",
    rooms: 4,
    sqft: 2800,
  },
  {
    id: "4",
    name: "Villa Almería",
    client: "Catalina Morales",
    location: "Almería, Spain",
    phase: "On Hold — Client Review",
    status: "hold",
    progress: 30,
    budget: "$1,200,000",
    spent: 20,
    startDate: "Feb 2025",
    endDate: "Jan 2026",
    image: "https://images.unsplash.com/photo-1512917774080-9991f1c4c750?w=600&h=360&fit=crop",
    team: [{ initials: "SC", name: "Sara Chen" }],
    tags: ["Residential", "Mediterranean", "Villa"],
    description: "Coastal villa redesign emphasising indoor-outdoor living, natural limestone finishes, and artisanal tile work throughout.",
    rooms: 12,
    sqft: 8500,
  },
  {
    id: "5",
    name: "Shoreditch Studio",
    client: "Pixel Labs Ltd.",
    location: "London, UK",
    phase: "Complete",
    status: "complete",
    progress: 100,
    budget: "$320,000",
    spent: 98,
    startDate: "Sep 2024",
    endDate: "Apr 2025",
    image: "https://images.unsplash.com/photo-1497366216548-37526070297c?w=600&h=360&fit=crop",
    team: [{ initials: "JL", name: "James Lee" }, { initials: "TD", name: "Thea D." }],
    tags: ["Commercial", "Office", "Tech"],
    description: "Creative studio office designed around collaboration zones, brand storytelling, and premium material finishes reflecting a design-forward culture.",
    rooms: 6,
    sqft: 3200,
  },
  {
    id: "6",
    name: "The Cotswolds Cottage",
    client: "H. & R. Whitmore",
    location: "Cotswolds, UK",
    phase: "Concept Development",
    status: "active",
    progress: 22,
    budget: "$185,000",
    spent: 15,
    startDate: "May 2025",
    endDate: "Feb 2026",
    image: "https://images.unsplash.com/photo-1568605114967-8130f3a36994?w=600&h=360&fit=crop",
    team: [{ initials: "TR", name: "Tara Roy" }, { initials: "MR", name: "Marcus R." }],
    tags: ["Residential", "Country", "Heritage"],
    description: "Charming Grade II listed cottage renovation preserving original stone features while introducing contemporary comfort and sustainability measures.",
    rooms: 5,
    sqft: 2100,
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

export default function Projects() {
  const [selected, setSelected] = useState<string | null>(null);
  const [filter, setFilter] = useState<string>("all");

  const filtered = filter === "all"
    ? PROJECTS
    : PROJECTS.filter((p) => p.status === filter);

  const project = PROJECTS.find((p) => p.id === selected);

  return (
    <div data-cmp="Projects" className="flex-1 flex overflow-hidden">
      {/* Main list */}
      <div className="flex-1 overflow-y-auto scrollbar-thin bg-background">
        <div className="max-w-[900px] mx-auto px-6 py-5">
          {/* Toolbar */}
          <div className="flex items-center gap-3 mb-5">
            <div className="flex items-center gap-2 bg-surface border border-border rounded-lg px-3 py-2 flex-1 max-w-xs">
              <Search size={13} className="text-muted-foreground" />
              <input
                type="text"
                placeholder="Search projects…"
                className="bg-transparent text-[12px] font-sans text-foreground placeholder:text-muted-foreground flex-1 outline-none"
              />
            </div>
            <button className="flex items-center gap-1.5 px-3 py-2 border border-border rounded-lg text-[12px] font-sans text-muted-foreground hover:text-foreground hover:bg-surface transition-all">
              <Filter size={12} /> Filter
            </button>
            <div className="flex items-center gap-1 bg-surface border border-border rounded-lg p-1 ml-auto">
              {["all", "active", "review", "hold", "complete"].map((f) => (
                <button
                  key={f}
                  onClick={() => setFilter(f)}
                  className={`px-3 py-1 rounded-md text-[11px] font-sans font-medium capitalize transition-all ${
                    filter === f
                      ? "bg-charcoal text-primary-foreground"
                      : "text-muted-foreground hover:text-foreground"
                  }`}
                >
                  {f === "all" ? "All" : STATUS_LABEL[f]}
                </button>
              ))}
            </div>
            <button className="flex items-center gap-1.5 bg-charcoal text-primary-foreground px-3 py-2 rounded-lg text-[12px] font-sans font-medium hover:opacity-90 transition-all">
              <Plus size={13} /> New Project
            </button>
          </div>

          {/* Cards */}
          <div className="flex flex-col gap-3">
            {filtered.map((p) => (
              <button
                key={p.id}
                onClick={() => setSelected(selected === p.id ? null : p.id)}
                className={`w-full bg-surface border rounded-xl shadow-custom overflow-hidden hover:border-muted-foreground/30 transition-all text-left group ${
                  selected === p.id ? "border-charcoal" : "border-border"
                }`}
              >
                <div className="flex">
                  {/* Image */}
                  <div
                    className="w-52 h-36 image-card shrink-0"
                    style={{ backgroundImage: `url(${p.image})` }}
                  />
                  {/* Content */}
                  <div className="flex-1 px-5 py-4 min-w-0">
                    <div className="flex items-start justify-between gap-3 mb-2">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2 flex-wrap">
                          <h3 className="font-serif text-[16px] font-semibold text-foreground">
                            {p.name}
                          </h3>
                          <span className={`tag-pill ${STATUS_MAP[p.status]}`}>
                            {STATUS_LABEL[p.status]}
                          </span>
                        </div>
                        <div className="flex items-center gap-2 text-muted-foreground text-[11px] font-sans mt-0.5">
                          <span>{p.client}</span>
                          <span>·</span>
                          <MapPin size={9} />
                          <span>{p.location}</span>
                        </div>
                      </div>
                      <div className="flex -space-x-1.5 shrink-0">
                        {p.team.slice(0, 3).map((m) => (
                          <div
                            key={m.initials}
                            title={m.name}
                            className="w-6 h-6 rounded-full bg-warm-beige-dark border border-border flex items-center justify-center text-[9px] font-semibold text-charcoal"
                          >
                            {m.initials}
                          </div>
                        ))}
                      </div>
                    </div>

                    <p className="text-muted-foreground text-[11px] font-sans leading-relaxed mb-3 line-clamp-2">
                      {p.description}
                    </p>

                    {/* Tags */}
                    <div className="flex items-center gap-1.5 flex-wrap mb-3">
                      {p.tags.map((t) => (
                        <span key={t} className="tag-pill">{t}</span>
                      ))}
                    </div>

                    {/* Meta row */}
                    <div className="flex items-center gap-6">
                      <div className="flex items-center gap-1.5">
                        <DollarSign size={11} className="text-muted-foreground" />
                        <span className="text-[11px] font-sans text-muted-foreground">
                          {p.budget}
                        </span>
                      </div>
                      <div className="flex items-center gap-1.5">
                        <CalendarDays size={11} className="text-muted-foreground" />
                        <span className="text-[11px] font-sans text-muted-foreground">
                          {p.startDate} – {p.endDate}
                        </span>
                      </div>
                      <div className="flex items-center gap-1.5">
                        <Users size={11} className="text-muted-foreground" />
                        <span className="text-[11px] font-sans text-muted-foreground">
                          {p.rooms} rooms · {p.sqft.toLocaleString()} sqft
                        </span>
                      </div>
                      {/* Progress */}
                      <div className="flex-1 flex items-center gap-2 ml-auto max-w-[180px]">
                        <div className="flex-1 h-1.5 bg-muted rounded-full overflow-hidden">
                          <div
                            className={`h-full rounded-full ${PROGRESS_COLOR[p.status]}`}
                            style={{ width: `${p.progress}%` }}
                          />
                        </div>
                        <span className="text-[11px] font-sans font-medium text-foreground w-8 text-right">
                          {p.progress}%
                        </span>
                      </div>
                    </div>
                  </div>
                  <div className="flex items-center pr-4">
                    <ArrowRight size={14} className="text-muted-foreground opacity-0 group-hover:opacity-100 transition-all" />
                  </div>
                </div>
                {/* Phase bar */}
                <div className="border-t border-border px-5 py-2 flex items-center gap-2 bg-muted/30">
                  <Clock size={10} className="text-muted-foreground" />
                  <span className="text-[10px] font-sans text-muted-foreground">
                    Phase: <span className="font-medium text-foreground">{p.phase}</span>
                  </span>
                </div>
              </button>
            ))}
          </div>
          <div className="h-6" />
        </div>
      </div>

      {/* Detail Panel */}
      {project && (
        <div className="w-80 shrink-0 border-l border-border bg-surface overflow-y-auto scrollbar-thin">
          <div className="sticky top-0 bg-surface border-b border-border px-5 py-3.5 flex items-center justify-between z-10">
            <h3 className="font-serif text-[15px] font-semibold text-foreground truncate">
              {project.name}
            </h3>
            <button
              onClick={() => setSelected(null)}
              className="w-6 h-6 rounded-md flex items-center justify-center text-muted-foreground hover:bg-accent hover:text-foreground transition-all"
            >
              <X size={13} />
            </button>
          </div>

          {/* Hero image */}
          <div
            className="h-44 image-card border-b border-border"
            style={{ backgroundImage: `url(${project.image})` }}
          />

          <div className="px-5 py-4">
            <span className={`tag-pill ${STATUS_MAP[project.status]}`}>
              {STATUS_LABEL[project.status]}
            </span>
            <p className="text-muted-foreground text-[11px] font-sans mt-2 leading-relaxed">
              {project.description}
            </p>

            {/* Stats */}
            <div className="grid mt-4 gap-y-3">
              {[
                { label: "Client", value: project.client },
                { label: "Location", value: project.location },
                { label: "Phase", value: project.phase },
                { label: "Budget", value: project.budget },
                { label: "Timeline", value: `${project.startDate} – ${project.endDate}` },
                { label: "Area", value: `${project.rooms} rooms · ${project.sqft.toLocaleString()} sqft` },
              ].map((row) => (
                <div key={row.label} className="flex items-start justify-between gap-2">
                  <span className="text-muted-foreground text-[10px] font-sans uppercase tracking-wide shrink-0">
                    {row.label}
                  </span>
                  <span className="text-foreground text-[12px] font-sans font-medium text-right">
                    {row.value}
                  </span>
                </div>
              ))}
            </div>

            {/* Progress */}
            <div className="mt-4">
              <div className="flex justify-between text-[10px] font-sans text-muted-foreground mb-1">
                <span>Overall Progress</span>
                <span className="font-semibold text-foreground">{project.progress}%</span>
              </div>
              <div className="h-2 bg-muted rounded-full overflow-hidden">
                <div
                  className={`h-full rounded-full ${PROGRESS_COLOR[project.status]}`}
                  style={{ width: `${project.progress}%` }}
                />
              </div>
            </div>

            {/* Team */}
            <div className="mt-4">
              <p className="text-muted-foreground text-[10px] font-sans uppercase tracking-wide mb-2">Team</p>
              <div className="flex flex-col gap-2">
                {project.team.map((m) => (
                  <div key={m.initials} className="flex items-center gap-2">
                    <div className="w-6 h-6 rounded-full bg-warm-beige-dark flex items-center justify-center text-[9px] font-semibold text-charcoal">
                      {m.initials}
                    </div>
                    <span className="text-[12px] font-sans text-foreground">{m.name}</span>
                  </div>
                ))}
              </div>
            </div>

            {/* Tags */}
            <div className="mt-4 flex flex-wrap gap-1.5">
              {project.tags.map((t) => (
                <span key={t} className="tag-pill">{t}</span>
              ))}
            </div>

            {/* Actions */}
            <div className="mt-5 flex flex-col gap-2">
              <button className="w-full py-2 bg-charcoal text-primary-foreground rounded-lg text-[12px] font-sans font-medium hover:opacity-90 transition-all">
                Open Full Project
              </button>
              <button className="w-full py-2 border border-border text-foreground rounded-lg text-[12px] font-sans hover:bg-muted transition-all">
                View Moodboard
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
