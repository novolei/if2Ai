import { useState } from "react";
import type { ReactNode } from "react";
import {
  FileText, ChevronRight, Send, Download, MessageSquare,
  CheckCircle, Clock, RotateCcw, Plus, Eye
} from "lucide-react";

type Section = {
  id: string;
  title: string;
  status: "approved" | "pending" | "revision";
  pages: number;
};

type Proposal = {
  id: string;
  title: string;
  project: string;
  client: string;
  version: string;
  date: string;
  status: "approved" | "pending" | "revision" | "draft";
  sections: Section[];
  comments: number;
};

const PROPOSALS: Proposal[] = [
  {
    id: "1",
    title: "Kensington Penthouse — Design Proposal v3",
    project: "Kensington Penthouse",
    client: "Alexandra Harrington",
    version: "v3.1",
    date: "14 Jun 2025",
    status: "pending",
    comments: 4,
    sections: [
      { id: "s1", title: "Executive Summary", status: "approved", pages: 2 },
      { id: "s2", title: "Design Concept", status: "approved", pages: 6 },
      { id: "s3", title: "Space Planning", status: "revision", pages: 4 },
      { id: "s4", title: "Material Palette", status: "pending", pages: 8 },
      { id: "s5", title: "FF&E Schedule", status: "pending", pages: 12 },
      { id: "s6", title: "Budget Estimate", status: "approved", pages: 3 },
      { id: "s7", title: "Project Timeline", status: "pending", pages: 2 },
    ],
  },
  {
    id: "2",
    title: "Maison Rivière — Concept Presentation",
    project: "Maison Rivière",
    client: "François Beaumont",
    version: "v1.0",
    date: "02 Jun 2025",
    status: "revision",
    comments: 9,
    sections: [
      { id: "s1", title: "Vision Statement", status: "approved", pages: 1 },
      { id: "s2", title: "Inspiration & References", status: "approved", pages: 5 },
      { id: "s3", title: "Proposed Layout", status: "revision", pages: 6 },
      { id: "s4", title: "Finishes & Palette", status: "revision", pages: 7 },
      { id: "s5", title: "Lighting Design", status: "pending", pages: 4 },
    ],
  },
  {
    id: "3",
    title: "Shoreditch Studio — Final Handover",
    project: "Shoreditch Studio",
    client: "Pixel Labs Ltd.",
    version: "v5.0",
    date: "28 Apr 2025",
    status: "approved",
    comments: 0,
    sections: [
      { id: "s1", title: "Project Overview", status: "approved", pages: 2 },
      { id: "s2", title: "As-Built Drawings", status: "approved", pages: 10 },
      { id: "s3", title: "Materials Record", status: "approved", pages: 8 },
      { id: "s4", title: "Photography", status: "approved", pages: 6 },
    ],
  },
];

const STATUS_ICON: Record<string, ReactNode> = {
  approved: <CheckCircle size={11} className="text-olive" />,
  pending: <Clock size={11} className="text-gold" />,
  revision: <RotateCcw size={11} className="text-terracotta" />,
  draft: <FileText size={11} className="text-muted-foreground" />,
};
const STATUS_LABEL: Record<string, string> = {
  approved: "Approved",
  pending: "Pending",
  revision: "Needs Revision",
  draft: "Draft",
};
const STATUS_CLS: Record<string, string> = {
  approved: "status-complete",
  pending: "status-review",
  revision: "status-hold",
  draft: "bg-muted text-muted-foreground border-transparent",
};

const COMMENTS = [
  { author: "Alexandra H.", avatar: "AH", time: "2h ago", section: "Space Planning", text: "The master suite layout feels slightly cramped near the dressing area — could we explore a wider corridor?" },
  { author: "Sara Chen", avatar: "SC", time: "3h ago", section: "Space Planning", text: "Noted. I'll revise the dressing corridor width to 900mm and re-check circulation paths." },
  { author: "Alexandra H.", avatar: "AH", time: "1d ago", section: "Material Palette", text: "Loving the travertine selection. Can we see it in the honed rather than polished finish?" },
  { author: "James Lee", avatar: "JL", time: "1d ago", section: "Material Palette", text: "Absolutely — I'll update the material boards with the honed samples." },
];

