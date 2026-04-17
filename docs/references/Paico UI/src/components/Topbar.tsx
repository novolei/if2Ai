import { Search, Plus, SlidersHorizontal, Bell, HelpCircle } from "lucide-react";

interface TopbarProps {
  title?: string;
  subtitle?: string;
  onNewProject?: () => void;
}

export default function Topbar({
  title = "Studio Dashboard",
  subtitle = "Thursday, 12 June 2025",
  onNewProject = () => {},
}: TopbarProps) {
  return (
    <header
      data-cmp="Topbar"
      className="h-14 shrink-0 bg-surface border-b border-border flex items-center px-6 gap-4"
    >
      {/* Title */}
      <div className="flex-1 min-w-0">
        <h1 className="font-serif text-[17px] font-semibold text-foreground leading-tight truncate">
          {title}
        </h1>
        <p className="text-muted-foreground text-[11px] font-sans leading-none mt-0.5">
          {subtitle}
        </p>
      </div>

      {/* Search */}
      <div className="flex items-center gap-2 bg-muted border border-border rounded-lg px-3 py-1.5 w-64">
        <Search size={13} className="text-muted-foreground shrink-0" />
        <input
          type="text"
          placeholder="Search projects, clients…"
          className="bg-transparent text-[12px] font-sans text-foreground placeholder:text-muted-foreground flex-1 outline-none min-w-0"
        />
        <span className="text-muted-foreground text-[10px] bg-accent border border-border rounded px-1 py-0.5 leading-none shrink-0">
          ⌘K
        </span>
      </div>

      {/* Actions */}
      <div className="flex items-center gap-2">
        <button className="w-8 h-8 rounded-md flex items-center justify-center text-muted-foreground hover:bg-accent hover:text-foreground transition-all">
          <SlidersHorizontal size={14} />
        </button>
        <button className="w-8 h-8 rounded-md flex items-center justify-center text-muted-foreground hover:bg-accent hover:text-foreground transition-all relative">
          <Bell size={14} />
          <span className="absolute top-1.5 right-1.5 w-1.5 h-1.5 bg-terracotta rounded-full" />
        </button>
        <button className="w-8 h-8 rounded-md flex items-center justify-center text-muted-foreground hover:bg-accent hover:text-foreground transition-all">
          <HelpCircle size={14} />
        </button>
        <div className="w-px h-5 bg-border mx-1" />
        <button
          onClick={onNewProject}
          className="flex items-center gap-1.5 bg-charcoal text-primary-foreground px-3 py-1.5 rounded-lg text-[12px] font-sans font-medium hover:opacity-90 transition-all"
        >
          <Plus size={13} />
          New Project
        </button>
      </div>
    </header>
  );
}
