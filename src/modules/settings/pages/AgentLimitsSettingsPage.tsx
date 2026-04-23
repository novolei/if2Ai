import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { Copy } from "lucide-react";
import { Button } from "@/components/ui/button";
import { SettingsSurface } from "../components/SettingsSurface";
import { useContextBarMode } from "@/components/chat/useContextBarMode";

type ThinkingPolicy = "auto" | "force-on" | "force-off";
const THINKING_POLICY_KEY = "IF2AI_THINKING_MODE_OVERRIDE";

function readThinkingPolicy(): ThinkingPolicy {
  if (typeof window === "undefined") return "auto";
  const raw = window.localStorage.getItem(THINKING_POLICY_KEY) ?? "auto";
  return raw === "force-on" || raw === "force-off" ? raw : "auto";
}

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="mb-3 text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
      {children}
    </div>
  );
}

const SNIPPET = `# Cost guard (P0-3) — 单位：美分/次；留空表示不限制
# export IF2AI_COST_MAX_PER_DAY_CENTS=1000
# export IF2AI_COST_MAX_LLM_CALLS_PER_HOUR=120
# export IF2AI_COST_MAX_PER_SESSION_DAY_CENTS=500
# export IF2AI_COST_ESTIMATE_PER_LLM_CALL_CENTS=1

# Smart routing (P1-8)
# export IF2AI_SMART_ROUTING=1
# export IF2AI_CHEAP_MODEL_ID=gpt-4o-mini
# export IF2AI_SMART_ROUTE_THRESHOLD=0.35

# Event log redaction (P2-14) — 设为 1 关闭脱敏
# export IF2AI_DISABLE_EVENT_LOG_REDACTION=0

# Transcript undo (P2-10) — 默认开启；设为 0 关闭
# export IF2AI_CONVERSATION_UNDO=0
`;

/**
 * Reference panel for Steward-adoption env knobs (cost, routing, redaction, undo).
 * Values apply after app restart when set in the shell or launchd plist.
 */
