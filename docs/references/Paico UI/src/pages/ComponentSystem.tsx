import { useState } from "react";
import {
  MessageSquare,
  LayoutDashboard,
  Settings,
  Users,
  FolderOpen,
  Bell,
  Layers,
  Zap,
  Send,
  Plus,
  ChevronDown,
  ChevronRight,
} from "lucide-react";

// ── DS Components ─────────────────────────────────────────────────────────────
import DSButton from "../components/ds/Button";
import StatusTag from "../components/ds/StatusTag";
import SidebarItem from "../components/ds/SidebarItem";
import MessageItem from "../components/ds/MessageItem";
import FileListItem from "../components/ds/FileListItem";
import InputComposer from "../components/ds/InputComposer";
import InspectorPanel from "../components/ds/InspectorPanel";
import ChatTimeline from "../components/ds/ChatTimeline";
import DSSidebar from "../components/ds/Sidebar";

import type { FileEntry } from "../components/ds/FileListItem";
import type { ChatMessage as Message } from "../components/ds/ChatTimeline";

// ─── Nav ─────────────────────────────────────────────────────────────────────

const SECTIONS = [
  { id: "button",       label: "Button",        icon: Zap },
  { id: "statustag",    label: "StatusTag",      icon: Bell },
  { id: "sidebaritem",  label: "SidebarItem",    icon: LayoutDashboard },
  { id: "sidebar",      label: "Sidebar",        icon: Layers },
  { id: "messageitem",  label: "MessageItem",    icon: MessageSquare },
  { id: "chattimeline", label: "ChatTimeline",   icon: MessageSquare },
  { id: "filelistitem", label: "FileListItem",   icon: FolderOpen },
  { id: "inputcomposer",label: "InputComposer",  icon: Send },
  { id: "inspector",    label: "InspectorPanel", icon: Settings },
];

// ─── Sample data ──────────────────────────────────────────────────────────────

const SAMPLE_FILES: FileEntry[] = [
  {
    id: "f1",
    name: "src",
    type: "folder",
    expanded: true,
    children: [
      { id: "f1a", name: "components", type: "folder", children: [
        { id: "f1a1", name: "Button.tsx",  type: "file", ext: "tsx" },
        { id: "f1a2", name: "Sidebar.tsx", type: "file", ext: "tsx" },
      ]},
      { id: "f1b", name: "index.css", type: "file", ext: "css" },
    ],
  },
  { id: "f2", name: "package.json", type: "file", ext: "json" },
  { id: "f3", name: "README.md",    type: "file", ext: "md" },
];

const SAMPLE_MESSAGES: Message[] = [
  {
    id: "m1",
    role: "user",
    content: `帮我用 React + Tailwind 实现一个可折叠的侧边栏组件`,
    time: "14:03",
  },
  {
    id: "m2",
    role: "assistant",
    content: `好的！以下是一个可折叠侧边栏的实现方案：\n\n使用 useState 管理展开状态，transition-all duration-300 实现动画，宽度在 collapsed 时从 220px 切换到 48px。\n\n核心结构只需两层 div：外层控制宽度，内层 overflow-hidden 防止内容溢出。`,
    time: "14:03",
    thinking: `用户需要一个可折叠侧边栏。\n考虑实现方式：\n1. CSS transition 控制宽度变化\n2. 图标始终显示，文字在展开时显示\n3. 需要 overflow-hidden 防止文字溢出`,
  },
  {
    id: "m3",
    role: "user",
    content: `动画如何更流畅？`,
    time: "14:05",
  },
  {
    id: "m4",
    role: "assistant",
    content: `可以结合 will-change: width 和 transform: translateX 两种方式，后者硬件加速更好。另外建议给文字加 whitespace-nowrap 避免换行导致的高度抖动。`,
    time: "14:05",
  },
];

// ─── Section wrapper ──────────────────────────────────────────────────────────

