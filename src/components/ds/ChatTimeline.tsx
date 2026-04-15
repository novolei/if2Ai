import { useState } from "react";
import MessageItem from "./MessageItem";

// ─── Types ────────────────────────────────────────────────────────────────────

export interface ChatMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
  time?: string;
  thinking?: string;
}

export interface ChatTimelineProps {
  /** Array of messages to display */
  messages?: ChatMessage[];
  /** Max width of the message column (px or tailwind value) */
  maxWidth?: number;
}

// ─── Component ────────────────────────────────────────────────────────────────

/**
 * ChatTimeline — scrollable message container with centered column.
 *
 * Uses `bg-background` and `scrollbar-thin` utility from the Paico design system.
 * Messages are rendered via `MessageItem` with collapsible thinking blocks.
 */
export default function ChatTimeline({
  messages = [],
  maxWidth = 672,
}: ChatTimelineProps) {
  const [expandedThinking, setExpandedThinking] = useState<Record<string, boolean>>({});

  const toggleThinking = (id: string) => {
    setExpandedThinking((prev) => ({ ...prev, [id]: !prev[id] }));
  };

  return (
    <div
      className="flex-1 overflow-y-auto scrollbar-thin px-6 py-5 bg-background"
    >
      <div
        className="mx-auto flex flex-col gap-6"
        style={{ maxWidth }}
      >
        {messages.map((msg) => (
          <MessageItem
            key={msg.id}
            id={msg.id}
            role={msg.role}
            content={msg.content}
            time={msg.time}
            thinking={msg.thinking}
            thinkingExpanded={!!expandedThinking[msg.id]}
            onToggleThinking={() => toggleThinking(msg.id)}
          />
        ))}
      </div>
    </div>
  );
}
