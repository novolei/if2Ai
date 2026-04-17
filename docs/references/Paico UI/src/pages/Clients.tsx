import { useState } from "react";
import {
  Search, Plus, Mail, Phone, MapPin, Calendar,
  MessageSquare, ArrowUpRight, X
} from "lucide-react";

type Interaction = { date: string; type: string; note: string };

type Client = {
  id: string;
  name: string;
  company?: string;
  email: string;
  phone: string;
  location: string;
  status: "active" | "prospective" | "past";
  joined: string;
  avatar: string;
  projects: string[];
  totalValue: string;
  lastContact: string;
  interactions: Interaction[];
  notes: string;
};

const CLIENTS: Client[] = [
  {
    id: "1",
    name: "Alexandra Harrington",
    company: undefined,
    email: "alexandra@harrington.co.uk",
    phone: "+44 7700 900123",
    location: "Kensington, London",
    status: "active",
    joined: "Jan 2024",
    avatar: "AH",
    projects: ["Kensington Penthouse"],
    totalValue: "$420,000",
    lastContact: "2d ago",
    notes: "Prefers morning calls. Has strong opinions on materials — always present samples in person.",
    interactions: [
      { date: "12 Jun 2025", type: "Meeting", note: "Presented Space Planning revisions for master suite" },
      { date: "05 Jun 2025", type: "Email", note: "Sent revised proposal v3.1 for review" },
      { date: "28 May 2025", type: "Call", note: "Discussed material palette — client prefers honed stone" },
    ],
  },
  {
    id: "2",
    name: "François Beaumont",
    company: "Beaumont Investissements",
    email: "f.beaumont@beau-invest.fr",
    phone: "+33 6 12 34 56 78",
    location: "Paris, France",
    status: "active",
    joined: "Mar 2025",
    avatar: "FB",
    projects: ["Maison Rivière"],
    totalValue: "$890,000",
    lastContact: "5d ago",
    notes: "French-speaking client — prefer bilingual presentations. Formal correspondence only.",
    interactions: [
      { date: "09 Jun 2025", type: "Meeting", note: "Paris site visit and brief review" },
      { date: "02 Jun 2025", type: "Email", note: "Submitted concept presentation" },
    ],
  },
  {
    id: "3",
    name: "Ngozi Okafor",
    company: "Okafor Creative Ltd.",
    email: "ngozi@okafor-creative.com",
    phone: "+44 7911 223344",
    location: "Shoreditch, London",
    status: "active",
    joined: "Oct 2024",
    avatar: "NO",
    projects: ["The Aldgate Loft"],
    totalValue: "$210,000",
    lastContact: "1w ago",
    notes: "Creative director — loves process transparency. Share progress photos weekly.",
    interactions: [
      { date: "06 Jun 2025", type: "Site Visit", note: "Steel partition installation review" },
      { date: "30 May 2025", type: "Call", note: "Updated on flooring timeline delay" },
    ],
  },
  {
    id: "4",
    name: "Catalina Morales",
    company: undefined,
    email: "c.morales@gmail.com",
    phone: "+34 612 345 678",
    location: "Almería, Spain",
    status: "active",
    joined: "Feb 2025",
    avatar: "CM",
    projects: ["Villa Almería"],
    totalValue: "$1,200,000",
    lastContact: "2w ago",
    notes: "Project currently on hold pending planning approval. Follow up end of June.",
    interactions: [
      { date: "02 Jun 2025", type: "Email", note: "Concept update shared — awaiting client feedback" },
      { date: "15 May 2025", type: "Call", note: "Planning delay confirmed — project paused" },
    ],
  },
  {
    id: "5",
    name: "Pixel Labs Ltd.",
    company: "Pixel Labs Ltd.",
    email: "studio@pixellabs.io",
    phone: "+44 20 7946 0823",
    location: "Shoreditch, London",
    status: "past",
    joined: "Sep 2024",
    avatar: "PL",
    projects: ["Shoreditch Studio"],
    totalValue: "$320,000",
    lastContact: "1m ago",
    notes: "Project completed. Happy client — potential referral source. Send case study when published.",
    interactions: [
      { date: "28 Apr 2025", type: "Handover", note: "Final project handover meeting" },
      { date: "10 Apr 2025", type: "Snagging", note: "Snagging list sign-off" },
    ],
  },
  {
    id: "6",
    name: "H. & R. Whitmore",
    company: undefined,
    email: "r.whitmore@whitmorefamily.com",
    phone: "+44 7800 111222",
    location: "Cotswolds, UK",
    status: "active",
    joined: "May 2025",
    avatar: "HW",
    projects: ["The Cotswolds Cottage"],
    totalValue: "$185,000",
    lastContact: "3d ago",
    notes: "New client — referred by Alexandra Harrington. Prefer low-tech communication (email, no apps).",
    interactions: [
      { date: "11 Jun 2025", type: "Meeting", note: "Initial brief and site survey" },
    ],
  },
];

