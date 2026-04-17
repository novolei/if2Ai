import { useState, useRef, useCallback, useEffect } from "react";
import type { LucideProps } from "lucide-react";
import type { ComponentType } from "react";
import {
  Search,
  Plus,
  ChevronRight,
  ChevronDown,
  Folder,
  File,
  Settings,
  PlusCircle,
  MessageSquare,
  Cpu,
  Mic,
  Send,
  MoreHorizontal,
  Play,
  AlignJustify,
  Code2,
  Globe,
  Sliders,
  FolderOpen,
  GitBranch,
  Clock,
  Zap,
  ArrowUpDown,
  SlidersHorizontal,
  Layers,
  ChevronLeft,
  PanelLeftClose,
  PanelLeftOpen,
  PanelRightClose,
  PanelRightOpen,
} from "lucide-react";

// ─── Types ───────────────────────────────────────────────────────────────────

interface Session {
  id: string;
  title: string;
  tokens: number;
  time: string;
  active?: boolean;
}

interface SessionGroup {
  id: string;
  name: string;
  sessions: Session[];
  expanded?: boolean;
}

interface FileItem {
  id: string;
  name: string;
  type: "folder" | "file";
  ext?: string;
  children?: FileItem[];
  expanded?: boolean;
}

interface ChatMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
  time: string;
  thinking?: string;
  thinkingExpanded?: boolean;
}

// ─── Mock Data ───────────────────────────────────────────────────────────────

const INITIAL_GROUPS: SessionGroup[] = [
  {
    id: "g1",
    name: "2222334",
    expanded: true,
    sessions: [
      { id: "s1", title: "/skills", tokens: 493, time: "16小时前" },
      { id: "s2", title: "/steve jobs perspec...", tokens: 98, time: "9分钟前", active: true },
      { id: "s3", title: "新对话", tokens: 26, time: "11小时前" },
      { id: "s4", title: "新对话", tokens: 35, time: "11小时前" },
      { id: "s5", title: "看看明天天气", tokens: 144, time: "11小时前" },
      { id: "s6", title: "分步使用TODOwrite...", tokens: 382, time: "11小时前" },
      { id: "s7", title: "/steve jobs perspec...", tokens: 28, time: "12小时前" },
      { id: "s8", title: "创建技能", tokens: 689, time: "12小时前" },
      { id: "s9", title: "/steve jobs perspec...", tokens: 56, time: "12小时前" },
      { id: "s10", title: "Skills Market install", tokens: 71, time: "13小时前" },
      { id: "s11", title: "/steve jobs perspec...", tokens: 21, time: "13小时前" },
    ],
  },
];

const MESSAGES: ChatMessage[] = [
  {
    id: "m1",
    role: "assistant",
    content: `你好啊！👋\n\n有什么我可以帮你的吗？无论是写代码、查资料、分析数据，还是想继续听夹话、用各种视角看问题，随时告诉我！`,
    time: "18:12",
    thinking: `I don't need to use any tools here, just a standard conversational response.\n\nI can mention that I'm ready to help with whatever they need - coding, information, skills, etc.\n\nKeep it brief and open-ended.`,
    thinkingExpanded: false,
  },
  {
    id: "m2",
    role: "user",
    content: "你好",
    time: "18:12",
  },
  {
    id: "m3",
    role: "assistant",
    content: `你好！有什么想聊的或者需要帮忙的吗？随时吩咐。😊`,
    time: "18:12",
    thinking: `The user is saying "你好" (Hello) again.\n\nI should respond in a friendly and helpful manner, keeping it brief since this is a continuation of a conversation where I've already asked how I can help.\n\nI will simply greet them back and offer assistance.\n\nPlan:\n1. Greet the user warmly.\n2. Reiterate readiness to help with any task.\n3. Keep it short.\n\nNo tools needed.`,
    thinkingExpanded: false,
  },
];

