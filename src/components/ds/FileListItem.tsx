import { useState } from "react";
import { ChevronRight, Folder, File, FileCode, FileImage, FileText } from "lucide-react";

// ─── Types ────────────────────────────────────────────────────────────────────

export interface FileEntry {
  id: string;
  name: string;
  type: "folder" | "file";
  ext?: string;
  children?: FileEntry[];
  expanded?: boolean;
}

export interface FileListItemProps {
  /** The file/folder entry to render */
  item?: FileEntry;
  /** Indentation depth (internal use for recursion) */
  depth?: number;
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

function ExtIcon({ ext }: { ext?: string }) {
  const cls = "shrink-0 text-muted-foreground";
  if (ext === "md" || ext === "txt") return <FileText size={13} className={cls} />;
  if (ext === "js" || ext === "ts" || ext === "tsx" || ext === "jsx" || ext === "css" || ext === "html")
    return <FileCode size={13} className={cls} />;
  if (ext === "png" || ext === "jpg" || ext === "jpeg" || ext === "svg" || ext === "icon")
    return <FileImage size={13} className={cls} />;
  return <File size={13} className={cls} />;
}

// ─── Component ────────────────────────────────────────────────────────────────

/**
 * FileListItem — single file or folder entry in a tree view.
 *
 * Supports recursive folder expansion with indentation.
 * Uses `text-jade` for folder icons, `bg-accent` on hover.
 */
export default function FileListItem({
  item = { id: "f0", name: "example.ts", type: "file", ext: "ts" },
  depth = 0,
}: FileListItemProps) {
  const [expanded, setExpanded] = useState(item.expanded ?? false);
  const isFolder = item.type === "folder";

  return (
    <div>
      <div
        className={`flex items-center gap-1.5 px-2 py-1 rounded-md cursor-pointer group hover:bg-accent transition-colors duration-100 ${
          isFolder ? "text-foreground/80" : "text-foreground/70"
        }`}
        style={{ paddingLeft: `${8 + depth * 12}px` }}
        onClick={() => {
          if (isFolder) {
            setExpanded(!expanded);
          }
        }}
      >
        {/* Chevron or spacer */}
        {isFolder ? (
          <ChevronRight
            size={12}
            className={`text-muted-foreground transition-transform duration-150 shrink-0 ${expanded ? "rotate-90" : ""}`}
          />
        ) : (
          <span className="w-3 shrink-0" />
        )}

        {/* Type icon */}
        {isFolder ? (
          <Folder size={13} className="text-jade shrink-0" />
        ) : (
          <ExtIcon ext={item.ext} />
        )}

        {/* Name */}
        <span className="text-[11px] truncate leading-normal">{item.name}</span>
      </div>

      {/* Children (recursive) */}
      {isFolder && expanded && item.children && item.children.length > 0 && (
        <div>
          {item.children.map((child) => (
            <FileListItem key={child.id} item={child} depth={depth + 1} />
          ))}
        </div>
      )}
    </div>
  );
}