export default function Proposals() {
  const [selectedProposal, setSelectedProposal] = useState<string>("1");
  const [selectedSection, setSelectedSection] = useState<string>("s1");
  const [showComments, setShowComments] = useState(true);

  const proposal = PROPOSALS.find((p) => p.id === selectedProposal) ?? PROPOSALS[0];
  const section = proposal.sections.find((s) => s.id === selectedSection) ?? proposal.sections[0];

  return (
    <div data-cmp="Proposals" className="flex-1 flex overflow-hidden h-full">
      {/* Left nav — proposal list */}
      <div className="w-56 shrink-0 border-r border-border bg-surface flex flex-col">
        <div className="px-4 pt-4 pb-3 border-b border-border">
          <div className="flex items-center justify-between mb-3">
            <h3 className="font-serif text-[13px] font-semibold text-foreground">Proposals</h3>
            <button className="w-6 h-6 rounded-md flex items-center justify-center text-muted-foreground hover:bg-accent hover:text-foreground transition-all">
              <Plus size={12} />
            </button>
          </div>
        </div>
        <div className="flex-1 overflow-y-auto scrollbar-thin py-2">
          {PROPOSALS.map((p) => (
            <button
              key={p.id}
              onClick={() => { setSelectedProposal(p.id); setSelectedSection(p.sections[0].id); }}
              className={`w-full text-left px-3 py-3 group transition-all border-l-2 ${
                selectedProposal === p.id
                  ? "bg-muted border-charcoal"
                  : "border-transparent hover:bg-muted/60"
              }`}
            >
              <div className="flex items-center gap-1.5 mb-1">
                <span className={`tag-pill ${STATUS_CLS[p.status]}`}>
                  {STATUS_LABEL[p.status]}
                </span>
                <span className="text-muted-foreground text-[10px] font-sans">{p.version}</span>
              </div>
              <p className="text-[11px] font-sans font-semibold text-foreground leading-snug">
                {p.title.split("—")[0].trim()}
              </p>
              <p className="text-muted-foreground text-[10px] font-sans mt-0.5">{p.client}</p>
              {p.comments > 0 && (
                <div className="flex items-center gap-1 mt-1">
                  <MessageSquare size={9} className="text-muted-foreground" />
                  <span className="text-[10px] font-sans text-muted-foreground">{p.comments} comments</span>
                </div>
              )}
            </button>
          ))}
        </div>
      </div>

      {/* Centre — section navigator + canvas */}
      <div className="flex flex-col flex-1 min-w-0 overflow-hidden border-r border-border">
        {/* Canvas header */}
        <div className="px-5 py-3 border-b border-border bg-surface flex items-center justify-between">
          <div className="min-w-0">
            <h2 className="font-serif text-[15px] font-semibold text-foreground truncate">
              {proposal.title}
            </h2>
            <p className="text-muted-foreground text-[11px] font-sans">
              {proposal.client} · {proposal.date}
            </p>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <button className="flex items-center gap-1.5 px-3 py-1.5 border border-border rounded-lg text-[11px] font-sans text-muted-foreground hover:text-foreground hover:bg-muted transition-all">
              <Eye size={11} /> Preview
            </button>
            <button className="flex items-center gap-1.5 px-3 py-1.5 border border-border rounded-lg text-[11px] font-sans text-muted-foreground hover:text-foreground hover:bg-muted transition-all">
              <Download size={11} /> Export
            </button>
            <button className="flex items-center gap-1.5 px-3 py-1.5 bg-charcoal text-primary-foreground rounded-lg text-[11px] font-sans font-medium hover:opacity-90 transition-all">
              <Send size={11} /> Share
            </button>
          </div>
        </div>

        {/* Section navigator */}
        <div className="border-b border-border bg-surface px-5 py-2 flex items-center gap-1 overflow-x-auto scrollbar-thin">
          {proposal.sections.map((s, i) => (
            <div key={s.id} className="flex items-center gap-1 shrink-0">
              <button
                onClick={() => setSelectedSection(s.id)}
                className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-[11px] font-sans transition-all ${
                  selectedSection === s.id
                    ? "bg-charcoal text-primary-foreground"
                    : "text-muted-foreground hover:text-foreground hover:bg-muted"
                }`}
              >
                {STATUS_ICON[s.status]}
                {s.title}
              </button>
              {i < proposal.sections.length - 1 && (
                <ChevronRight size={10} className="text-muted-foreground" />
              )}
            </div>
          ))}
        </div>

        {/* Canvas area */}
        <div className="flex-1 overflow-y-auto scrollbar-thin bg-background p-6">
          {/* Simulated document canvas */}
          <div className="max-w-[640px] mx-auto">
            <div className="bg-surface border border-border rounded-xl shadow-custom overflow-hidden">
              {/* Document header */}
              <div className="px-8 py-6 border-b border-border">
                <div className="flex items-center justify-between mb-4">
                  <span className="font-serif text-[11px] uppercase tracking-widest text-muted-foreground">
                    ATELIER Studio OS
                  </span>
                  <div className={`tag-pill ${STATUS_CLS[section.status]}`}>
                    {STATUS_LABEL[section.status]}
                  </div>
                </div>
                <h1 className="font-serif text-[28px] font-light text-foreground mb-1">
                  {section.title}
                </h1>
                <p className="text-muted-foreground text-[12px] font-sans">
                  {proposal.title} · {section.pages} pages
                </p>
              </div>

              {/* Mock content blocks */}
              <div className="px-8 py-6 space-y-5">
                {/* Hero image placeholder */}
                <div
                  className="h-52 image-card rounded-lg border border-border"
                  style={{
                    backgroundImage: `url(https://images.unsplash.com/photo-1600607687939-ce8a6c25118c?w=640&h=420&fit=crop)`,
                  }}
                />

                {/* Body text blocks */}
                <div className="space-y-2">
                  <div className="h-3 bg-muted rounded-full w-full" />
                  <div className="h-3 bg-muted rounded-full w-11/12" />
                  <div className="h-3 bg-muted rounded-full w-4/5" />
                  <div className="h-3 bg-muted rounded-full w-full" />
                  <div className="h-3 bg-muted rounded-full w-10/12" />
                </div>

                {/* 2-col images */}
                <div className="flex gap-3">
                  <div
                    className="flex-1 h-36 image-card rounded-lg border border-border"
                    style={{
                      backgroundImage: `url(https://images.unsplash.com/photo-1555041469-a586c61ea9bc?w=320&h=240&fit=crop)`,
                    }}
                  />
                  <div
                    className="flex-1 h-36 image-card rounded-lg border border-border"
                    style={{
                      backgroundImage: `url(https://images.unsplash.com/photo-1616486338812-3dadae4b4ace?w=320&h=240&fit=crop)`,
                    }}
                  />
                </div>

                {/* More body text */}
                <div className="space-y-2">
                  <div className="h-3 bg-muted rounded-full w-full" />
                  <div className="h-3 bg-muted rounded-full w-9/12" />
                  <div className="h-3 bg-muted rounded-full w-11/12" />
                </div>

                {/* Callout box */}
                <div className="border-l-2 border-gold bg-gold/10 px-4 py-3 rounded-r-lg">
                  <p className="font-serif text-[13px] text-foreground italic">
                    "The design concept revolves around the interplay of warm natural materials, filtered light, and carefully considered spatial transitions."
                  </p>
                </div>

                <div className="space-y-2">
                  <div className="h-3 bg-muted rounded-full w-7/12" />
                  <div className="h-3 bg-muted rounded-full w-full" />
                </div>
              </div>

              {/* Page footer */}
              <div className="px-8 py-3 border-t border-border flex items-center justify-between">
                <span className="text-muted-foreground text-[10px] font-sans">
                  {proposal.version} · {proposal.date}
                </span>
                <span className="text-muted-foreground text-[10px] font-sans">
                  Page 1 of {section.pages}
                </span>
              </div>
            </div>
          </div>
          <div className="h-6" />
        </div>
      </div>

      {/* Right panel — comments */}
      {showComments && (
        <div className="w-72 shrink-0 bg-surface flex flex-col overflow-hidden">
          <div className="px-4 py-3.5 border-b border-border flex items-center justify-between">
            <h3 className="font-serif text-[13px] font-semibold text-foreground">
              Comments ({COMMENTS.length})
            </h3>
            <button
              onClick={() => setShowComments(false)}
              className="w-6 h-6 rounded-md flex items-center justify-center text-muted-foreground hover:bg-accent hover:text-foreground transition-all text-[13px]"
            >
              ✕
            </button>
          </div>
          <div className="flex-1 overflow-y-auto scrollbar-thin px-4 py-3 space-y-4">
            {COMMENTS.map((c, i) => (
              <div key={i} className="flex gap-2.5">
                <div className="w-6 h-6 rounded-full bg-warm-beige-dark flex items-center justify-center text-[9px] font-semibold text-charcoal shrink-0 mt-0.5">
                  {c.avatar}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex items-baseline justify-between gap-1 mb-0.5">
                    <span className="text-[11px] font-sans font-semibold text-foreground">
                      {c.author}
                    </span>
                    <span className="text-muted-foreground text-[10px] font-sans shrink-0">
                      {c.time}
                    </span>
                  </div>
                  <p className="text-[10px] font-sans text-muted-foreground mb-1">
                    re: <span className="font-medium text-foreground">{c.section}</span>
                  </p>
                  <p className="text-[11px] font-sans text-foreground leading-relaxed">
                    {c.text}
                  </p>
                </div>
              </div>
            ))}
          </div>
          <div className="px-4 py-3 border-t border-border">
            <div className="flex items-center gap-2 bg-muted rounded-lg px-3 py-2">
              <input
                type="text"
                placeholder="Add a comment…"
                className="bg-transparent flex-1 text-[11px] font-sans text-foreground placeholder:text-muted-foreground outline-none"
              />
              <button className="text-muted-foreground hover:text-foreground transition-all">
                <Send size={12} />
              </button>
            </div>
          </div>
        </div>
      )}

      {!showComments && (
        <button
          onClick={() => setShowComments(true)}
          className="w-10 shrink-0 border-l border-border bg-surface flex flex-col items-center justify-center gap-2 hover:bg-muted transition-all"
        >
          <MessageSquare size={14} className="text-muted-foreground" />
          <span className="text-[9px] font-sans text-muted-foreground rotate-90 whitespace-nowrap">
            Comments
          </span>
        </button>
      )}
    </div>
  );
}
