import { useState } from "react";
import { Plus, Mic, Send, Cpu, Globe, GitBranch, ChevronDown, SlidersHorizontal } from "lucide-react";

// ─── Types ────────────────────────────────────────────────────────────────────

export interface InputComposerProps {
  /** Controlled value */
  value?: string;
  /** Change callback */
  onChange?: (val: string) => void;
  /** Submit callback */
  onSubmit?: (val: string) => void;
  /** Input placeholder */
  placeholder?: string;
  /** Model display name */
  modelName?: string;
  /** Branch name */
  branchName?: string;
  /** Whether the input is disabled */
  disabled?: boolean;
}

// ─── Component ────────────────────────────────────────────────────────────────

/**
 * InputComposer — chat input box with attach, mic, send buttons and meta bar.
 *
 * States:
 * - empty: send button bg-muted, text-muted-foreground
 * - has-text: send button bg-jade, text-primary-foreground
 * - focused: container border-ring + shadow-ring-jade
 *
 * Uses Paico tokens: bg-jade, bg-card, border-input, shadow-token-sm, etc.
 */
export default function InputComposer({
  value       = "",
  onChange    = () => {},
  onSubmit    = () => {},
  placeholder = "输入消息…",
  modelName   = "GPT-5.4-Mini",
  branchName  = `feature/main`,
  disabled    = false,
}: InputComposerProps) {
  const [internal, setInternal] = useState(value);
  const current = value !== undefined ? value : internal;

  const handleChange = (v: string) => {
    setInternal(v);
    onChange(v);
  };

  const handleSubmit = () => {
    if (!current.trim() || disabled) return;
    onSubmit(current);
    setInternal("");
    onChange("");
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  return (
    <div
      className={disabled ? "opacity-60 pointer-events-none" : ""}
    >
      {/* ── Input box ── */}
      <div className="flex items-center gap-3 rounded-xl border border-input px-4 py-2.5 bg-card shadow-token-sm transition-shadow duration-150 focus-within:border-ring focus-within:shadow-ring-jade">
        {/* Attach button */}
        <button
          className="w-6 h-6 flex items-center justify-center rounded-md text-muted-foreground hover:text-jade transition-colors duration-150 shrink-0"
          aria-label="Attach"
        >
          <Plus size={16} />
        </button>

        {/* Text input */}
        <input
          type="text"
          value={current}
          onChange={(e) => handleChange(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          className="flex-1 bg-transparent text-foreground placeholder:text-muted-foreground outline-none text-[13px]"
        />

        {/* Mic button */}
        <button
          className="w-6 h-6 flex items-center justify-center rounded-md text-muted-foreground hover:text-jade transition-colors duration-150 shrink-0"
          aria-label="Voice input"
        >
          <Mic size={14} />
        </button>

        {/* Send button */}
        <button
          onClick={handleSubmit}
          aria-label="Send"
          className={`w-7 h-7 flex items-center justify-center rounded-lg transition-all duration-150 shrink-0 ${
            current.trim()
              ? "bg-jade text-primary-foreground hover:opacity-90 active:scale-95"
              : "bg-muted text-muted-foreground cursor-not-allowed"
          }`}
        >
          <Send size={13} />
        </button>
      </div>

      {/* ── Meta bar ── */}
      <div className="flex items-center justify-between mt-2 px-1">
        {/* Left: model + language */}
        <div className="flex items-center gap-3">
          <button className="flex items-center gap-1.5 text-[11px] text-muted-foreground hover:text-foreground transition-colors duration-150">
            <Cpu size={11} />
            {modelName}
            <ChevronDown size={10} />
          </button>
          <div className="h-3 w-px bg-border" />
          <button className="flex items-center gap-1.5 text-[11px] text-muted-foreground hover:text-foreground transition-colors duration-150">
            <Globe size={11} />
            中
            <ChevronDown size={10} />
          </button>
        </div>

        {/* Right: access level + branch + settings */}
        <div className="flex items-center gap-3">
          <span className="text-[11px] text-muted-foreground">完全访问权限</span>
          <div className="h-3 w-px bg-border" />
          <button className="flex items-center gap-1.5 text-[11px] text-muted-foreground hover:text-foreground transition-colors duration-150">
            <GitBranch size={11} />
            <span className="max-w-[120px] truncate">{branchName}</span>
            <ChevronDown size={10} />
          </button>
          <div className="h-3 w-px bg-border" />
          <button className="w-5 h-5 flex items-center justify-center rounded text-muted-foreground hover:text-foreground hover:bg-accent transition-colors duration-100">
            <SlidersHorizontal size={12} />
          </button>
        </div>
      </div>
    </div>
  );
}