const FILES: FileItem[] = [
  { id: "f1", name: "AI3", type: "folder" },
  { id: "f2", name: "bubble-shooter", type: "folder" },
  { id: "f3", name: "bubble_shooter", type: "folder" },
  { id: "f4", name: "ai_dreamer", type: "folder" },
  { id: "f5", name: "test", type: "folder" },
  { id: "f6", name: "AI2", type: "folder" },
  { id: "f7", name: "ai", type: "folder" },
  { id: "f8", name: "Untitled.icon", type: "file", ext: "icon" },
  { id: "f9", name: "hh.md", type: "file", ext: "md" },
  { id: "f10", name: "style.css", type: "file", ext: "css" },
  { id: "f11", name: "README.md", type: "file", ext: "md" },
  { id: "f12", name: "index.html", type: "file", ext: "html" },
  { id: "f13", name: "game.js", type: "file", ext: "js" },
  { id: "f14", name: "iF2 AI智能体logo设计.png", type: "file", ext: "png" },
  { id: "f15", name: "if_2_ai_动态视觉系统规范.md", type: "file", ext: "md" },
  { id: "f16", name: "Icon-iOS-Default-1024x1024@1x.png", type: "file", ext: "png" },
  { id: "f17", name: "RUST_CODE_STYLE.md", type: "file", ext: "md" },
  { id: "f18", name: "logo2.png", type: "file", ext: "png" },
  { id: "f19", name: "logo-if2.png", type: "file", ext: "png" },
];

// ─── Sub-components ───────────────────────────────────────────────────────────

function NavIcon({
  icon: Icon,
  active = false,
  onClick = () => {},
  tooltip = "",
}: {
  icon: ComponentType<LucideProps>;
  active?: boolean;
  onClick?: () => void;
  tooltip?: string;
}) {
  return (
    <button
      title={tooltip}
      onClick={onClick}
      className={`w-8 h-8 flex items-center justify-center rounded-lg transition-all duration-150 ${
        active
          ? "bg-sidebar-accent text-sidebar-primary"
          : "text-sidebar-foreground/50 hover:text-sidebar-foreground hover:bg-sidebar-accent/60"
      }`}
    >
      <Icon size={16} />
    </button>
  );
}

function FileRow({ item }: { item: FileItem }) {
  const [expanded, setExpanded] = useState(item.expanded ?? false);

  return (
    <div>
      <div
        className={`flex items-center gap-1.5 px-2 py-1 rounded-md cursor-pointer group hover:bg-accent transition-colors duration-100 ${
          item.type === "folder" ? "text-foreground/80" : "text-foreground/70"
        }`}
        onClick={() => item.type === "folder" && setExpanded(!expanded)}
      >
        {item.type === "folder" ? (
          <>
            <ChevronRight
              size={12}
              className={`text-muted-foreground transition-transform duration-150 shrink-0 ${
                expanded ? "rotate-90" : ""
              }`}
            />
            <Folder size={13} className="text-jade shrink-0" />
          </>
        ) : (
          <>
            <span className="w-3 shrink-0" />
            <File size={13} className="text-muted-foreground shrink-0" />
          </>
        )}
        <span className="text-token-xs truncate">{item.name}</span>
      </div>
      {item.type === "folder" && expanded && item.children && (
        <div className="pl-4">
          {item.children.map((child) => (
            <FileRow key={child.id} item={child} />
          ))}
        </div>
      )}
    </div>
  );
}

function ThinkingBlock({ text }: { text: string }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="mb-3">
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-1.5 text-token-xs text-muted-foreground hover:text-foreground/70 transition-colors duration-150 mb-1"
      >
        <span className="w-1 h-1 rounded-full bg-status-active shrink-0" />
        <span>已完成思考</span>
        <ChevronDown
          size={11}
          className={`transition-transform duration-150 ${open ? "rotate-180" : ""}`}
        />
      </button>
      {open && (
        <div className="border-l-2 border-border pl-3 ml-1 py-1">
          {text.split("\n").map((line, i) => (
            <p
              key={i}
              className={`text-token-xs text-muted-foreground leading-relaxed italic ${
                line === "" ? "h-2" : ""
              }`}
            >
              {line}
            </p>
          ))}
        </div>
      )}
    </div>
  );
}

// ─── Drag Divider ─────────────────────────────────────────────────────────────