const STATUS_CONFIG: Record<string, { label: string; cls: string }> = {
  active: { label: "Active", cls: "status-active" },
  prospective: { label: "Prospective", cls: "status-review" },
  past: { label: "Past Client", cls: "bg-muted text-muted-foreground border-transparent" },
};

const INTERACTION_ICON: Record<string, string> = {
  Meeting: "🤝",
  Email: "📧",
  Call: "📞",
  "Site Visit": "🏗️",
  Handover: "🎉",
  Snagging: "🔍",
};

export default function Clients() {
  const [search, setSearch] = useState("");
  const [statusFilter, setStatusFilter] = useState("all");
  const [selected, setSelected] = useState<string>("1");

  const filtered = CLIENTS.filter((c) => {
    const matchSearch =
      c.name.toLowerCase().includes(search.toLowerCase()) ||
      (c.company ?? "").toLowerCase().includes(search.toLowerCase());
    const matchStatus = statusFilter === "all" || c.status === statusFilter;
    return matchSearch && matchStatus;
  });

  const client = CLIENTS.find((c) => c.id === selected);

  return (
    <div data-cmp="Clients" className="flex-1 flex overflow-hidden">
      {/* Client list */}
      <div className="w-80 shrink-0 border-r border-border bg-surface flex flex-col">
        <div className="px-4 pt-4 pb-3 border-b border-border">
          <div className="flex items-center gap-2 bg-background border border-border rounded-lg px-3 py-2 mb-3">
            <Search size={13} className="text-muted-foreground" />
            <input
              type="text"
              placeholder="Search clients…"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="bg-transparent text-[12px] font-sans text-foreground placeholder:text-muted-foreground flex-1 outline-none"
            />
          </div>
          <div className="flex items-center gap-1">
            {["all", "active", "past"].map((f) => (
              <button
                key={f}
                onClick={() => setStatusFilter(f)}
                className={`flex-1 py-1 rounded-md text-[11px] font-sans capitalize transition-all ${
                  statusFilter === f
                    ? "bg-charcoal text-primary-foreground"
                    : "text-muted-foreground hover:text-foreground hover:bg-muted"
                }`}
              >
                {f === "all" ? "All" : f === "active" ? "Active" : "Past"}
              </button>
            ))}
          </div>
        </div>

        <div className="flex-1 overflow-y-auto scrollbar-thin py-1">
          {filtered.map((c) => (
            <button
              key={c.id}
              onClick={() => setSelected(c.id)}
              className={`w-full text-left px-4 py-3 border-l-2 transition-all ${
                selected === c.id
                  ? "bg-muted border-charcoal"
                  : "border-transparent hover:bg-muted/60"
              }`}
            >
              <div className="flex items-center gap-3">
                <div className="w-9 h-9 rounded-full bg-warm-beige-dark flex items-center justify-center text-[11px] font-semibold text-charcoal shrink-0">
                  {c.avatar}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 justify-between">
                    <span className="text-[12px] font-sans font-semibold text-foreground truncate">
                      {c.name}
                    </span>
                    <span className={`tag-pill shrink-0 ${STATUS_CONFIG[c.status].cls}`}>
                      {STATUS_CONFIG[c.status].label}
                    </span>
                  </div>
                  <p className="text-muted-foreground text-[10px] font-sans truncate mt-0.5">
                    {c.projects[0]}
                  </p>
                  <p className="text-muted-foreground text-[10px] font-sans">
                    Last contact: {c.lastContact}
                  </p>
                </div>
              </div>
            </button>
          ))}
        </div>

        <div className="px-4 py-3 border-t border-border">
          <button className="w-full flex items-center justify-center gap-1.5 bg-charcoal text-primary-foreground py-2 rounded-lg text-[12px] font-sans font-medium hover:opacity-90 transition-all">
            <Plus size={13} /> Add Client
          </button>
        </div>
      </div>

      {/* Detail panel */}
      {client ? (
        <div className="flex-1 overflow-y-auto scrollbar-thin bg-background">
          <div className="max-w-[680px] mx-auto px-6 py-6">
            {/* Header */}
            <div className="bg-surface border border-border rounded-xl shadow-custom p-5 mb-4">
              <div className="flex items-start gap-4">
                <div className="w-14 h-14 rounded-full bg-warm-beige-dark flex items-center justify-center text-[16px] font-semibold text-charcoal shrink-0">
                  {client.avatar}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 flex-wrap">
                    <h2 className="font-serif text-[20px] font-semibold text-foreground">
                      {client.name}
                    </h2>
                    <span className={`tag-pill ${STATUS_CONFIG[client.status].cls}`}>
                      {STATUS_CONFIG[client.status].label}
                    </span>
                  </div>
                  {client.company && (
                    <p className="text-muted-foreground text-[12px] font-sans mt-0.5">{client.company}</p>
                  )}
                  <div className="flex items-center gap-4 mt-2 flex-wrap">
                    <div className="flex items-center gap-1 text-muted-foreground text-[11px] font-sans">
                      <MapPin size={10} /> {client.location}
                    </div>
                    <div className="flex items-center gap-1 text-muted-foreground text-[11px] font-sans">
                      <Calendar size={10} /> Client since {client.joined}
                    </div>
                  </div>
                </div>
                <div className="flex items-center gap-2 shrink-0">
                  <button className="w-8 h-8 rounded-lg border border-border flex items-center justify-center text-muted-foreground hover:bg-muted hover:text-foreground transition-all">
                    <Mail size={13} />
                  </button>
                  <button className="w-8 h-8 rounded-lg border border-border flex items-center justify-center text-muted-foreground hover:bg-muted hover:text-foreground transition-all">
                    <Phone size={13} />
                  </button>
                  <button className="w-8 h-8 rounded-lg border border-border flex items-center justify-center text-muted-foreground hover:bg-muted hover:text-foreground transition-all">
                    <MessageSquare size={13} />
                  </button>
                </div>
              </div>

              {/* Contact info */}
              <div className="flex gap-4 mt-4 pt-4 border-t border-border flex-wrap">
                <div>
                  <p className="text-muted-foreground text-[10px] font-sans uppercase tracking-wide">Email</p>
                  <p className="text-foreground text-[12px] font-sans mt-0.5">{client.email}</p>
                </div>
                <div>
                  <p className="text-muted-foreground text-[10px] font-sans uppercase tracking-wide">Phone</p>
                  <p className="text-foreground text-[12px] font-sans mt-0.5">{client.phone}</p>
                </div>
                <div>
                  <p className="text-muted-foreground text-[10px] font-sans uppercase tracking-wide">Total Project Value</p>
                  <p className="text-foreground text-[12px] font-sans font-semibold mt-0.5">{client.totalValue}</p>
                </div>
              </div>
            </div>

            {/* Projects */}
            <div className="bg-surface border border-border rounded-xl shadow-custom p-5 mb-4">
              <div className="flex items-center justify-between mb-3">
                <h3 className="font-serif text-[14px] font-semibold text-foreground">Projects</h3>
                <button className="text-[11px] font-sans text-muted-foreground hover:text-foreground flex items-center gap-1">
                  View all <ArrowUpRight size={10} />
                </button>
              </div>
              {client.projects.map((p) => (
                <div key={p} className="flex items-center gap-3 py-2 border-b border-border last:border-0">
                  <div className="w-1.5 h-1.5 rounded-full bg-olive" />
                  <span className="text-[12px] font-sans text-foreground flex-1">{p}</span>
                  <span className="text-[11px] font-sans text-muted-foreground">{client.totalValue}</span>
                </div>
              ))}
            </div>

            {/* Notes */}
            <div className="bg-surface border border-border rounded-xl shadow-custom p-5 mb-4">
              <h3 className="font-serif text-[14px] font-semibold text-foreground mb-2">Notes</h3>
              <p className="text-muted-foreground text-[12px] font-sans leading-relaxed">{client.notes}</p>
            </div>

            {/* Interactions */}
            <div className="bg-surface border border-border rounded-xl shadow-custom p-5">
              <div className="flex items-center justify-between mb-3">
                <h3 className="font-serif text-[14px] font-semibold text-foreground">Interaction History</h3>
                <button className="flex items-center gap-1 text-[11px] font-sans text-muted-foreground hover:text-foreground">
                  <Plus size={10} /> Log
                </button>
              </div>
              <div className="flex flex-col gap-3">
                {client.interactions.map((item, i) => (
                  <div key={i} className="flex items-start gap-3">
                    <div className="w-7 h-7 rounded-md bg-muted flex items-center justify-center text-[13px] shrink-0 mt-0.5">
                      {INTERACTION_ICON[item.type] ?? "📝"}
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-baseline justify-between gap-2 mb-0.5">
                        <span className="text-[11px] font-sans font-semibold text-foreground">{item.type}</span>
                        <span className="text-muted-foreground text-[10px] font-sans shrink-0">{item.date}</span>
                      </div>
                      <p className="text-[11px] font-sans text-muted-foreground leading-relaxed">{item.note}</p>
                    </div>
                  </div>
                ))}
              </div>
            </div>
            <div className="h-6" />
          </div>
        </div>
      ) : (
        <div className="flex-1 flex items-center justify-center text-muted-foreground text-[13px] font-sans">
          Select a client to view details
        </div>
      )}
    </div>
  );
}
