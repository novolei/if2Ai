import { useState } from "react";
import { Check, Copy } from "lucide-react";

/* ── tiny copy helper ── */
function useCopy() {
  const [copied, setCopied] = useState<string | null>(null);
  const copy = (val: string) => {
    navigator.clipboard.writeText(val).catch(() => {});
    setCopied(val);
    setTimeout(() => setCopied(null), 1500);
  };
  return { copied, copy };
}

/* ── section wrapper ── */
function Section({
  id,
  title,
  sub,
  children,
}: {
  id: string;
  title: string;
  sub: string;
  children: React.ReactNode;
}) {
  return (
    <section id={id} className="mb-12">
      <div className="mb-5">
        <p className="text-token-xs text-muted-foreground uppercase tracking-widest mb-1 font-sans">
          {sub}
        </p>
        <h2 className="font-serif text-token-2xl text-foreground">{title}</h2>
      </div>
      {children}
    </section>
  );
}

/* ── color swatch ── */
function Swatch({
  label,
  variable,
  textDark = false,
  copy,
  copied,
}: {
  label: string;
  variable: string;
  textDark?: boolean;
  copy: (v: string) => void;
  copied: string | null;
}) {
  const val = `var(${variable})`;
  const isCopied = copied === variable;
  return (
    <button
      onClick={() => copy(variable)}
      className="group relative flex flex-col overflow-hidden rounded-xl border border-border transition-all duration-200 hover:-translate-y-0.5 shadow-token-xs hover:shadow-token-sm text-left w-full"
      style={{ background: val }}
      title={`复制 ${variable}`}
    >
      <div className="h-14" />
      <div
        className="px-3 py-2.5 border-t border-border"
        style={{ background: `color-mix(in oklch, var(--surface-raised) 92%, ${val})` }}
      >
        <p
          className="font-sans font-medium leading-tight mb-0.5"
          style={{
            fontSize: "var(--text-xs)",
            color: textDark ? "var(--ink)" : "var(--foreground)",
          }}
        >
          {label}
        </p>
        <p
          className="font-mono leading-none"
          style={{ fontSize: "10px", color: "var(--muted-foreground)" }}
        >
          {variable}
        </p>
      </div>
      <span
        className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity p-1 rounded-md"
        style={{ background: "var(--surface-overlay)" }}
      >
        {isCopied ? (
          <Check size={12} style={{ color: "var(--status-success)" }} />
        ) : (
          <Copy size={12} style={{ color: "var(--muted-foreground)" }} />
        )}
      </span>
    </button>
  );
}

/* ── spacing row ── */
function SpaceRow({
  token,
  value,
  px,
  copy,
  copied,
}: {
  token: string;
  value: string;
  px: string;
  copy: (v: string) => void;
  copied: string | null;
}) {
  const isCopied = copied === token;
  return (
    <button
      onClick={() => copy(token)}
      className="group flex items-center gap-4 w-full py-2.5 px-4 rounded-xl hover:bg-accent transition-colors text-left"
    >
      <span
        className="flex-shrink-0 rounded-md"
        style={{
          width: value,
          height: "var(--space-4)",
          background: "var(--jade-light)",
          minWidth: "4px",
        }}
      />
      <span
        className="font-mono text-foreground flex-shrink-0"
        style={{ fontSize: "var(--text-sm)", width: "5rem" }}
      >
        {token}
      </span>
      <span
        className="text-muted-foreground flex-shrink-0"
        style={{ fontSize: "var(--text-sm)", width: "4rem" }}
      >
        {value}
      </span>
      <span className="text-muted-foreground" style={{ fontSize: "var(--text-xs)" }}>
        {px}
      </span>
      <span className="ml-auto opacity-0 group-hover:opacity-100 transition-opacity">
        {isCopied ? (
          <Check size={13} style={{ color: "var(--status-success)" }} />
        ) : (
          <Copy size={13} style={{ color: "var(--muted-foreground)" }} />
        )}
      </span>
    </button>
  );
}

