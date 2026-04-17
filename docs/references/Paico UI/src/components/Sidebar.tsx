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
  ChevronRight,
  Layers,
} from "lucide-react";

type NavItem = {
  id: string;
  label: string;
  icon: React.ComponentType<{ size?: number; className?: string }>;
  badge?: number;
};

const NAV_ITEMS: NavItem[] = [
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { id: "projects", label: "Projects", icon: FolderOpen, badge: 3 },
  { id: "moodboards", label: "Moodboards", icon: Image },
  { id: "materials", label: "Materials", icon: Package },
  { id: "proposals", label: "Proposals", icon: FileText, badge: 2 },
  { id: "timeline", label: "Timeline", icon: CalendarDays },
  { id: "clients", label: "Clients", icon: Users },
  { id: "files", label: "Files", icon: Files },
];

interface SidebarProps {
  activeSection?: string;
  onNavigate?: (id: string) => void;
}

export default function Sidebar({
  activeSection = "dashboard",
  onNavigate = () => {},
}: SidebarProps) {
  return (
    <aside
      data-cmp="Sidebar"
      className="flex flex-col h-screen w-[220px] shrink-0 bg-sidebar border-r border-sidebar-border"
      style={{ minHeight: "100vh" }}
    >
      {/* Logo */}
      <div className="px-5 pt-6 pb-5 border-b border-sidebar-border">
        <div className="flex items-center gap-2.5">
          <div className="w-7 h-7 rounded-md bg-terracotta flex items-center justify-center">
            <Layers size={14} className="text-primary-foreground" />
          </div>
          <div>
            <p className="text-sidebar-foreground font-serif text-[15px] font-semibold leading-none tracking-wide">
              ATELIER
            </p>
            <p className="text-muted-foreground text-[9px] tracking-widest uppercase mt-0.5">
              Studio OS
            </p>
          </div>
        </div>
      </div>

      {/* Nav */}
      <nav className="flex-1 overflow-y-auto scrollbar-thin px-3 py-4">
        <p className="text-muted-foreground text-[9px] tracking-widest uppercase px-2 mb-3 font-sans">
          Workspace
        </p>
        <ul className="flex flex-col gap-0.5">
          {NAV_ITEMS.map((item) => {
            const Icon = item.icon;
            const isActive = activeSection === item.id;
            return (
              <li key={item.id}>
                <button
                  onClick={() => onNavigate(item.id)}
                  className={`w-full flex items-center gap-3 px-2.5 py-2 rounded-md text-[13px] font-sans font-medium transition-all duration-150 ${
                    isActive
                      ? "bg-sidebar-accent text-sidebar-primary text-primary-foreground"
                      : "text-muted-foreground hover:bg-sidebar-accent hover:text-sidebar-foreground"
                  }`}
                >
                  <Icon
                    size={15}
                    className={isActive ? "text-gold" : "opacity-60"}
                  />
                  <span className="flex-1 text-left">{item.label}</span>
                  {item.badge ? (
                    <span className="text-[10px] bg-terracotta text-primary-foreground rounded-full w-4 h-4 flex items-center justify-center font-semibold">
                      {item.badge}
                    </span>
                  ) : null}
                  {isActive && (
                    <ChevronRight size={12} className="text-gold opacity-60" />
                  )}
                </button>
              </li>
            );
          })}
        </ul>

        <div className="border-t border-sidebar-border mt-4 pt-4">
          <p className="text-muted-foreground text-[9px] tracking-widest uppercase px-2 mb-3 font-sans">
            Studio
          </p>
          <p className="text-muted-foreground text-[11px] px-2 leading-snug font-sans">
            Whitmore Interiors
          </p>
          <p className="text-muted-foreground text-[10px] px-2 mt-0.5 font-sans opacity-60">
            Studio Plan · 12 seats
          </p>
        </div>
      </nav>

      {/* Bottom */}
      <div className="border-t border-sidebar-border px-3 py-4">
        <button className="w-full flex items-center gap-3 px-2.5 py-2 rounded-md text-muted-foreground hover:bg-sidebar-accent hover:text-sidebar-foreground text-[13px] font-sans transition-all">
          <Bell size={15} className="opacity-60" />
          <span className="flex-1 text-left">Notifications</span>
          <span className="text-[10px] bg-gold text-charcoal rounded-full w-4 h-4 flex items-center justify-center font-semibold">
            5
          </span>
        </button>
        <button className="w-full flex items-center gap-3 px-2.5 py-2 rounded-md text-muted-foreground hover:bg-sidebar-accent hover:text-sidebar-foreground text-[13px] font-sans transition-all">
          <Settings size={15} className="opacity-60" />
          <span className="flex-1 text-left">Settings</span>
        </button>

        {/* User */}
        <div className="flex items-center gap-2.5 px-2.5 py-2.5 mt-2 rounded-md bg-sidebar-accent">
          <div className="w-7 h-7 rounded-full bg-terracotta-light flex items-center justify-center text-terracotta text-[11px] font-semibold shrink-0">
            SC
          </div>
          <div className="flex-1 min-w-0">
            <p className="text-sidebar-foreground text-[12px] font-semibold truncate">
              Sara Chen
            </p>
            <p className="text-muted-foreground text-[10px] truncate opacity-70">
              Lead Designer
            </p>
          </div>
        </div>
      </div>
    </aside>
  );
}
