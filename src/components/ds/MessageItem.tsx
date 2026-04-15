import { ChevronDown, Copy } from "lucide-react";

// ─── Types ────────────────────────────────────────────────────────────────────

export interface MessageItemProps {
  /** Message id */
  id?: string;
  /** Who sent the message */
  role?: "user" | "assistant";
  /** Message body (supports \n line breaks) */
  content?: string;
  /** Timestamp label */
  time?: string;
  /** Inner thinking / reasoning text */
  thinking?: string;
  /** Whether thinking block is expanded */
  thinkingExpanded?: boolean;
  /** Toggle thinking callback */
  onToggleThinking?: () => void;
}

// ─── Component ────────────────────────────────────────────────────────────────

/**
 * MessageItem — single chat message bubble.
 *
 * Assistant mode: optional collapsible thinking block + full-width message body + meta row.
 * User mode: right-aligned bubble with rounded-2xl shape.
 *
 * All colors driven by CSS variables (status-active-bg, muted-foreground, secondary, etc.).
 */
export default function MessageItem({
  id: _id            = "msg",
  role            = "assistant",
  content         = "Hello! How can I help you?",
  time            = "",
  thinking,
  thinkingExpanded = false,
  onToggleThinking = () => {},
}: MessageItemProps) {
  const lines = content.split("\n");

  if (role === "assistant") {
    return (
      <div
        className="flex flex-col items-start w-full group"
      >
        {/* Thinking block */}
        {thinking && (
          <div className="mb-3 w-full">
            <button
              onClick={onToggleThinking}
              className="flex items-center gap-1.5 text-[11px] text-muted-foreground hover:text-foreground/70 transition-colors duration-150 mb-1"
            >
              <span className="w-1.5 h-1.5 rounded-full bg-status-active shrink-0" />
              <span>已完成思考</span>
              <ChevronDown
                size={11}
                className={`transition-transform duration-200 ${thinkingExpanded ? "rotate-180" : ""}`}
              />
            </button>
            {thinkingExpanded && (
              <div className="border-l-2 border-border pl-3 ml-1 py-1">
                {thinking.split("\n").map((line, i) => (
                  <p
                    key={i}
                    className={`text-[11px] text-muted-foreground leading-relaxed italic ${line === "" ? "h-2" : ""}`}
                  >
                    {line}
                  </p>
                ))}
              </div>
            )}
          </div>
        )}

        {/* Body */}
        <div
          className="text-foreground/85 w-full"
          style={{ fontSize: "var(--text-sm)", lineHeight: "var(--lh-relaxed)" }}
        >
          {lines.map((line, i) => (
            <p key={i} className={line === "" ? "h-3" : ""}>{line}</p>
          ))}
        </div>

        {/* Meta row */}
        <div className="flex items-center gap-2 mt-2 opacity-0 group-hover:opacity-100 transition-opacity duration-150">
          {time && <span className="text-[11px] text-muted-foreground">{time}</span>}
          <button
            className="w-5 h-5 flex items-center justify-center rounded text-muted-foreground/60 hover:text-muted-foreground hover:bg-accent transition-colors duration-100"
            aria-label="Copy"
          >
            <Copy size={11} />
          </button>
        </div>
      </div>
    );
  }

  // ── User bubble ──
  return (
    <div
      className="flex flex-col items-end w-full group"
    >
      <div
        className="px-4 py-2.5 rounded-2xl rounded-tr-sm text-foreground/85 bg-secondary"
        style={{
          fontSize: "var(--text-sm)",
          lineHeight: "var(--lh-relaxed)",
          maxWidth: 360,
        }}
      >
        {content}
      </div>
      <div className="flex items-center gap-2 mt-1.5 opacity-0 group-hover:opacity-100 transition-opacity duration-150">
        {time && <span className="text-[11px] text-muted-foreground">{time}</span>}
        <button
          className="w-5 h-5 flex items-center justify-center rounded text-muted-foreground/60 hover:text-muted-foreground hover:bg-accent transition-colors duration-100"
          aria-label="Copy"
        >
          <Copy size={11} />
        </button>
      </div>
    </div>
  );
}