/* ── radius row ── */
function RadiusRow({
  token,
  value,
  px,
  copy,
  copied,
}: {
  token: string;
  value: string;
  px: string;
  copy: (v: string) => void;
  copied: string | null;
}) {
  const isCopied = copied === token;
  return (
    <button
      onClick={() => copy(token)}
      className="group flex items-center gap-5 w-full py-2.5 px-4 rounded-xl hover:bg-accent transition-colors text-left"
    >
      <div
        className="flex-shrink-0 border border-border"
        style={{
          width: 40,
          height: 40,
          borderRadius: value === "9999px" ? "9999px" : value,
          background: "var(--jade-light)",
        }}
      />
      <span
        className="font-mono text-foreground flex-shrink-0"
        style={{ fontSize: "var(--text-sm)", width: "6rem" }}
      >
        {token}
      </span>
      <span
        className="text-muted-foreground flex-shrink-0"
        style={{ fontSize: "var(--text-sm)", width: "4.5rem" }}
      >
        {value}
      </span>
      <span className="text-muted-foreground" style={{ fontSize: "var(--text-xs)" }}>
        {px}
      </span>
      <span className="ml-auto opacity-0 group-hover:opacity-100 transition-opacity">
        {isCopied ? (
          <Check size={13} style={{ color: "var(--status-success)" }} />
        ) : (
          <Copy size={13} style={{ color: "var(--muted-foreground)" }} />
        )}
      </span>
    </button>
  );
}

/* ── shadow card ── */
function ShadowCard({
  token,
  variable,
  label,
  copy,
  copied,
}: {
  token: string;
  variable: string;
  label: string;
  copy: (v: string) => void;
  copied: string | null;
}) {
  const isCopied = copied === variable;
  return (
    <button
      onClick={() => copy(variable)}
      className="group flex flex-col items-center gap-4 p-6 rounded-2xl border border-border bg-card transition-all duration-200 hover:-translate-y-0.5 hover:bg-accent w-full text-left"
    >
      <div
        className="w-16 h-16 rounded-2xl bg-surface-raised border border-border"
        style={{ boxShadow: `var(${variable})` }}
      />
      <div className="text-center">
        <p className="font-mono text-foreground font-medium" style={{ fontSize: "var(--text-sm)" }}>
          {token}
        </p>
        <p className="text-muted-foreground mt-0.5" style={{ fontSize: "var(--text-xs)" }}>
          {label}
        </p>
      </div>
      <span className="opacity-0 group-hover:opacity-100 transition-opacity">
        {isCopied ? (
          <Check size={13} style={{ color: "var(--status-success)" }} />
        ) : (
          <Copy size={13} style={{ color: "var(--muted-foreground)" }} />
        )}
      </span>
    </button>
  );
}

/* ── typography row ── */
function TypeRow({
  name,
  desc,
  sizeVar,
  weight,
  serif = false,
  lhVar,
}: {
  name: string;
  desc: string;
  sizeVar: string;
  weight: string;
  serif?: boolean;
  lhVar?: string;
}) {
  return (
    <div className="flex items-baseline gap-6 py-4 px-4 rounded-xl hover:bg-accent transition-colors border-b border-border last:border-0">
      <div className="flex-shrink-0" style={{ width: "8rem" }}>
        <p className="text-muted-foreground" style={{ fontSize: "var(--text-xs)" }}>
          {name}
        </p>
        <p className="text-muted-foreground mt-0.5" style={{ fontSize: "10px" }}>
          {desc}
        </p>
      </div>
      <p
        style={{
          fontFamily: serif ? "var(--fontSerif)" : "var(--fontSans)",
          fontSize: `var(${sizeVar})`,
          fontWeight: weight,
          lineHeight: lhVar ? `var(${lhVar})` : "var(--lh-normal)",
          color: "var(--foreground)",
        }}
        className="flex-1 min-w-0 truncate"
      >
        雾感玉石青 · Jade Mist Teal
      </p>
      <span className="font-mono text-muted-foreground flex-shrink-0" style={{ fontSize: "10px" }}>
        {sizeVar}
      </span>
    </div>
  );
}