function DragDivider({
  onDrag,
  side = "left",
}: {
  onDrag: (delta: number) => void;
  side?: "left" | "right";
}) {
  const dragging = useRef(false);
  const lastX = useRef(0);

  const onMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      dragging.current = true;
      lastX.current = e.clientX;

      const onMove = (ev: MouseEvent) => {
        if (!dragging.current) return;
        const delta = ev.clientX - lastX.current;
        lastX.current = ev.clientX;
        onDrag(delta);
      };
      const onUp = () => {
        dragging.current = false;
        document.removeEventListener("mousemove", onMove);
        document.removeEventListener("mouseup", onUp);
        document.body.style.cursor = "";
        document.body.style.userSelect = "";
      };

      document.addEventListener("mousemove", onMove);
      document.addEventListener("mouseup", onUp);
      document.body.style.cursor = "col-resize";
      document.body.style.userSelect = "none";
    },
    [onDrag]
  );

  return (
    /* Outer track — transparent, wider hit area */
    <div
      onMouseDown={onMouseDown}
      className="relative shrink-0 flex items-center justify-center cursor-col-resize group"
      style={{ width: 12, zIndex: 10 }}
    >
      {/* Visual line with rounded caps */}
      <div
        className="absolute inset-y-6 w-px transition-all duration-150 group-hover:w-[3px] group-active:w-[3px]"
        style={{
          left: "50%",
          transform: "translateX(-50%)",
          background: "var(--border)",
          borderRadius: 9999,
          /* Subtle elevation so it reads as a raised seam */
          boxShadow: side === "left"
            ? `1px 0 4px var(--shadow-color), -1px 0 2px var(--shadow-color)`
            : `-1px 0 4px var(--shadow-color), 1px 0 2px var(--shadow-color)`,
          opacity: 0.85,
        }}
      />
      {/* Hover highlight overlay dot in center */}
      <div
        className="absolute w-1 h-5 rounded-full opacity-0 group-hover:opacity-100 transition-all duration-150"
        style={{
          background: "var(--jade)",
          left: "50%",
          top: "50%",
          transform: "translate(-50%, -50%)",
        }}
      />
    </div>
  );
}

// ─── Main Page ────────────────────────────────────────────────────────────────

const RAIL_WIDTH = 48;
const SIDEBAR_MIN = 160;
const SIDEBAR_MAX = 380;
const INSPECTOR_MIN = 180;
const INSPECTOR_MAX = 420;
const CHAT_CONTENT_MAX = 672; // max readable width for chat content
const CHAT_CONTENT_MIN = 320;
const CHAT_CONTENT_PADDING = 48; // px-6 * 2

