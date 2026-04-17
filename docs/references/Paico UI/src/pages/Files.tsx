import { useState } from "react";
import type { ComponentType } from "react";
import {
  Search, Plus, Upload, FileText, Image, Film, Archive,
  Download, Eye, Trash2, Clock, CheckCircle, FolderOpen
} from "lucide-react";

type FileItem = {
  id: string;
  name: string;
  type: "pdf" | "image" | "video" | "archive" | "doc";
  project: string;
  category: string;
  size: string;
  updated: string;
  version: string;
  status: "approved" | "review" | "draft";
  uploader: string;
  thumbnail?: string;
};

const FILES: FileItem[] = [
  { id: "1", name: "Kensington_SpacePlan_v3.pdf", type: "pdf", project: "Kensington Penthouse", category: "Drawings", size: "4.2 MB", updated: "12 Jun 2025", version: "v3", status: "review", uploader: "JL", thumbnail: undefined },
  { id: "2", name: "Kensington_Moodboard_Final.jpg", type: "image", project: "Kensington Penthouse", category: "Moodboards", size: "8.1 MB", updated: "10 Jun 2025", version: "v1", status: "approved", uploader: "SC", thumbnail: "https://images.unsplash.com/photo-1600607687939-ce8a6c25118c?w=200&h=140&fit=crop" },
  { id: "3", name: "Maison_Riviere_Proposal_v1.pdf", type: "pdf", project: "Maison Rivière", category: "Proposals", size: "12.4 MB", updated: "09 Jun 2025", version: "v1", status: "review", uploader: "SC", thumbnail: undefined },
  { id: "4", name: "Aldgate_Materials_Board.jpg", type: "image", project: "The Aldgate Loft", category: "Moodboards", size: "5.6 MB", updated: "08 Jun 2025", version: "v2", status: "approved", uploader: "JL", thumbnail: "https://images.unsplash.com/photo-1618221195710-dd6b41faaea6?w=200&h=140&fit=crop" },
  { id: "5", name: "Villa_Almeria_Concept.mp4", type: "video", project: "Villa Almería", category: "Presentations", size: "240 MB", updated: "06 Jun 2025", version: "v1", status: "draft", uploader: "SC", thumbnail: "https://images.unsplash.com/photo-1512917774080-9991f1c4c750?w=200&h=140&fit=crop" },
  { id: "6", name: "FF&E_Schedule_Kensington.xlsx", type: "doc", project: "Kensington Penthouse", category: "Schedules", size: "1.8 MB", updated: "05 Jun 2025", version: "v4", status: "approved", uploader: "MR", thumbnail: undefined },
  { id: "7", name: "Cotswolds_Survey_Photos.zip", type: "archive", project: "The Cotswolds Cottage", category: "Photography", size: "320 MB", updated: "03 Jun 2025", version: "v1", status: "approved", uploader: "TR", thumbnail: undefined },
  { id: "8", name: "Shoreditch_AsBuilt_Set.pdf", type: "pdf", project: "Shoreditch Studio", category: "Drawings", size: "22.7 MB", updated: "28 Apr 2025", version: "v5", status: "approved", uploader: "JL", thumbnail: undefined },
  { id: "9", name: "Maison_Riviere_Inspiration.jpg", type: "image", project: "Maison Rivière", category: "Moodboards", size: "3.9 MB", updated: "27 Apr 2025", version: "v1", status: "approved", uploader: "SC", thumbnail: "https://images.unsplash.com/photo-1586023492125-27b2c045efd7?w=200&h=140&fit=crop" },
  { id: "10", name: "Kensington_Budget_Summary.pdf", type: "pdf", project: "Kensington Penthouse", category: "Finance", size: "0.9 MB", updated: "26 Apr 2025", version: "v2", status: "approved", uploader: "SC", thumbnail: undefined },
  { id: "11", name: "Aldgate_Lighting_Schedule.xlsx", type: "doc", project: "The Aldgate Loft", category: "Schedules", size: "2.1 MB", updated: "20 Apr 2025", version: "v1", status: "review", uploader: "TD", thumbnail: undefined },
  { id: "12", name: "Villa_Almeria_Palette.jpg", type: "image", project: "Villa Almería", category: "Moodboards", size: "6.3 MB", updated: "15 Apr 2025", version: "v1", status: "draft", uploader: "SC", thumbnail: "https://images.unsplash.com/photo-1523217582562-09d0def993a6?w=200&h=140&fit=crop" },
];

const CATEGORIES = ["All", "Drawings", "Moodboards", "Proposals", "Schedules", "Finance", "Photography", "Presentations"];

