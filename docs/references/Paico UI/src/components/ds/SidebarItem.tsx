import { ChevronRight } from "lucide-react";
import type { LucideIcon } from "lucide-react";

// ─── Types ────────────────────────────────────────────────────────────────────

export interface SidebarItemProps {
  /** Unique identifier */
  id?: string;
  /** Display label */
  label?: string;
  /** Lucide icon component */
  icon?: LucideIcon;
  /** Optional numeric badge */
  badge?: number;
  /** Is this item the active/selected page? */
  selected?: boolean;
  /** Is this item being hovered? (controlled externally for grouping) */
  hovered?: boolean;
  /** Click callback */
  onClick?: () => void;
  /** Hover enter callback */
  onHover?: (id: string) => void;
  /** Hover leave callback */
  onLeave?: () => void;
}

// ─── Component ────────────────────────────────────────────────────────────────
/*
 * Structure
 * ┌──────────────────────────────────────────────────────┐
 * │  [Icon]  Label Text            [Badge?]  [Chevron?]  │
 * └──────────────────────────────────────────────────────┘
 *
 * States
 * • default  : muted foreground, transparent background
 * • hover    : sidebar-accent background, sidebar-foreground text
 * • selected : sidebar-accent background, sidebar-primary text, chevron visible
 *
 * Padding / Spacing
 * • px-2.5 py-2   (10px / 8px)
 * • icon size 15px, gap-3 (12px) between icon and label
 * • badge: 16×16px circle, text-[10px]
 * • border-radius: rounded-md (var(--radius-md) = 8px)
 *
 * Color rules (all from CSS variables, no hardcodes)
 * • bg              : transparent → bg-sidebar-accent
 * • text (default)  : text-muted-foreground
 * • text (selected) : text-sidebar-primary
 * • icon (selected) : text-jade
 * • icon (default)  : opacity-60
 * • badge bg        : bg-jade
 * • badge text      : text-primary-foreground
 */
export default function SidebarItem({
  id        = "item",
  label     = "Nav Item",
  icon: Icon,
  badge,
  selected  = false,
  hovered   = false,
  onClick   = () => {},
  onHover   = () => {},
  onLeave   = () => {},
}: SidebarItemProps) {
  const isHighlighted = selected || hovered;

  return (
    <button
      data-cmp="SidebarItem"
      data-state={selected ? "selected" : hovered ? "hover" : "default"}
      onClick={onClick}
      onMouseEnter={() => onHover(id)}
      onMouseLeave={onLeave}
      className={[
        `w-full flex items-center gap-3 px-2.5 py-2 rounded-md text-[13px] font-sans font-medium`,
        `transition-all duration-150`,
        isHighlighted
          ? "bg-sidebar-accent text-sidebar-primary"
          : "text-muted-foreground",
      ].join(" ")}
    >
      {/* Icon */}
      {Icon && (
        <Icon
          size={15}
          className={[
            "shrink-0 transition-opacity duration-150",
            selected ? "text-jade" : "opacity-60",
          ].join(" ")}
        />
      )}

      {/* Label */}
      <span className="flex-1 text-left truncate">{label}</span>

      {/* Badge */}
      {badge != null && badge > 0 && (
        <span className="text-[10px] bg-jade text-primary-foreground rounded-full w-4 h-4 flex items-center justify-center font-semibold shrink-0">
          {badge}
        </span>
      )}

      {/* Chevron (selected only) */}
      {selected && (
        <ChevronRight size={12} className="text-jade opacity-60 shrink-0" />
      )}
    </button>
  );
}