export default function ChatWorkspace() {
  const [groups, setGroups] = useState<SessionGroup[]>(INITIAL_GROUPS);
  const [activeSession, setActiveSession] = useState("s2");
  const [inputValue, setInputValue] = useState("");

  // Panel states
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [inspectorCollapsed, setInspectorCollapsed] = useState(false);
  const [sidebarWidth, setSidebarWidth] = useState(212); // session list width (excluding rail)
  const [inspectorWidth, setInspectorWidth] = useState(240);

  // Track main panel width for dynamic chat content sizing
  const mainRef = useRef<HTMLElement>(null);
  const [mainWidth, setMainWidth] = useState(0);

  useEffect(() => {
    const el = mainRef.current;
    if (!el) return;
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const w = entry.contentRect.width;
        setMainWidth(w);
        console.log("ChatWorkspace main panel width:", w);
      }
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Compute the content max-width: fill the available space minus padding,
  // but never exceed CHAT_CONTENT_MAX or shrink below CHAT_CONTENT_MIN
  const dynamicContentWidth = mainWidth > 0
    ? Math.min(CHAT_CONTENT_MAX, Math.max(CHAT_CONTENT_MIN, mainWidth - CHAT_CONTENT_PADDING))
    : CHAT_CONTENT_MAX;

  const toggleGroup = (gid: string) => {
    setGroups((prev) =>
      prev.map((g) => (g.id === gid ? { ...g, expanded: !g.expanded } : g))
    );
  };

  // Drag handlers
  const handleSidebarDrag = useCallback((delta: number) => {
    setSidebarWidth((w) =>
      Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, w + delta))
    );
  }, []);

  const handleInspectorDrag = useCallback((delta: number) => {
    setInspectorWidth((w) =>
      Math.min(INSPECTOR_MAX, Math.max(INSPECTOR_MIN, w - delta))
    );
  }, []);

  console.log("ChatWorkspace render, activeSession:", activeSession, "sidebarCollapsed:", sidebarCollapsed, "inspectorCollapsed:", inspectorCollapsed, "mainWidth:", mainWidth, "dynamicContentWidth:", dynamicContentWidth);

  // Total left panel width
  const leftPanelWidth = sidebarCollapsed
    ? RAIL_WIDTH
    : RAIL_WIDTH + sidebarWidth;

  return (
    <div
      data-cmp="ChatWorkspace"
      className="flex h-screen w-full overflow-hidden bg-background"
      style={{ fontFamily: "var(--fontSans)" }}
    >
      {/* ══════════════════════════════════════════════════════════
          LEFT PANEL  (nav rail + session list)
          ══════════════════════════════════════════════════════════ */}
      <aside
        className="flex h-full shrink-0 transition-all duration-300"
        style={{ width: leftPanelWidth }}
      >
        {/* ── Nav icon rail ── */}
        <div
          className="flex flex-col items-center py-4 gap-3 shrink-0 relative"
          style={{
            width: RAIL_WIDTH,
            background: "var(--sidebar)",
            /* subtle right-edge inner shadow for depth */
            boxShadow: "inset -1px 0 0 var(--sidebar-border)",
          }}
        >
          {/* Logo */}
          <div className="w-8 h-8 rounded-lg bg-sidebar-accent flex items-center justify-center mb-1 shrink-0">
            <Zap size={14} className="text-sidebar-foreground" />
          </div>

          <div className="flex flex-col gap-1 items-center">
            <NavIcon icon={PlusCircle} tooltip="新线程" />
            <NavIcon icon={Search} tooltip="搜索" />
            <NavIcon icon={Layers} active tooltip="线程列表" />
          </div>

          <div className="flex-1" />

          <div className="flex flex-col gap-1 items-center">
            <NavIcon icon={PlusCircle} tooltip="新建" />
            <NavIcon icon={Settings} tooltip="设置" />
          </div>
        </div>

        {/* ── Session list panel — hidden when collapsed ── */}
        <div
          className="flex flex-col overflow-hidden transition-all duration-300"
          style={{
            width: sidebarCollapsed ? 0 : sidebarWidth,
            opacity: sidebarCollapsed ? 0 : 1,
            background: "var(--sidebar)",
          }}
        >
          {/* Header */}
          <div className="flex items-center justify-between px-4 pt-4 pb-3 shrink-0">
            <span
              className="font-semibold text-sidebar-foreground"
              style={{ fontSize: "var(--text-sm)", letterSpacing: "var(--ls-wide)" }}
            >
              Chat
            </span>
            <button className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-jade text-primary-foreground text-token-xs font-medium transition-all duration-150 hover:opacity-90 active:scale-95">
              <Plus size={12} />
              新线程
            </button>
          </div>

          {/* Quick nav */}
          <div className="px-3 pb-2 flex flex-col gap-0.5 shrink-0">
            {[
              { icon: PlusCircle, label: "新线程" },
              { icon: Search, label: "Search" },
              { icon: Layers, label: "/skills" },
            ].map(({ icon: Icon, label }) => (
              <button
                key={label}
                className="flex items-center gap-2.5 px-2 py-1.5 rounded-md text-sidebar-foreground/60 hover:text-sidebar-foreground hover:bg-sidebar-accent/50 text-token-xs transition-colors duration-100"
              >
                <Icon size={13} className="shrink-0" />
                {label}
              </button>
            ))}
          </div>

          <div className="mx-3 mb-2 border-t border-sidebar-border" />

          {/* Session groups */}
          <div className="flex-1 overflow-y-auto scrollbar-thin px-3 pb-4">
            <div className="flex items-center justify-between mb-1 px-1">
              <span
                className="text-sidebar-foreground/60"
                style={{ fontSize: "var(--text-xs)", letterSpacing: "var(--ls-wider)", textTransform: "uppercase" }}
              >
                线程
              </span>
              <div className="flex items-center gap-0.5">
                {[ArrowUpDown, SlidersHorizontal, Layers].map((Icon, i) => (
                  <button
                    key={i}
                    className="w-5 h-5 flex items-center justify-center text-sidebar-foreground/50 hover:text-sidebar-foreground/80 rounded transition-colors duration-100"
                  >
                    <Icon size={12} />
                  </button>
                ))}
              </div>
            </div>

            {groups.map((group) => (
              <div key={group.id} className="mb-1">
                <button
                  onClick={() => toggleGroup(group.id)}
                  className="flex items-center gap-1.5 w-full px-2 py-1.5 rounded-md hover:bg-sidebar-accent/40 transition-colors duration-100"
                >
                  {group.expanded ? (
                    <ChevronDown size={12} className="text-sidebar-foreground/60 shrink-0" />
                  ) : (
                    <ChevronRight size={12} className="text-sidebar-foreground/60 shrink-0" />
                  )}
                  <Folder size={13} className="text-jade/90 shrink-0" />
                  <span className="text-sidebar-foreground/90 text-token-xs font-medium">
                    {group.name}
                  </span>
                </button>

                {group.expanded && (
                  <div className="ml-1 flex flex-col gap-0.5">
                    {group.sessions.map((session) => (
                      <button
                        key={session.id}
                        onClick={() => setActiveSession(session.id)}
                        className={`flex items-center gap-2 w-full px-2 py-1.5 rounded-md text-left transition-all duration-150 group ${
                          session.active || session.id === activeSession
                            ? "bg-sidebar-accent text-sidebar-foreground"
                            : "text-sidebar-foreground/70 hover:text-sidebar-foreground/100 hover:bg-sidebar-accent/40"
                        }`}
                      >
                        <MessageSquare size={12} className="shrink-0 opacity-70" />
                        <span className="flex-1 truncate text-token-xs">{session.title}</span>
                        <span className="text-sidebar-foreground/50 shrink-0" style={{ fontSize: "var(--text-xs)" }}>
                          {session.tokens}
                        </span>
                        <span className="text-sidebar-foreground/45 shrink-0 hidden group-hover:block" style={{ fontSize: "var(--text-xs)" }}>
                          {session.time}
                        </span>
                      </button>
                    ))}
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      </aside>

      {/* ── Sidebar drag divider ── */}
      {!sidebarCollapsed && (
        <DragDivider onDrag={handleSidebarDrag} side="left" />
      )}

      {/* ══════════════════════════════════════════════════════════
          CENTER PANEL  (main chat content)
          ══════════════════════════════════════════════════════════ */}
      <main className="flex flex-col flex-1 overflow-hidden bg-background min-w-0" ref={mainRef}>
        {/* ── Top toolbar ── */}
        <header
          className="flex items-center justify-between px-4 border-b border-border shrink-0"
          style={{ height: 48 }}
        >
          {/* Left group: collapse button + title */}
          <div className="flex items-center gap-2">
            {/* Sidebar toggle */}
            <button
              onClick={() => setSidebarCollapsed((v) => !v)}
              title={sidebarCollapsed ? "展开侧边栏" : "折叠侧边栏"}
              className="w-7 h-7 flex items-center justify-center rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors duration-150"
            >
              {sidebarCollapsed ? (
                <PanelLeftOpen size={15} />
              ) : (
                <PanelLeftClose size={15} />
              )}
            </button>

            <div className="h-4 w-px bg-border mx-0.5" />

            <button className="w-7 h-7 flex items-center justify-center rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors duration-150">
              <ChevronLeft size={15} />
            </button>

            <div className="flex items-center gap-2">
              <MessageSquare size={14} className="text-jade" />
              <span className="text-token-sm text-foreground/80 font-medium truncate max-w-xs">
                /steve jobs perspective 说明一下对当 if2Ai
              </span>
              <button className="text-muted-foreground hover:text-foreground transition-colors duration-150">
                <MoreHorizontal size={15} />
              </button>
            </div>
          </div>

          {/* Right group */}
          <div className="flex items-center gap-1.5">
            <button className="w-7 h-7 flex items-center justify-center rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors duration-100">
              <Play size={13} />
            </button>
            <div className="h-4 w-px bg-border mx-0.5" />
            {[
              { label: "字体" },
              { label: "Aa" },
              { label: "Aa" },
              { label: "密度" },
            ].map(({ label }, i) => (
              <button
                key={i}
                className="px-2 py-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors duration-100"
                style={{ fontSize: "var(--text-xs)" }}
              >
                {label}
              </button>
            ))}
            <div className="h-4 w-px bg-border mx-0.5" />
            <button className="w-7 h-7 flex items-center justify-center rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors duration-100">
              <Code2 size={14} />
            </button>
            <div className="h-4 w-px bg-border mx-0.5" />
            {/* Model selector */}
            <button className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-accent hover:bg-secondary transition-colors duration-150">
              <Cpu size={12} className="text-jade" />
              <span className="text-token-xs text-foreground/80 font-medium">Gpt 5.4 Mini</span>
              <ChevronDown size={11} className="text-muted-foreground" />
            </button>
            <div className="h-4 w-px bg-border mx-0.5" />
            <button className="w-7 h-7 flex items-center justify-center rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors duration-100">
              <AlignJustify size={14} />
            </button>
            {/* Submit */}
            <button className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-jade text-primary-foreground text-token-xs font-medium hover:opacity-90 active:scale-95 transition-all duration-150">
              提交
              <ChevronDown size={11} />
            </button>
            <button className="w-7 h-7 flex items-center justify-center rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors duration-100">
              <Sliders size={13} />
            </button>

            <div className="h-4 w-px bg-border mx-0.5" />

            {/* Inspector toggle */}
            <button
              onClick={() => setInspectorCollapsed((v) => !v)}
              title={inspectorCollapsed ? "展开检查器" : "折叠检查器"}
              className="w-7 h-7 flex items-center justify-center rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors duration-150"
            >
              {inspectorCollapsed ? (
                <PanelRightOpen size={15} />
              ) : (
                <PanelRightClose size={15} />
              )}
            </button>
          </div>
        </header>

        {/* ── Chat messages ── */}
        <div className="flex-1 overflow-y-auto scrollbar-thin px-6 py-5">
          <div className="mx-auto flex flex-col gap-6 transition-all duration-300" style={{ maxWidth: dynamicContentWidth }}>
            {MESSAGES.map((msg) => (
              <div key={msg.id} className={`flex flex-col ${msg.role === "user" ? "items-end" : "items-start"}`}>
                {msg.role === "assistant" ? (
                  <div className="w-full">
                    {msg.thinking && <ThinkingBlock text={msg.thinking} />}
                    <div className="text-foreground/85 leading-relaxed" style={{ fontSize: "var(--text-sm)", lineHeight: "var(--lh-relaxed)" }}>
                      {msg.content.split("\n").map((line, i) => (
                        <p key={i} className={line === "" ? "h-3" : ""}>
                          {line}
                        </p>
                      ))}
                    </div>
                    <div className="flex items-center gap-2 mt-2">
                      <span className="text-token-xs text-muted-foreground">{msg.time}</span>
                      <button className="w-5 h-5 flex items-center justify-center rounded text-muted-foreground/60 hover:text-muted-foreground hover:bg-accent transition-colors duration-100">
                        <Layers size={11} />
                      </button>
                    </div>
                  </div>
                ) : (
                  <div className="flex flex-col items-end gap-1.5">
                    <div
                      className="px-4 py-2.5 rounded-2xl rounded-tr-sm text-foreground/85"
                      style={{
                        background: "var(--secondary)",
                        fontSize: "var(--text-sm)",
                        lineHeight: "var(--lh-relaxed)",
                        maxWidth: 360,
                      }}
                    >
                      {msg.content}
                    </div>
                    <div className="flex items-center gap-2">
                      <span className="text-token-xs text-muted-foreground">{msg.time}</span>
                      <button className="w-5 h-5 flex items-center justify-center rounded text-muted-foreground/60 hover:text-muted-foreground hover:bg-accent transition-colors duration-100">
                        <Layers size={11} />
                      </button>
                    </div>
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>

        {/* ── Input area ── */}
        <div className="px-6 pb-4 shrink-0">
          <div className="mx-auto transition-all duration-300" style={{ maxWidth: dynamicContentWidth }}>
            <div className="flex items-center gap-3 rounded-xl border border-input px-4 py-2.5 bg-card shadow-token-sm transition-shadow duration-150 focus-within:border-ring focus-within:shadow-ring-jade">
              <button className="w-6 h-6 flex items-center justify-center rounded-md text-muted-foreground hover:text-jade transition-colors duration-150 shrink-0">
                <Plus size={16} />
              </button>
              <input
                type="text"
                value={inputValue}
                onChange={(e) => setInputValue(e.target.value)}
                placeholder="输入消息…"
                className="flex-1 bg-transparent text-foreground placeholder:text-muted-foreground outline-none text-token-sm"
              />
              <button className="w-6 h-6 flex items-center justify-center rounded-md text-muted-foreground hover:text-jade transition-colors duration-150 shrink-0">
                <Mic size={14} />
              </button>
              <button
                className={`w-7 h-7 flex items-center justify-center rounded-lg transition-all duration-150 ${
                  inputValue
                    ? "bg-jade text-primary-foreground hover:opacity-90"
                    : "bg-muted text-muted-foreground cursor-not-allowed"
                }`}
              >
                <Send size={13} />
              </button>
            </div>

            <div className="flex items-center justify-between mt-2 px-1">
              <div className="flex items-center gap-3">
                <button className="flex items-center gap-1.5 text-token-xs text-muted-foreground hover:text-foreground transition-colors duration-150">
                  <Cpu size={11} />
                  GPT-5.4-Mini
                  <ChevronDown size={10} />
                </button>
                <div className="h-3 w-px bg-border" />
                <button className="flex items-center gap-1.5 text-token-xs text-muted-foreground hover:text-foreground transition-colors duration-150">
                  <Globe size={11} />
                  中
                  <ChevronDown size={10} />
                </button>
              </div>
              <div className="flex items-center gap-3">
                <span className="text-token-xs text-muted-foreground">完全访问权限</span>
                <div className="h-3 w-px bg-border" />
                <button className="flex items-center gap-1.5 text-token-xs text-muted-foreground hover:text-foreground transition-colors duration-150">
                  <GitBranch size={11} />
                  feature/consolidate-codebase
                  <ChevronDown size={10} />
                </button>
                <div className="h-3 w-px bg-border" />
                <button className="w-5 h-5 flex items-center justify-center rounded text-muted-foreground hover:text-foreground hover:bg-accent transition-colors duration-100">
                  <SlidersHorizontal size={12} />
                </button>
              </div>
            </div>
          </div>
        </div>
      </main>

      {/* ── Inspector drag divider ── */}
      {!inspectorCollapsed && (
        <DragDivider onDrag={handleInspectorDrag} side="right" />
      )}

      {/* ══════════════════════════════════════════════════════════
          RIGHT PANEL  (inspector)
          ══════════════════════════════════════════════════════════ */}
      <aside
        className="flex flex-col h-full shrink-0 overflow-hidden transition-all duration-300"
        style={{
          width: inspectorCollapsed ? 0 : inspectorWidth,
          opacity: inspectorCollapsed ? 0 : 1,
          background: "var(--inspector)",
          borderLeft: inspectorCollapsed ? "none" : "1px solid var(--inspector-border)",
        }}
      >
        {/* ── Project header ── */}
        <div className="px-4 pt-4 pb-3 shrink-0 border-b border-border">
          <div className="flex items-center justify-between mb-2">
            <span className="text-token-sm font-semibold text-foreground/85 font-serif">
              project_nginx
            </span>
            <button className="w-6 h-6 flex items-center justify-center rounded text-muted-foreground hover:text-foreground hover:bg-accent transition-colors duration-100">
              <ChevronRight size={13} />
            </button>
          </div>
          <div className="flex items-center gap-2">
            <button className="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-accent hover:bg-secondary border border-border/60 text-token-xs text-foreground/75 transition-colors duration-150">
              <FolderOpen size={11} className="text-jade" />
              打开文件夹
            </button>
            <button className="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-accent hover:bg-secondary border border-border/60 text-token-xs text-foreground/75 transition-colors duration-150">
              <Layers size={11} className="text-jade" />
              项目技能 2
            </button>
          </div>
        </div>

        {/* ── Skills badge ── */}
        <div className="px-4 py-2.5 shrink-0 border-b border-border">
          <div className="flex items-center gap-1.5">
            <span
              className="px-2 py-0.5 rounded-md text-token-xs font-medium"
              style={{
                background: "var(--status-active-bg)",
                color: "var(--status-active)",
              }}
            >
              技能
            </span>
            <span className="text-token-xs font-semibold text-foreground/80">19</span>
          </div>
        </div>

        {/* ── Time sort ── */}
        <div className="flex items-center justify-end px-4 py-2 shrink-0">
          <button className="flex items-center gap-1 text-token-xs text-muted-foreground hover:text-foreground transition-colors duration-150">
            <Clock size={11} />
            时间
          </button>
        </div>

        {/* ── File tree ── */}
        <div className="flex-1 overflow-y-auto scrollbar-thin px-3 pb-4">
          {FILES.map((file) => (
            <FileRow key={file.id} item={file} />
          ))}
        </div>
      </aside>
    </div>
  );
}