const FILE_ICONS: Record<string, ComponentType<{ size?: number; className?: string }>> = {
  pdf: FileText,
  image: Image,
  video: Film,
  archive: Archive,
  doc: FileText,
};

const FILE_COLORS: Record<string, string> = {
  pdf: "text-terracotta",
  image: "text-olive",
  video: "text-gold",
  archive: "text-muted-foreground",
  doc: "text-status-complete",
};

const FILE_BG: Record<string, string> = {
  pdf: "bg-terracotta/10",
  image: "bg-olive/10",
  video: "bg-gold/10",
  archive: "bg-muted",
  doc: "bg-status-complete/10",
};

const STATUS_CONFIG: Record<string, { label: string; cls: string; icon: ComponentType<{ size?: number; className?: string }> }> = {
  approved: { label: "Approved", cls: "status-complete", icon: CheckCircle },
  review: { label: "In Review", cls: "status-review", icon: Clock },
  draft: { label: "Draft", cls: "bg-muted text-muted-foreground border-transparent", icon: FileText },
};

export default function Files() {
  const [category, setCategory] = useState("All");
  const [viewMode, setViewMode] = useState<"grid" | "list">("list");

  const filtered = category === "All" ? FILES : FILES.filter((f) => f.category === category);

  return (
    <div data-cmp="Files" className="flex-1 overflow-y-auto scrollbar-thin bg-background">
      <div className="max-w-[960px] mx-auto px-6 py-5">
        {/* Toolbar */}
        <div className="flex items-center gap-3 mb-4">
          <div className="flex items-center gap-2 bg-surface border border-border rounded-lg px-3 py-2 flex-1 max-w-xs">
            <Search size={13} className="text-muted-foreground" />
            <input
              type="text"
              placeholder="Search files…"
              className="bg-transparent text-[12px] font-sans text-foreground placeholder:text-muted-foreground flex-1 outline-none"
            />
          </div>
          {/* View toggle */}
          <div className="flex items-center bg-surface border border-border rounded-lg overflow-hidden">
            {(["list", "grid"] as const).map((v) => (
              <button
                key={v}
                onClick={() => setViewMode(v)}
                className={`px-3 py-2 text-[11px] font-sans capitalize transition-all ${
                  viewMode === v
                    ? "bg-charcoal text-primary-foreground"
                    : "text-muted-foreground hover:text-foreground"
                }`}
              >
                {v}
              </button>
            ))}
          </div>
          <button className="flex items-center gap-1.5 px-3 py-2 border border-border rounded-lg text-[12px] font-sans text-muted-foreground hover:text-foreground hover:bg-surface transition-all">
            <Upload size={12} /> Upload
          </button>
          <button className="flex items-center gap-1.5 bg-charcoal text-primary-foreground px-3 py-2 rounded-lg text-[12px] font-sans font-medium hover:opacity-90 transition-all ml-auto">
            <Plus size={13} /> New Folder
          </button>
        </div>

        {/* Category pills */}
        <div className="flex items-center gap-2 flex-wrap mb-5">
          {CATEGORIES.map((c) => (
            <button
              key={c}
              onClick={() => setCategory(c)}
              className={`flex items-center gap-1.5 px-3 py-1 rounded-full text-[11px] font-sans font-medium transition-all ${
                category === c
                  ? "bg-charcoal text-primary-foreground"
                  : "bg-surface border border-border text-muted-foreground hover:text-foreground"
              }`}
            >
              {c === "All" && <FolderOpen size={10} />}
              {c}
            </button>
          ))}
        </div>

        {viewMode === "list" ? (
          /* List view */
          <div className="bg-surface border border-border rounded-xl shadow-custom overflow-hidden">
            {/* Header */}
            <div className="flex items-center px-4 py-2.5 border-b border-border bg-muted/30">
              <span className="flex-1 text-[10px] font-sans text-muted-foreground uppercase tracking-wide">Name</span>
              <span className="w-36 text-[10px] font-sans text-muted-foreground uppercase tracking-wide">Project</span>
              <span className="w-24 text-[10px] font-sans text-muted-foreground uppercase tracking-wide">Category</span>
              <span className="w-20 text-[10px] font-sans text-muted-foreground uppercase tracking-wide">Status</span>
              <span className="w-16 text-[10px] font-sans text-muted-foreground uppercase tracking-wide">Size</span>
              <span className="w-24 text-[10px] font-sans text-muted-foreground uppercase tracking-wide">Updated</span>
              <span className="w-16 text-[10px] font-sans text-muted-foreground uppercase tracking-wide">Ver.</span>
              <span className="w-20" />
            </div>
            {filtered.map((file) => {
              const FileIcon = FILE_ICONS[file.type];
              const StatusIcon = STATUS_CONFIG[file.status].icon;
              return (
                <div
                  key={file.id}
                  className="flex items-center px-4 py-3 border-b border-border last:border-0 hover:bg-muted/30 transition-all group"
                >
                  <div className="flex-1 flex items-center gap-3 min-w-0 pr-4">
                    <div className={`w-8 h-8 rounded-md flex items-center justify-center ${FILE_BG[file.type]} shrink-0`}>
                      <FileIcon size={14} className={FILE_COLORS[file.type]} />
                    </div>
                    <div className="min-w-0">
                      <p className="text-[12px] font-sans font-medium text-foreground truncate">{file.name}</p>
                      <p className="text-[10px] font-sans text-muted-foreground">by {file.uploader}</p>
                    </div>
                  </div>
                  <span className="w-36 text-[11px] font-sans text-muted-foreground truncate pr-2">{file.project}</span>
                  <span className="w-24 text-[11px] font-sans text-muted-foreground">{file.category}</span>
                  <div className="w-20">
                    <span className={`tag-pill flex items-center gap-1 w-fit ${STATUS_CONFIG[file.status].cls}`}>
                      <StatusIcon size={9} />
                      {STATUS_CONFIG[file.status].label}
                    </span>
                  </div>
                  <span className="w-16 text-[11px] font-sans text-muted-foreground">{file.size}</span>
                  <span className="w-24 text-[11px] font-sans text-muted-foreground">{file.updated}</span>
                  <span className="w-16 text-[11px] font-sans text-muted-foreground font-medium">{file.version}</span>
                  <div className="w-20 flex items-center justify-end gap-1 opacity-0 group-hover:opacity-100 transition-all">
                    <button className="w-6 h-6 rounded flex items-center justify-center text-muted-foreground hover:bg-muted hover:text-foreground transition-all">
                      <Eye size={11} />
                    </button>
                    <button className="w-6 h-6 rounded flex items-center justify-center text-muted-foreground hover:bg-muted hover:text-foreground transition-all">
                      <Download size={11} />
                    </button>
                    <button className="w-6 h-6 rounded flex items-center justify-center text-muted-foreground hover:bg-muted hover:text-foreground transition-all">
                      <Trash2 size={11} />
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        ) : (
          /* Grid view */
          <div className="flex flex-wrap gap-3">
            {filtered.map((file) => {
              const FileIcon = FILE_ICONS[file.type];
              const StatusIcon = STATUS_CONFIG[file.status].icon;
              return (
                <div
                  key={file.id}
                  className="w-[calc(25%-9px)] bg-surface border border-border rounded-xl shadow-custom overflow-hidden hover:border-muted-foreground/30 transition-all group"
                >
                  {/* Thumbnail or icon */}
                  {file.thumbnail ? (
                    <div
                      className="h-28 image-card border-b border-border"
                      style={{ backgroundImage: `url(${file.thumbnail})` }}
                    />
                  ) : (
                    <div className={`h-28 flex items-center justify-center border-b border-border ${FILE_BG[file.type]}`}>
                      <FileIcon size={32} className={FILE_COLORS[file.type]} />
                    </div>
                  )}
                  <div className="p-3">
                    <p className="text-[11px] font-sans font-semibold text-foreground truncate">{file.name}</p>
                    <p className="text-muted-foreground text-[10px] font-sans truncate mt-0.5">{file.project}</p>
                    <div className="flex items-center justify-between mt-2">
                      <span className={`tag-pill flex items-center gap-1 ${STATUS_CONFIG[file.status].cls}`}>
                        <StatusIcon size={9} />
                        {STATUS_CONFIG[file.status].label}
                      </span>
                      <span className="text-muted-foreground text-[10px] font-sans">{file.version}</span>
                    </div>
                    <div className="flex items-center justify-between mt-2 pt-2 border-t border-border">
                      <span className="text-muted-foreground text-[10px] font-sans">{file.size}</span>
                      <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-all">
                        <button className="w-5 h-5 rounded flex items-center justify-center text-muted-foreground hover:text-foreground transition-all">
                          <Eye size={10} />
                        </button>
                        <button className="w-5 h-5 rounded flex items-center justify-center text-muted-foreground hover:text-foreground transition-all">
                          <Download size={10} />
                        </button>
                      </div>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        )}
        <div className="h-6" />
      </div>
    </div>
  );
}