/* ══════════════════════════════════════════════════════════════════
   MAIN PAGE
   ══════════════════════════════════════════════════════════════════ */
export default function TokenSystem() {
  const { copied, copy } = useCopy();

  const navItems = [
    { id: "color", label: "颜色" },
    { id: "spacing", label: "间距" },
    { id: "radius", label: "圆角" },
    { id: "shadow", label: "阴影" },
    { id: "typography", label: "字体" },
  ];

  const bgColors = [
    { label: "Background", variable: "--background" },
    { label: "Surface", variable: "--surface" },
    { label: "Surface Raised", variable: "--surface-raised" },
    { label: "Card", variable: "--card" },
    { label: "Popover", variable: "--popover" },
    { label: "Mist", variable: "--mist" },
    { label: "Mist Dark", variable: "--mist-dark" },
  ];

  const accentColors = [
    { label: "Primary", variable: "--primary" },
    { label: "Jade", variable: "--jade" },
    { label: "Jade Light", variable: "--jade-light" },
    { label: "Jade Dim", variable: "--jade-dim" },
    { label: "Teal", variable: "--teal" },
    { label: "Teal Light", variable: "--teal-light" },
    { label: "Celadon", variable: "--celadon" },
    { label: "Celadon Light", variable: "--celadon-light" },
  ];

  const textColors = [
    { label: "Foreground", variable: "--foreground" },
    { label: "Card Foreground", variable: "--card-foreground" },
    { label: "Muted Foreground", variable: "--muted-foreground" },
    { label: "Ink", variable: "--ink" },
    { label: "Stone", variable: "--stone" },
    { label: "Muted", variable: "--muted" },
    { label: "Secondary", variable: "--secondary" },
    { label: "Accent", variable: "--accent" },
  ];

  const borderColors = [
    { label: "Border", variable: "--border" },
    { label: "Input", variable: "--input" },
    { label: "Ring", variable: "--ring" },
    { label: "Sidebar Border", variable: "--sidebar-border" },
  ];

  const warmColors = [
    { label: "Sand Warm", variable: "--sand-warm" },
    { label: "Sand", variable: "--sand" },
  ];

  const statusColors = [
    { label: "Active", variable: "--status-active", bg: "--status-active-bg" },
    { label: "Pending", variable: "--status-pending", bg: "--status-pending-bg" },
    { label: "Warning", variable: "--status-warning", bg: "--status-warning-bg" },
    { label: "Success", variable: "--status-success", bg: "--status-success-bg" },
    { label: "Error", variable: "--status-error", bg: "--status-error-bg" },
    { label: "Neutral", variable: "--status-neutral", bg: "--status-neutral-bg" },
  ];

  const spaces = [
    { token: "--space-1", value: "0.25rem", px: "4px" },
    { token: "--space-2", value: "0.5rem", px: "8px" },
    { token: "--space-3", value: "0.75rem", px: "12px" },
    { token: "--space-4", value: "1rem", px: "16px" },
    { token: "--space-5", value: "1.25rem", px: "20px" },
    { token: "--space-6", value: "1.5rem", px: "24px" },
    { token: "--space-8", value: "2rem", px: "32px" },
    { token: "--space-10", value: "2.5rem", px: "40px" },
    { token: "--space-12", value: "3rem", px: "48px" },
    { token: "--space-16", value: "4rem", px: "64px" },
  ];

  const radii = [
    { token: "--radius-xs", value: "0.25rem", px: "4px" },
    { token: "--radius-sm", value: "0.375rem", px: "6px" },
    { token: "--radius-md", value: "0.5rem", px: "8px" },
    { token: "--radius", value: "0.625rem", px: "10px  (base)" },
    { token: "--radius-lg", value: "0.75rem", px: "12px" },
    { token: "--radius-xl", value: "1rem", px: "16px" },
    { token: "--radius-2xl", value: "1.5rem", px: "24px" },
    { token: "--radius-full", value: "9999px", px: "pill" },
  ];

  const shadows = [
    { token: "--shadow-xs", variable: "--shadow-xs", label: "极轻 · 悬停提示" },
    { token: "--shadow-sm", variable: "--shadow-sm", label: "轻柔 · 卡片默认" },
    { token: "--shadow-md", variable: "--shadow-md", label: "中柔 · 下拉/弹层" },
    { token: "--shadow-lg", variable: "--shadow-lg", label: "深柔 · 模态/对话框" },
    { token: "--shadow-inset", variable: "--shadow-inset", label: "内嵌 · 输入框凹陷" },
    { token: "--shadow-ring", variable: "--shadow-ring", label: "焦点环 · 交互反馈" },
  ];

  const typeScale = [
    { name: "Display", desc: "4xl · Serif · 300", sizeVar: "--text-4xl", weight: "300", serif: true, lhVar: "--lh-tight" },
    { name: "Title 1", desc: "3xl · Serif · 400", sizeVar: "--text-3xl", weight: "400", serif: true, lhVar: "--lh-tight" },
    { name: "Title 2", desc: "2xl · Serif · 400", sizeVar: "--text-2xl", weight: "400", serif: true, lhVar: "--lh-snug" },
    { name: "Heading", desc: "xl · Sans · 500", sizeVar: "--text-xl", weight: "500", serif: false, lhVar: "--lh-snug" },
    { name: "Subheading", desc: "lg · Sans · 500", sizeVar: "--text-lg", weight: "500", serif: false, lhVar: "--lh-snug" },
    { name: "Body MD", desc: "md · Sans · 400", sizeVar: "--text-md", weight: "400", serif: false },
    { name: "Body", desc: "base · Sans · 400", sizeVar: "--text-base", weight: "400", serif: false },
    { name: "Small", desc: "sm · Sans · 400", sizeVar: "--text-sm", weight: "400", serif: false },
    { name: "Caption", desc: "xs · Sans · 500", sizeVar: "--text-xs", weight: "500", serif: false },
  ];

  const scrollTo = (id: string) => {
    document.getElementById(id)?.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  return (
    <div data-cmp="TokenSystem" className="flex h-full w-full overflow-hidden bg-background">

      {/* ── Side Nav ── */}
      <aside
        className="flex-shrink-0 flex flex-col border-r border-border bg-card overflow-y-auto scrollbar-thin"
        style={{ width: "13rem" }}
      >
        <div className="px-5 pt-7 pb-5 border-b border-border">
          <p className="text-muted-foreground uppercase tracking-widest mb-1" style={{ fontSize: "10px" }}>
            Design System
          </p>
          <h1 className="font-serif text-foreground leading-tight" style={{ fontSize: "var(--text-lg)" }}>
            Token
          </h1>
          <p className="font-serif text-muted-foreground italic" style={{ fontSize: "var(--text-sm)" }}>
            雾感玉石青
          </p>
        </div>
        <nav className="p-3 flex flex-col gap-0.5 flex-1">
          {navItems.map((n) => (
            <button
              key={n.id}
              onClick={() => scrollTo(n.id)}
              className="w-full text-left px-3 py-2 rounded-lg text-foreground hover:bg-accent transition-colors"
              style={{ fontSize: "var(--text-sm)" }}
            >
              {n.label}
            </button>
          ))}
        </nav>
        <div className="p-4 border-t border-border">
          <p className="text-muted-foreground" style={{ fontSize: "10px", lineHeight: "1.6" }}>
            点击任意 Token<br />即可复制变量名
          </p>
        </div>
      </aside>

      {/* ── Main content ── */}
      <main className="flex-1 min-w-0 overflow-y-auto scrollbar-thin px-10 py-10">
        <div style={{ maxWidth: "900px" }}>

          {/* ── Hero ── */}
          <div className="mb-12 pb-8 border-b border-border">
            <p className="text-muted-foreground uppercase tracking-widest mb-2" style={{ fontSize: "var(--text-xs)" }}>
              CSS Variable Design Token System
            </p>
            <h1
              className="font-serif text-foreground mb-3"
              style={{ fontSize: "var(--text-4xl)", fontWeight: "var(--fw-light)", lineHeight: "var(--lh-tight)" }}
            >
              雾感玉石青
            </h1>
            <p className="text-muted-foreground" style={{ fontSize: "var(--text-base)", lineHeight: "var(--lh-relaxed)", maxWidth: "520px" }}>
              低对比 · 柔和 · 温暖 · 可直接用于生产环境的完整 Token 系统，基于 oklch 色彩空间构建，统一、无冲突。
            </p>
            <div className="flex items-center gap-3 mt-5">
              {["oklch 色彩空间", "CSS Variables", "Tailwind 映射", "无冲突"].map((tag) => (
                <span key={tag} className="tag-pill">{tag}</span>
              ))}
            </div>
          </div>

          {/* ════════════ COLOR ════════════ */}
          <Section id="color" title="颜色体系" sub="Color Tokens">

            {/* backgrounds */}
            <div className="mb-7">
              <p
                className="text-muted-foreground uppercase tracking-widest mb-3"
                style={{ fontSize: "var(--text-xs)" }}
              >
                背景 · Background
              </p>
              <div className="flex flex-wrap gap-3">
                {bgColors.map((c) => (
                  <div key={c.variable} style={{ width: "calc(14.28% - 12px)", minWidth: "100px" }}>
                    <Swatch {...c} copy={copy} copied={copied} />
                  </div>
                ))}
              </div>
            </div>

            {/* accent / brand */}
            <div className="mb-7">
              <p
                className="text-muted-foreground uppercase tracking-widest mb-3"
                style={{ fontSize: "var(--text-xs)" }}
              >
                强调色 · Accent & Brand
              </p>
              <div className="flex flex-wrap gap-3">
                {accentColors.map((c) => (
                  <div key={c.variable} style={{ width: "calc(12.5% - 12px)", minWidth: "96px" }}>
                    <Swatch {...c} copy={copy} copied={copied} />
                  </div>
                ))}
              </div>
            </div>

            {/* text & surface */}
            <div className="mb-7">
              <p
                className="text-muted-foreground uppercase tracking-widest mb-3"
                style={{ fontSize: "var(--text-xs)" }}
              >
                文字 & 面层 · Text & Surface
              </p>
              <div className="flex flex-wrap gap-3">
                {textColors.map((c) => (
                  <div key={c.variable} style={{ width: "calc(12.5% - 12px)", minWidth: "96px" }}>
                    <Swatch {...c} copy={copy} copied={copied} />
                  </div>
                ))}
              </div>
            </div>

            {/* warm neutral */}
            <div className="mb-7">
              <p
                className="text-muted-foreground uppercase tracking-widest mb-3"
                style={{ fontSize: "var(--text-xs)" }}
              >
                温暖中性 · Warm Neutrals
              </p>
              <div className="flex flex-wrap gap-3">
                {warmColors.map((c) => (
                  <div key={c.variable} style={{ width: "160px" }}>
                    <Swatch {...c} copy={copy} copied={copied} />
                  </div>
                ))}
              </div>
            </div>

            {/* border */}
            <div className="mb-7">
              <p
                className="text-muted-foreground uppercase tracking-widest mb-3"
                style={{ fontSize: "var(--text-xs)" }}
              >
                边框 · Border
              </p>
              <div className="flex flex-wrap gap-3">
                {borderColors.map((c) => (
                  <div key={c.variable} style={{ width: "160px" }}>
                    <Swatch {...c} copy={copy} copied={copied} />
                  </div>
                ))}
              </div>
            </div>

            {/* status */}
            <div>
              <p
                className="text-muted-foreground uppercase tracking-widest mb-3"
                style={{ fontSize: "var(--text-xs)" }}
              >
                状态色 · Status Colors
              </p>
              <div className="flex flex-col gap-2">
                {statusColors.map((s) => (
                  <div
                    key={s.variable}
                    className="flex items-center gap-4 px-4 py-3 rounded-xl border border-border bg-card"
                  >
                    <button
                      onClick={() => copy(s.variable)}
                      className="flex-shrink-0 w-8 h-8 rounded-lg border border-border transition-transform hover:scale-105"
                      style={{ background: `var(${s.variable})` }}
                      title={`复制 ${s.variable}`}
                    />
                    <button
                      onClick={() => copy(s.bg)}
                      className="flex-shrink-0 w-8 h-8 rounded-lg border border-border transition-transform hover:scale-105"
                      style={{ background: `var(${s.bg})` }}
                      title={`复制 ${s.bg}`}
                    />
                    <span
                      className="font-mono flex-shrink-0"
                      style={{ fontSize: "var(--text-sm)", color: "var(--foreground)", width: "8rem" }}
                    >
                      {s.label}
                    </span>
                    <span
                      className="px-3 py-1 rounded-full font-medium"
                      style={{
                        background: `var(${s.bg})`,
                        color: `var(${s.variable})`,
                        fontSize: "var(--text-xs)",
                        letterSpacing: "var(--ls-wide)",
                        textTransform: "uppercase",
                      }}
                    >
                      {s.label}
                    </span>
                    <div className="ml-auto flex items-center gap-6">
                      <div className="flex flex-col items-end">
                        <span className="text-muted-foreground" style={{ fontSize: "10px" }}>
                          前景色
                        </span>
                        <span className="font-mono text-foreground" style={{ fontSize: "var(--text-xs)" }}>
                          {s.variable}
                        </span>
                      </div>
                      <div className="flex flex-col items-end">
                        <span className="text-muted-foreground" style={{ fontSize: "10px" }}>
                          背景色
                        </span>
                        <span className="font-mono text-foreground" style={{ fontSize: "var(--text-xs)" }}>
                          {s.bg}
                        </span>
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </Section>

          {/* ════════════ SPACING ════════════ */}
          <Section id="spacing" title="间距体系" sub="Spacing Tokens">
            <div className="rounded-2xl border border-border bg-card overflow-hidden">
              <div className="flex items-center gap-4 px-4 py-2.5 border-b border-border bg-muted">
                <span className="text-muted-foreground uppercase tracking-widest" style={{ fontSize: "10px", width: "calc(4px + 4rem)" }}>
                  视觉
                </span>
                <span className="text-muted-foreground uppercase tracking-widest" style={{ fontSize: "10px", width: "5rem" }}>
                  Token
                </span>
                <span className="text-muted-foreground uppercase tracking-widest" style={{ fontSize: "10px", width: "4rem" }}>
                  rem
                </span>
                <span className="text-muted-foreground uppercase tracking-widest" style={{ fontSize: "10px" }}>
                  px
                </span>
              </div>
              {spaces.map((s) => (
                <SpaceRow key={s.token} {...s} copy={copy} copied={copied} />
              ))}
            </div>
          </Section>

          {/* ════════════ RADIUS ════════════ */}
          <Section id="radius" title="圆角体系" sub="Border Radius Tokens">
            <div className="rounded-2xl border border-border bg-card overflow-hidden">
              <div className="flex items-center gap-5 px-4 py-2.5 border-b border-border bg-muted">
                <span className="text-muted-foreground uppercase tracking-widest" style={{ fontSize: "10px", width: "40px" }}>
                  预览
                </span>
                <span className="text-muted-foreground uppercase tracking-widest" style={{ fontSize: "10px", width: "6rem" }}>
                  Token
                </span>
                <span className="text-muted-foreground uppercase tracking-widest" style={{ fontSize: "10px", width: "4.5rem" }}>
                  rem
                </span>
                <span className="text-muted-foreground uppercase tracking-widest" style={{ fontSize: "10px" }}>
                  说明
                </span>
              </div>
              {radii.map((r) => (
                <RadiusRow key={r.token} {...r} copy={copy} copied={copied} />
              ))}
            </div>
          </Section>

          {/* ════════════ SHADOW ════════════ */}
          <Section id="shadow" title="阴影体系" sub="Shadow Tokens">
            <div className="flex flex-wrap gap-4">
              {shadows.map((s) => (
                <div key={s.token} style={{ width: "calc(33.33% - 12px)", minWidth: "160px" }}>
                  <ShadowCard {...s} copy={copy} copied={copied} />
                </div>
              ))}
            </div>

            {/* shadow-custom utility */}
            <div className="mt-6 p-5 rounded-2xl border border-border bg-card">
              <p className="font-mono text-foreground font-medium mb-2" style={{ fontSize: "var(--text-sm)" }}>
                .shadow-custom
              </p>
              <p className="text-muted-foreground mb-4" style={{ fontSize: "var(--text-xs)" }}>
                由 4 个分量 Token 合成，修改分量变量即可全局调整
              </p>
              <div className="flex flex-wrap gap-3">
                {["--shadow-x", "--shadow-y", "--shadow-blur", "--shadow-spread", "--shadow-color"].map(
                  (v) => (
                    <button
                      key={v}
                      onClick={() => copy(v)}
                      className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border bg-muted hover:bg-accent transition-colors"
                    >
                      <span className="font-mono text-foreground" style={{ fontSize: "10px" }}>
                        {v}
                      </span>
                      {copied === v ? (
                        <Check size={11} style={{ color: "var(--status-success)" }} />
                      ) : (
                        <Copy size={11} style={{ color: "var(--muted-foreground)" }} />
                      )}
                    </button>
                  )
                )}
              </div>
            </div>
          </Section>

          {/* ════════════ TYPOGRAPHY ════════════ */}
          <Section id="typography" title="字体层级" sub="Typography Scale">
            {/* font families */}
            <div className="flex gap-4 mb-7">
              <div className="flex-1 p-5 rounded-2xl border border-border bg-card">
                <p className="text-muted-foreground uppercase tracking-widest mb-3" style={{ fontSize: "10px" }}>
                  --fontSans
                </p>
                <p
                  style={{
                    fontFamily: "var(--fontSans)",
                    fontSize: "var(--text-xl)",
                    fontWeight: "var(--fw-medium)",
                    color: "var(--foreground)",
                    lineHeight: "var(--lh-snug)",
                  }}
                >
                  Inter · PingFang SC
                </p>
                <p className="text-muted-foreground mt-1" style={{ fontSize: "var(--text-xs)" }}>
                  正文 / 界面 / 标注
                </p>
              </div>
              <div className="flex-1 p-5 rounded-2xl border border-border bg-card">
                <p className="text-muted-foreground uppercase tracking-widest mb-3" style={{ fontSize: "10px" }}>
                  --fontSerif
                </p>
                <p
                  style={{
                    fontFamily: "var(--fontSerif)",
                    fontSize: "var(--text-xl)",
                    fontWeight: "var(--fw-regular)",
                    color: "var(--foreground)",
                    lineHeight: "var(--lh-snug)",
                  }}
                >
                  Noto Serif SC · 宋体
                </p>
                <p className="text-muted-foreground mt-1" style={{ fontSize: "var(--text-xs)" }}>
                  展示标题 / 品牌字 / 装饰
                </p>
              </div>
            </div>

            {/* type scale */}
            <div className="rounded-2xl border border-border bg-card overflow-hidden mb-7">
              <div className="flex items-center gap-6 px-4 py-2.5 border-b border-border bg-muted">
                <span className="text-muted-foreground uppercase tracking-widest" style={{ fontSize: "10px", width: "8rem" }}>
                  名称
                </span>
                <span className="text-muted-foreground uppercase tracking-widest flex-1" style={{ fontSize: "10px" }}>
                  示例
                </span>
                <span className="text-muted-foreground uppercase tracking-widest" style={{ fontSize: "10px" }}>
                  变量
                </span>
              </div>
              {typeScale.map((t) => (
                <TypeRow key={t.name} {...t} />
              ))}
            </div>

            {/* font weights */}
            <div className="mb-7">
              <p className="text-muted-foreground uppercase tracking-widest mb-3" style={{ fontSize: "var(--text-xs)" }}>
                字重 · Font Weight
              </p>
              <div className="flex gap-3">
                {[
                  { token: "--fw-light", val: "300", label: "Light" },
                  { token: "--fw-regular", val: "400", label: "Regular" },
                  { token: "--fw-medium", val: "500", label: "Medium" },
                  { token: "--fw-semibold", val: "600", label: "Semibold" },
                ].map((fw) => (
                  <button
                    key={fw.token}
                    onClick={() => copy(fw.token)}
                    className="group flex-1 flex flex-col items-center gap-2 py-5 px-3 rounded-2xl border border-border bg-card hover:bg-accent transition-all"
                  >
                    <span
                      style={{
                        fontSize: "var(--text-2xl)",
                        fontWeight: fw.val,
                        color: "var(--foreground)",
                        lineHeight: "1",
                      }}
                    >
                      Aa
                    </span>
                    <span className="text-foreground" style={{ fontSize: "var(--text-sm)", fontWeight: fw.val }}>
                      {fw.label}
                    </span>
                    <span className="font-mono text-muted-foreground" style={{ fontSize: "10px" }}>
                      {fw.token}
                    </span>
                    <span className="opacity-0 group-hover:opacity-100 transition-opacity">
                      {copied === fw.token ? (
                        <Check size={12} style={{ color: "var(--status-success)" }} />
                      ) : (
                        <Copy size={12} style={{ color: "var(--muted-foreground)" }} />
                      )}
                    </span>
                  </button>
                ))}
              </div>
            </div>

            {/* line height */}
            <div>
              <p className="text-muted-foreground uppercase tracking-widest mb-3" style={{ fontSize: "var(--text-xs)" }}>
                行高 · Line Height
              </p>
              <div className="flex gap-3">
                {[
                  { token: "--lh-tight", val: "1.25", label: "Tight" },
                  { token: "--lh-snug", val: "1.4", label: "Snug" },
                  { token: "--lh-normal", val: "1.6", label: "Normal" },
                  { token: "--lh-relaxed", val: "1.8", label: "Relaxed" },
                ].map((lh) => (
                  <button
                    key={lh.token}
                    onClick={() => copy(lh.token)}
                    className="group flex-1 flex flex-col gap-1.5 p-4 rounded-2xl border border-border bg-card hover:bg-accent transition-all text-left"
                  >
                    <div
                      style={{
                        fontSize: "var(--text-xs)",
                        lineHeight: lh.val,
                        color: "var(--foreground)",
                        height: "3.6rem",
                        overflow: "hidden",
                      }}
                    >
                      雾感<br />玉石青<br />设计语言
                    </div>
                    <span className="text-foreground font-medium" style={{ fontSize: "var(--text-xs)" }}>
                      {lh.label}
                    </span>
                    <span className="font-mono text-muted-foreground" style={{ fontSize: "10px" }}>
                      {lh.val}
                    </span>
                    <span className="opacity-0 group-hover:opacity-100 transition-opacity">
                      {copied === lh.token ? (
                        <Check size={12} style={{ color: "var(--status-success)" }} />
                      ) : (
                        <Copy size={12} style={{ color: "var(--muted-foreground)" }} />
                      )}
                    </span>
                  </button>
                ))}
              </div>
            </div>
          </Section>

          {/* ── footer ── */}
          <div className="mt-4 pt-8 border-t border-border flex items-center justify-between">
            <p className="font-serif italic text-muted-foreground" style={{ fontSize: "var(--text-sm)" }}>
              雾感玉石青 · Jade Mist Teal Design Tokens
            </p>
            <p className="text-muted-foreground" style={{ fontSize: "var(--text-xs)" }}>
              基于 oklch 色彩空间 · 生产就绪
            </p>
          </div>

        </div>
      </main>
    </div>
  );
}
