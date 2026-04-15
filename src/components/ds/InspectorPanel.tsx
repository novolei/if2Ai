import { useState } from "react";
import { ChevronRight, FolderOpen, Layers, Clock } from "lucide-react";
import FileListItem from "./FileListItem";
import type { FileEntry } from "./FileListItem";

// ─── Types ────────────────────────────────────────────────────────────────────

export interface InspectorPanelProps {
  /** Project name shown in the header */
  projectName?: string;
  /** Number of skills/entries */
  skillCount?: number;
  /** File entries to display */
  files?: FileEntry[];
  /** Panel width in px */
  width?: number;
}

// ─── Component ────────────────────────────────────────────────────────────────

/**
 * InspectorPanel — right-side panel (240px default) with project header,
 * skills badge, sort row, and file tree.
 *
 * Uses `var(--inspector)` background and `var(--inspector-border)` for
 * warm sand panel differentiation from the cold white-qing sidebar.
 */
export default function InspectorPanel({
  projectName = "project_nginx",
  skillCount  = 19,
  files       = [],
  width       = 240,
}: InspectorPanelProps) {
  const [_sortDir, setSortDir] = useState<"asc" | "desc">("desc");

  return (
    <aside
      className="flex flex-col h-full shrink-0 overflow-hidden"
      style={{ width, background: "var(--inspector)", borderLeft: "1px solid var(--inspector-border)" }}
    >
      {/* ── Project header ── */}
      <div className="px-4 pt-4 pb-3 shrink-0 border-b border-border">
        <div className="flex items-center justify-between mb-2">
          <span className="text-[13px] font-semibold text-foreground/85 font-serif truncate flex-1 min-w-0 mr-2">
            {projectName}
          </span>
          <button className="w-6 h-6 flex items-center justify-center rounded text-muted-foreground hover:text-foreground hover:bg-accent transition-colors duration-100 shrink-0">
            <ChevronRight size={13} />
          </button>
        </div>
        <div className="flex items-center gap-2 flex-wrap">
          <button className="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-accent hover:bg-secondary border border-border/60 text-[11px] text-foreground/75 transition-colors duration-150">
            <FolderOpen size={11} className="text-jade" />
            打开文件夹
          </button>
          <button className="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-accent hover:bg-secondary border border-border/60 text-[11px] text-foreground/75 transition-colors duration-150">
            <Layers size={11} className="text-jade" />
            项目技能 {skillCount}
          </button>
        </div>
      </div>

      {/* ── Skills badge ── */}
      <div className="px-4 py-2.5 shrink-0 border-b border-border">
        <div className="flex items-center gap-1.5">
          <span
            className="px-2 py-0.5 rounded-md text-[11px] font-medium"
            style={{
              background: "var(--status-active-bg)",
              color: "var(--status-active)",
            }}
          >
            技能
          </span>
          <span className="text-[11px] font-semibold text-foreground/80">{skillCount}</span>
        </div>
      </div>

      {/* ── Sort row ── */}
      <div className="flex items-center justify-end px-4 py-2 shrink-0">
        <button
          onClick={() => setSortDir((d) => (d === "asc" ? "desc" : "asc"))}
          className="flex items-center gap-1 text-[11px] text-muted-foreground hover:text-foreground transition-colors duration-150"
        >
          <Clock size={11} />
          时间
        </button>
      </div>

      {/* ── File tree ── */}
      <div className="flex-1 overflow-y-auto scrollbar-thin px-3 pb-4">
        {files.map((file) => (
          <FileListItem key={file.id} item={file} />
        ))}
      </div>
    </aside>
  );
}
