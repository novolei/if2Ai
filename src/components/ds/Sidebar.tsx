import { useState } from "react";
import {
  LayoutDashboard,
  FolderOpen,
  Image,
  Package,
  FileText,
  CalendarDays,
  Users,
  Files,
  Settings,
  Bell,
  Layers,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import SidebarItem from "./SidebarItem";

// ─── Types ────────────────────────────────────────────────────────────────────

export interface NavItemDef {
  id: string;
  label: string;
  icon: LucideIcon;
  badge?: number;
}

export interface SidebarProps {
  /** Currently active section id */
  activeSection?: string;
  /** Callback when user clicks a nav item */
  onNavigate?: (id: string) => void;
  /** Studio name */
  studioName?: string;
  /** Studio plan */
  studioPlan?: string;
  /** User display name */
  userName?: string;
  /** User role */
  userRole?: string;
  /** User avatar initials */
  userInitials?: string;
  /** Notification count */
  notificationCount?: number;
}

// ─── Default nav items ────────────────────────────────────────────────────────

const DEFAULT_NAV_ITEMS: NavItemDef[] = [
  { id: "dashboard",  label: "Dashboard",  icon: LayoutDashboard },
  { id: "projects",   label: "Projects",   icon: FolderOpen, badge: 3 },
  { id: "moodboards", label: "Moodboards", icon: Image },
  { id: "materials",  label: "Materials",  icon: Package },
  { id: "proposals",  label: "Proposals",  icon: FileText, badge: 2 },
  { id: "timeline",   label: "Timeline",   icon: CalendarDays },
  { id: "clients",    label: "Clients",    icon: Users },
  { id: "files",      label: "Files",      icon: Files },
];

// ─── Component ────────────────────────────────────────────────────────────────

/**
 * Sidebar — full 220px left sidebar with logo, nav, and user card.
 *
 * Uses Paico sidebar CSS variables: bg-sidebar, border-sidebar-border,
 * sidebar-accent, sidebar-foreground, etc.
 */
export default function Sidebar({
  activeSection     = "dashboard",
  onNavigate        = () => {},
  studioName        = "Whitmore Interiors",
  studioPlan        = `Studio Plan · 12 seats`,
  userName          = "Sara Chen",
  userRole          = "Lead Designer",
  userInitials      = "SC",
  notificationCount = 5,
}: SidebarProps) {
  const [hoveredId, setHoveredId] = useState<string | null>(null);

  return (
    <aside
      className="flex flex-col h-screen w-[220px] shrink-0 bg-sidebar border-r border-sidebar-border"
    >
      {/* ── Logo ── */}
      <div className="px-5 pt-6 pb-5 border-b border-sidebar-border">
        <div className="flex items-center gap-2.5">
          <div className="w-7 h-7 rounded-md bg-jade flex items-center justify-center shrink-0">
            <Layers size={14} className="text-primary-foreground" />
          </div>
          <div>
            <p className="text-sidebar-foreground font-serif text-[15px] font-semibold leading-none tracking-wide">
              ATELIER
            </p>
            <p className="text-muted-foreground text-[9px] tracking-widest uppercase mt-0.5 font-sans">
              Studio OS
            </p>
          </div>
        </div>
      </div>

      {/* ── Nav ── */}
      <nav className="flex-1 overflow-y-auto scrollbar-thin px-3 py-4">
        <p className="text-muted-foreground text-[9px] tracking-widest uppercase px-2 mb-3 font-sans">
          Workspace
        </p>
        <ul className="flex flex-col gap-0.5">
          {DEFAULT_NAV_ITEMS.map((item) => (
            <li key={item.id}>
              <SidebarItem
                id={item.id}
                label={item.label}
                icon={item.icon}
                badge={item.badge}
                selected={activeSection === item.id}
                hovered={hoveredId === item.id}
                onHover={(id) => setHoveredId(id)}
                onLeave={() => setHoveredId(null)}
                onClick={() => onNavigate(item.id)}
              />
            </li>
          ))}
        </ul>

        {/* Studio block */}
        <div className="border-t border-sidebar-border mt-4 pt-4">
          <p className="text-muted-foreground text-[9px] tracking-widest uppercase px-2 mb-3 font-sans">
            Studio
          </p>
          <p className="text-muted-foreground text-[11px] px-2 leading-snug font-sans">
            {studioName}
          </p>
          <p className="text-muted-foreground text-[10px] px-2 mt-0.5 font-sans opacity-60">
            {studioPlan}
          </p>
        </div>
      </nav>

      {/* ── Bottom ── */}
      <div className="border-t border-sidebar-border px-3 py-4">
        {/* Notifications */}
        <button className="w-full flex items-center gap-3 px-2.5 py-2 rounded-md text-muted-foreground hover:bg-sidebar-accent hover:text-sidebar-foreground text-[13px] font-sans transition-all duration-150 group">
          <Bell size={15} className="opacity-60 group-hover:opacity-100 transition-opacity" />
          <span className="flex-1 text-left">Notifications</span>
          {notificationCount > 0 && (
            <span className="text-[10px] bg-jade text-primary-foreground rounded-full w-4 h-4 flex items-center justify-center font-semibold shrink-0">
              {notificationCount}
            </span>
          )}
        </button>

        {/* Settings */}
        <button className="w-full flex items-center gap-3 px-2.5 py-2 rounded-md text-muted-foreground hover:bg-sidebar-accent hover:text-sidebar-foreground text-[13px] font-sans transition-all duration-150 group">
          <Settings size={15} className="opacity-60 group-hover:opacity-100 transition-opacity" />
          <span className="flex-1 text-left">Settings</span>
        </button>

        {/* User card */}
        <div className="flex items-center gap-2.5 px-2.5 py-2.5 mt-2 rounded-md bg-sidebar-accent">
          <div className="w-7 h-7 rounded-full bg-celadon-light flex items-center justify-center text-jade text-[11px] font-semibold shrink-0">
            {userInitials}
          </div>
          <div className="flex-1 min-w-0">
            <p className="text-sidebar-foreground text-[12px] font-semibold truncate">{userName}</p>
            <p className="text-muted-foreground text-[10px] truncate opacity-70">{userRole}</p>
          </div>
        </div>
      </div>
    </aside>
  );
}