export function AgentLimitsSettingsPage() {
  const [copied, setCopied] = useState(false);
  const [barMode, setBarMode] = useContextBarMode();
  const [thinkingPolicy, setThinkingPolicy] = useState<ThinkingPolicy>(readThinkingPolicy);

  useEffect(() => {
    const handler = (e: StorageEvent) => {
      if (e.key === THINKING_POLICY_KEY) setThinkingPolicy(readThinkingPolicy());
    };
    window.addEventListener("storage", handler);
    return () => window.removeEventListener("storage", handler);
  }, []);

  const updatePolicy = useCallback((next: ThinkingPolicy) => {
    if (typeof window !== "undefined") {
      window.localStorage.setItem(THINKING_POLICY_KEY, next);
      window.dispatchEvent(
        new StorageEvent("storage", { key: THINKING_POLICY_KEY, newValue: next }),
      );
    }
    setThinkingPolicy(next);
  }, []);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(SNIPPET);
      setCopied(true);
      toast.success("已复制示例 export 片段");
      setTimeout(() => setCopied(false), 2000);
    } catch {
      toast.error("复制失败");
    }
  }, []);

  return (
    <div className="flex flex-col gap-4">
      <SettingsSurface className="px-5 py-4">
        <SectionLabel>ContextBar 显示模式</SectionLabel>
        <p className="mb-3 text-[12.5px] leading-relaxed text-black/55">
          聊天上方的上下文条默认按 <code className="rounded bg-black/[0.06] px-1">系统/记忆/历史/输出</code>{" "}
          四段显示「下一轮请求的预算分配」。其中
          <strong className="text-black/75"> 系统/记忆/输出 </strong>
          基本是跨 session 的常量，不同会话看起来很相似。切换为
          <strong className="text-black/75"> 仅会话 </strong>
          模式后，上述三段折叠为一个浅灰「基线」段，把
          <strong className="text-black/75"> 历史 </strong>
          段（每个 session 独有）放大显示，便于一眼看出 per-session 体量差异。
        </p>
        <div className="mb-2 flex items-center gap-2">
          <Button
            type="button"
            variant={barMode === "full" ? "default" : "outline"}
            size="sm"
            className="rounded-xl"
            onClick={() => setBarMode("full")}
          >
            完整模式（默认）
          </Button>
          <Button
            type="button"
            variant={barMode === "session-only" ? "default" : "outline"}
            size="sm"
            className="rounded-xl"
            onClick={() => setBarMode("session-only")}
          >
            仅会话历史
          </Button>
        </div>
        <p className="text-[11.5px] leading-relaxed text-black/45">
          当前选择：
          <code className="ml-1 rounded bg-black/[0.06] px-1">
            IF2AI_CONTEXT_BAR_MODE={barMode}
          </code>
          ；切换即时生效，配置保存在浏览器
          <code className="mx-1 rounded bg-black/[0.06] px-1">localStorage</code>
          ，无需重启。
        </p>
      </SettingsSurface>
      <SettingsSurface className="px-5 py-4">
        <SectionLabel>思维模式策略 (Reasoning Override)</SectionLabel>
        <p className="mb-3 text-[12.5px] leading-relaxed text-black/55">
          控制后端是否给请求体写入 <code className="rounded bg-black/[0.06] px-1">reasoning_content</code>
          、<code className="rounded bg-black/[0.06] px-1">enable_thinking</code>、
          <code className="rounded bg-black/[0.06] px-1">reasoning_effort</code> 等思考字段。默认按
          模型词典自动判定（Kimi-thinking / DeepSeek-R1 / o1 等）；
          <strong className="text-black/75"> 强制开启 </strong> 用于排错；
          <strong className="text-black/75"> 强制关闭 </strong>
          用于绕开供应商对思考字段的拒绝。
        </p>
        <div className="mb-2 flex items-center gap-2">
          <Button
            type="button"
            variant={thinkingPolicy === "auto" ? "default" : "outline"}
            size="sm"
            className="rounded-xl"
            onClick={() => updatePolicy("auto")}
          >
            自动（默认）
          </Button>
          <Button
            type="button"
            variant={thinkingPolicy === "force-on" ? "default" : "outline"}
            size="sm"
            className="rounded-xl"
            onClick={() => updatePolicy("force-on")}
          >
            强制开启
          </Button>
          <Button
            type="button"
            variant={thinkingPolicy === "force-off" ? "default" : "outline"}
            size="sm"
            className="rounded-xl"
            onClick={() => updatePolicy("force-off")}
          >
            强制关闭
          </Button>
        </div>
        <p className="text-[11.5px] leading-relaxed text-black/45">
          当前选择：
          <code className="ml-1 rounded bg-black/[0.06] px-1">
            IF2AI_THINKING_MODE_OVERRIDE={thinkingPolicy}
          </code>
          ；同时建议在启动环境里 <code className="mx-1 rounded bg-black/[0.06] px-1">
            export IF2AI_THINKING_MODE_OVERRIDE={thinkingPolicy}
          </code>
          以便后端立即生效（前端 localStorage 仅用于 UI 状态记忆）。
        </p>
      </SettingsSurface>
      <SettingsSurface className="px-5 py-4">
        <SectionLabel>治理与成本（环境变量）</SectionLabel>
        <p className="mb-3 text-[12.5px] leading-relaxed text-black/55">
          下列开关由后端读取，需写入启动环境后重启应用生效。此处为说明与一键复制模板，不在应用内持久化。
        </p>
        <ul className="mb-4 list-disc space-y-1.5 pl-5 text-[12px] text-black/60">
          <li>
            <strong className="text-black/75">CostGuard</strong>：日预算、每小时 LLM
            调用上限、每会话日 cap（见代码{" "}
            <code className="rounded bg-black/[0.06] px-1">cost_guard.rs</code>）。
          </li>
          <li>
            <strong className="text-black/75">Smart routing</strong>：低复杂度走 cheap
            模型（<code className="rounded bg-black/[0.06] px-1">IF2AI_CHEAP_MODEL_ID</code>
            ）。
          </li>
          <li>
            <strong className="text-black/75">脱敏</strong>：事件日志默认脱敏；可用{" "}
            <code className="rounded bg-black/[0.06] px-1">
              IF2AI_DISABLE_EVENT_LOG_REDACTION
            </code>{" "}
            关闭（高风险）。
          </li>
          <li>
            <strong className="text-black/75">Undo</strong>：会话级撤销默认开启；{" "}
            <code className="rounded bg-black/[0.06] px-1">IF2AI_CONVERSATION_UNDO=0</code>{" "}
            可关闭。
          </li>
        </ul>
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="gap-2 rounded-xl"
          onClick={() => void handleCopy()}
        >
          <Copy className="h-3.5 w-3.5" />
          {copied ? "已复制" : "复制 export 模板"}
        </Button>
      </SettingsSurface>
    </div>
  );
}