function Section({ id, title, subtitle, children }: {
  id: string;
  title: string;
  subtitle?: string;
  children: React.ReactNode;
}) {
  return (
    <section id={id} className="mb-16 scroll-mt-20">
      <div className="mb-5">
        <h2 className="text-[22px] font-semibold text-foreground font-serif">{title}</h2>
        {subtitle && <p className="text-[13px] text-muted-foreground mt-1">{subtitle}</p>}
      </div>
      {children}
    </section>
  );
}

function SpecCard({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="rounded-xl border border-border overflow-hidden bg-card shadow-token-sm">
      <div className="px-4 py-2 bg-accent border-b border-border">
        <span className="text-[11px] font-semibold text-muted-foreground uppercase tracking-widest">{label}</span>
      </div>
      <div className="p-5">{children}</div>
    </div>
  );
}

function TokenRow({ name, desc }: { name: string; desc: string }) {
  return (
    <div className="flex items-center justify-between py-1.5 border-b border-border/50 last:border-0">
      <code className="text-[11px] text-jade font-mono">{name}</code>
      <span className="text-[11px] text-muted-foreground">{desc}</span>
    </div>
  );
}

// ─── Main page ───────────────────────────────────────────────────────────────

export default function ComponentSystem() {
  const [activeSection, setActiveSection] = useState("button");
  const [sidebarActive, setSidebarActive] = useState("chat");
  const [selectedSidebarItem, setSelectedSidebarItem] = useState("chat");
  const [composerValue, setComposerValue] = useState("");
  const [navOpen, setNavOpen] = useState(true);

  const scrollToSection = (id: string) => {
    const el = document.getElementById(id);
    if (el) el.scrollIntoView({ behavior: "smooth", block: "start" });
    setActiveSection(id);
    console.log("ComponentSystem: scrollTo", id);
  };

  return (
    <div data-cmp="ComponentSystem" className="flex h-screen overflow-hidden bg-background">

      {/* ── Left nav ── */}
      <aside className="w-56 shrink-0 border-r border-border flex flex-col bg-sidebar overflow-hidden">
        {/* Logo */}
        <div className="px-5 pt-6 pb-4 shrink-0">
          <div className="flex items-center gap-2.5">
            <div className="w-6 h-6 rounded-md bg-jade flex items-center justify-center shrink-0">
              <Layers size={13} className="text-primary-foreground" />
            </div>
            <span className="text-[13px] font-semibold text-sidebar-foreground font-serif">DS 规范</span>
          </div>
          <p className="text-[11px] text-muted-foreground mt-1">9 个设计系统组件</p>
        </div>

        {/* Nav */}
        <nav className="flex-1 overflow-y-auto scrollbar-thin px-3 py-2">
          {SECTIONS.map((s) => (
            <button
              key={s.id}
              onClick={() => scrollToSection(s.id)}
              className={`w-full flex items-center gap-2.5 px-2.5 py-2 rounded-lg text-[12px] transition-colors duration-150 mb-0.5 ${
                activeSection === s.id
                  ? "bg-sidebar-accent text-sidebar-primary font-medium"
                  : "text-muted-foreground hover:bg-sidebar-accent hover:text-sidebar-foreground"
              }`}
            >
              <s.icon size={13} className="shrink-0" />
              {s.label}
              {activeSection === s.id && <ChevronRight size={11} className="ml-auto" />}
            </button>
          ))}
        </nav>

        {/* Footer */}
        <div className="px-4 py-4 border-t border-sidebar-border shrink-0">
          <p className="text-[10px] text-muted-foreground">Jade Mist Teal · v1.0</p>
        </div>
      </aside>

      {/* ── Main scroll area ── */}
      <main className="flex-1 overflow-y-auto scrollbar-thin px-8 py-8">
        {/* Header */}
        <div className="mb-10">
          <h1 className="text-[30px] font-semibold text-foreground font-serif">组件系统</h1>
          <p className="text-[13px] text-muted-foreground mt-2 max-w-xl">
            Jade Mist Teal 设计语言 · 完整组件规范文档。每个区块展示结构、状态、内边距与颜色规则。
          </p>
        </div>

        {/* ════════════════════════════════════════════ BUTTON */}
        <Section id="button" title="Button" subtitle="三种视觉变体 × 三种尺寸 · 主动作 / 次要动作 / 幽灵">
          <div className="flex flex-col gap-5">

            <SpecCard label="Variants">
              <div className="flex items-center gap-3 flex-wrap">
                <DSButton variant="primary"   label="主要按钮" />
                <DSButton variant="secondary" label="次要按钮" />
                <DSButton variant="ghost"     label="幽灵按钮" />
              </div>
            </SpecCard>

            <SpecCard label="Sizes">
              <div className="flex items-end gap-3 flex-wrap">
                <DSButton variant="primary" size="sm" label="Small" />
                <DSButton variant="primary" size="md" label="Medium" />
                <DSButton variant="primary" size="lg" label="Large" />
              </div>
            </SpecCard>

            <SpecCard label="With Icon">
              <div className="flex items-center gap-3 flex-wrap">
                <DSButton variant="primary"   Icon={Plus}    label="新建" />
                <DSButton variant="secondary" Icon={FolderOpen} label="打开" />
                <DSButton variant="ghost"     Icon={Settings}  label="设置" />
              </div>
            </SpecCard>

            <SpecCard label="Disabled State">
              <div className="flex items-center gap-3 flex-wrap">
                <DSButton variant="primary"   label="主要"   disabled />
                <DSButton variant="secondary" label="次要"   disabled />
                <DSButton variant="ghost"     label="幽灵"   disabled />
              </div>
            </SpecCard>

            <SpecCard label="Token Reference">
              <TokenRow name="bg-jade"                  desc="primary variant background" />
              <TokenRow name="text-primary-foreground"  desc="primary variant text" />
              <TokenRow name="bg-accent"                desc="secondary variant background" />
              <TokenRow name="border-border"            desc="secondary variant border" />
              <TokenRow name="hover:opacity-90"         desc="primary hover" />
              <TokenRow name="active:scale-95"          desc="press feedback" />
            </SpecCard>
          </div>
        </Section>

        {/* ════════════════════════════════════════════ STATUS TAG */}
        <Section id="statustag" title="StatusTag" subtitle="6 种语义状态 · 可选 dot 指示符 · tag-pill 工具类">
          <div className="flex flex-col gap-5">
            <SpecCard label="All Status Variants (without dot)">
              <div className="flex items-center gap-3 flex-wrap">
                <StatusTag status="active"  />
                <StatusTag status="pending" />
                <StatusTag status="warning" />
                <StatusTag status="success" />
                <StatusTag status="error"   />
                <StatusTag status="neutral" />
              </div>
            </SpecCard>

            <SpecCard label="With Dot Indicator">
              <div className="flex items-center gap-3 flex-wrap">
                <StatusTag status="active"  dot />
                <StatusTag status="pending" dot />
                <StatusTag status="warning" dot />
                <StatusTag status="success" dot />
                <StatusTag status="error"   dot />
                <StatusTag status="neutral" dot />
              </div>
            </SpecCard>

            <SpecCard label="Custom Labels">
              <div className="flex items-center gap-3 flex-wrap">
                <StatusTag status="active"  dot label="运行中" />
                <StatusTag status="pending" dot label="队列中" />
                <StatusTag status="error"   dot label="构建失败" />
              </div>
            </SpecCard>

            <SpecCard label="Token Reference">
              <TokenRow name="--status-active-bg / --status-active"    desc="进行中 background / foreground" />
              <TokenRow name="--status-pending-bg / --status-pending"   desc="等待中 background / foreground" />
              <TokenRow name="--status-warning-bg / --status-warning"   desc="警告   background / foreground" />
              <TokenRow name="--status-success-bg / --status-success"   desc="成功   background / foreground" />
              <TokenRow name="--status-error-bg / --status-error"       desc="错误   background / foreground" />
              <TokenRow name="--status-neutral-bg / --status-neutral"   desc="中性   background / foreground" />
              <TokenRow name=".tag-pill"                                 desc="shape: rounded-full, px-2 py-0.5, text-xs" />
            </SpecCard>
          </div>
        </Section>

        {/* ════════════════════════════════════════════ SIDEBAR ITEM */}
        <Section id="sidebaritem" title="SidebarItem" subtitle="导航单元 · 三种状态 · 可选 badge 与 chevron">
          <div className="flex flex-col gap-5">
            <SpecCard label="States">
              <div className="w-52 flex flex-col gap-0.5 bg-sidebar p-3 rounded-xl border border-border">
                <SidebarItem
                  id="chat"
                  label="对话"
                  icon={MessageSquare}
                  selected={selectedSidebarItem === "chat"}
                  onClick={() => { setSelectedSidebarItem("chat"); console.log("SidebarItem: chat"); }}
                />
                <SidebarItem
                  id="projects"
                  label="项目"
                  icon={FolderOpen}
                  badge={3}
                  selected={selectedSidebarItem === "projects"}
                  onClick={() => { setSelectedSidebarItem("projects"); console.log("SidebarItem: projects"); }}
                />
                <SidebarItem
                  id="team"
                  label="团队"
                  icon={Users}
                  selected={selectedSidebarItem === "team"}
                  onClick={() => { setSelectedSidebarItem("team"); console.log("SidebarItem: team"); }}
                />
                <SidebarItem
                  id="settings"
                  label="设置"
                  icon={Settings}
                  selected={selectedSidebarItem === "settings"}
                  onClick={() => { setSelectedSidebarItem("settings"); console.log("SidebarItem: settings"); }}
                />
              </div>
            </SpecCard>

            <SpecCard label="Token Reference">
              <TokenRow name="px-2.5 py-2"             desc="item padding: 10px / 8px" />
              <TokenRow name="gap-3"                    desc="icon → label gap: 12px" />
              <TokenRow name="bg-sidebar-accent"        desc="hover & selected background" />
              <TokenRow name="text-sidebar-primary"     desc="selected text + icon" />
              <TokenRow name="text-muted-foreground"    desc="default text" />
              <TokenRow name="bg-jade"                  desc="badge background" />
              <TokenRow name="w-4 h-4 rounded-full"     desc="badge shape" />
            </SpecCard>
          </div>
        </Section>

        {/* ════════════════════════════════════════════ SIDEBAR */}
        <Section id="sidebar" title="Sidebar" subtitle="220px 完整侧边栏 · Logo + Nav + 底部用户卡片">
          <div className="flex flex-col gap-5">
            <SpecCard label="Full Sidebar (220px)">
              <div className="h-[480px] w-56 overflow-hidden rounded-xl border border-border shadow-token-sm">
                <DSSidebar
                  activeSection={sidebarActive}
                  onNavigate={(id) => { setSidebarActive(id); console.log("Sidebar nav:", id); }}
                  studioName="Jade Studio"
                  studioPlan="Pro"
                  userName="吴明远"
                  userRole="设计工程师"
                  userInitials="吴"
                  notificationCount={2}
                />
              </div>
            </SpecCard>

            <SpecCard label="Token Reference">
              <TokenRow name="--sidebar"                desc="panel background: oklch(94.5% 0.004 240)" />
              <TokenRow name="--sidebar-accent"         desc="hover / selected item bg" />
              <TokenRow name="--sidebar-foreground"     desc="primary text" />
              <TokenRow name="--sidebar-primary"        desc="active icon & selected text (jade)" />
              <TokenRow name="--sidebar-border"         desc="divider & right border" />
              <TokenRow name="w-[220px]"                desc="panel width" />
              <TokenRow name="px-5 pt-6 pb-5"          desc="logo area padding" />
              <TokenRow name="px-3 py-4"               desc="nav area padding" />
            </SpecCard>
          </div>
        </Section>

        {/* ════════════════════════════════════════════ MESSAGE ITEM */}
        <Section id="messageitem" title="MessageItem" subtitle="User 气泡 · Assistant 文本 · ThinkingBlock 折叠 · 悬停元数据">
          <div className="flex flex-col gap-5">
            <SpecCard label="Assistant Message (with ThinkingBlock)">
              <div className="max-w-xl">
                <MessageItem
                  id="demo-a1"
                  role="assistant"
                  content={`好的！以下是一个可折叠侧边栏的实现方案：\n\n使用 useState 管理展开状态，transition-all duration-300 实现动画，宽度在 collapsed 时从 220px 切换到 48px。`}
                  time="14:03"
                  thinking={`用户需要一个可折叠侧边栏。\n考虑实现方式：\n1. CSS transition 控制宽度变化\n2. 图标始终显示，文字在展开时显示`}
                  thinkingExpanded={navOpen}
                  onToggleThinking={() => { setNavOpen(!navOpen); console.log("MessageItem: toggle thinking"); }}
                />
              </div>
            </SpecCard>

            <SpecCard label="User Bubble">
              <div className="max-w-xl">
                <MessageItem
                  id="demo-u1"
                  role="user"
                  content="帮我用 React + Tailwind 实现一个可折叠的侧边栏组件"
                  time="14:03"
                />
              </div>
            </SpecCard>

            <SpecCard label="Token Reference">
              <TokenRow name="bg-secondary"             desc="user bubble background" />
              <TokenRow name="px-4 py-2.5"             desc="user bubble padding" />
              <TokenRow name="rounded-2xl rounded-tr-sm" desc="user bubble corners" />
              <TokenRow name="max-w-[360px]"            desc="user bubble max width" />
              <TokenRow name="text-foreground/85"       desc="message text color" />
              <TokenRow name="text-muted-foreground"    desc="timestamp & copy icon" />
              <TokenRow name="bg-status-active"         desc="thinking dot indicator" />
              <TokenRow name="opacity-0 group-hover:opacity-100" desc="meta row fade-in on hover" />
            </SpecCard>
          </div>
        </Section>

        {/* ════════════════════════════════════════════ CHAT TIMELINE */}
        <Section id="chattimeline" title="ChatTimeline" subtitle="消息流容器 · max-w-2xl 中心列 · 滚动 · 管理 ThinkingBlock 状态">
          <div className="flex flex-col gap-5">
            <SpecCard label="Timeline Preview (fixed height)">
              <div className="h-[380px] rounded-xl border border-border overflow-hidden">
                <ChatTimeline messages={SAMPLE_MESSAGES} />
              </div>
            </SpecCard>

            <SpecCard label="Token Reference">
              <TokenRow name="flex-1 overflow-y-auto"  desc="scroll container" />
              <TokenRow name="max-w-2xl mx-auto"       desc="content column centering" />
              <TokenRow name="px-6 py-5"               desc="container padding: 24/20px" />
              <TokenRow name="gap-6"                   desc="message gap: 24px" />
              <TokenRow name="bg-background"           desc="panel background" />
              <TokenRow name=".scrollbar-thin"         desc="thin custom scrollbar" />
            </SpecCard>
          </div>
        </Section>

        {/* ════════════════════════════════════════════ FILE LIST ITEM */}
        <Section id="filelistitem" title="FileListItem" subtitle="文件/文件夹树节点 · 递归展开 · 扩展名图标 · 缩进层级">
          <div className="flex flex-col gap-5">
            <SpecCard label="File Tree (nested)">
              <div className="w-56 bg-surface rounded-xl border border-border p-3">
                {SAMPLE_FILES.map((f) => (
                  <FileListItem key={f.id} item={f} />
                ))}
              </div>
            </SpecCard>

            <SpecCard label="Individual Entries">
              <div className="w-56 flex flex-col bg-surface rounded-xl border border-border p-2">
                <FileListItem item={{ id: "x1", name: "components",  type: "folder" }} />
                <FileListItem item={{ id: "x2", name: "Button.tsx",  type: "file", ext: "tsx" }} />
                <FileListItem item={{ id: "x3", name: "README.md",   type: "file", ext: "md"  }} />
                <FileListItem item={{ id: "x4", name: "logo.png",    type: "file", ext: "png" }} />
                <FileListItem item={{ id: "x5", name: "styles.css",  type: "file", ext: "css" }} />
              </div>
            </SpecCard>

            <SpecCard label="Token Reference">
              <TokenRow name="px-2 py-1"             desc="item padding: 8px / 4px" />
              <TokenRow name="gap-1.5"               desc="chevron → icon → name gap" />
              <TokenRow name="rounded-md"            desc="item corner radius" />
              <TokenRow name="text-jade"             desc="folder icon color" />
              <TokenRow name="text-muted-foreground" desc="file icon color" />
              <TokenRow name="hover:bg-accent"       desc="hover background" />
              <TokenRow name="pl + depth×12px"       desc="indent per nesting level" />
              <TokenRow name="rotate-90"             desc="chevron rotation when expanded" />
            </SpecCard>
          </div>
        </Section>

        {/* ════════════════════════════════════════════ INPUT COMPOSER */}
        <Section id="inputcomposer" title="InputComposer" subtitle="多功能输入框 · 状态感知发送按钮 · 底部元数据条">
          <div className="flex flex-col gap-5">
            <SpecCard label="Default (empty)">
              <div className="max-w-xl">
                <InputComposer value={composerValue} onChange={setComposerValue} />
              </div>
            </SpecCard>

            <SpecCard label="With Content (send button activates)">
              <div className="max-w-xl">
                <InputComposer
                  value="帮我实现一个动态表单验证方案"
                  onChange={() => {}}
                  placeholder="输入消息…"
                  modelName="Claude-3.7"
                  branchName="feature/form-validation"
                />
              </div>
            </SpecCard>

            <SpecCard label="Disabled State">
              <div className="max-w-xl">
                <InputComposer
                  value="正在生成…"
                  onChange={() => {}}
                  disabled
                />
              </div>
            </SpecCard>

            <SpecCard label="Token Reference">
              <TokenRow name="bg-card"                desc="input box background" />
              <TokenRow name="border-input"           desc="default border" />
              <TokenRow name="focus-within:border-ring" desc="focused border" />
              <TokenRow name="shadow-ring-jade"       desc="focused glow" />
              <TokenRow name="rounded-xl"             desc="input box radius: 12px" />
              <TokenRow name="px-4 py-2.5"           desc="input padding" />
              <TokenRow name="bg-jade"                desc="send button (active)" />
              <TokenRow name="bg-muted"               desc="send button (empty/disabled)" />
              <TokenRow name="w-7 h-7 rounded-lg"    desc="send button size" />
            </SpecCard>
          </div>
        </Section>

        {/* ════════════════════════════════════════════ INSPECTOR PANEL */}
        <Section id="inspector" title="InspectorPanel" subtitle="右侧检查器 · 240px · 项目头 + 技能标签 + 文件树">
          <div className="flex flex-col gap-5">
            <SpecCard label="Full Panel (240px, fixed height)">
              <div className="h-[480px] overflow-hidden rounded-xl border border-border shadow-token-sm">
                <InspectorPanel
                  projectName="project_nginx"
                  skillCount={19}
                  width={240}
                />
              </div>
            </SpecCard>

            <SpecCard label="Token Reference">
              <TokenRow name="--surface"                 desc="panel background" />
              <TokenRow name="width: 240px"              desc="panel fixed width" />
              <TokenRow name="border-l border-border"   desc="left separator" />
              <TokenRow name="px-4 pt-4 pb-3"           desc="header padding" />
              <TokenRow name="--status-active-bg / --status-active" desc="skills badge colors" />
              <TokenRow name="bg-accent hover:bg-secondary" desc="action button states" />
              <TokenRow name="flex-1 overflow-y-auto"    desc="file tree scroll area" />
            </SpecCard>
          </div>
        </Section>

        {/* Bottom spacer */}
        <div className="h-20" />
      </main>
    </div>
  );
}
