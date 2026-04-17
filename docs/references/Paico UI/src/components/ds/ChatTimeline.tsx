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
/*
 * Structure
 * ┌─────────────────────────────────────────────────────────┐
 * │  scroll container (flex-1, overflow-y-auto)             │
 * │  ┌───────────────────────────────────────────────────┐  │
 * │  │  max-w-2xl column, mx-auto                        │  │
 * │  │  ┌─ MessageItem (assistant) ───────────────────┐  │  │
 * │  │  └─────────────────────────────────────────────┘  │  │
 * │  │  ┌─ MessageItem (user) ────────────────────────┐  │  │
 * │  │  └─────────────────────────────────────────────┘  │  │
 * │  └───────────────────────────────────────────────────┘  │
 * └─────────────────────────────────────────────────────────┘
 *
 * Padding / Spacing
 * • px-6 py-5 (24px / 20px) from container edges
 * • gap-6 (24px) between messages
 *
 * Colors
 * • container bg : bg-background
 * • scrollbar    : scrollbar-thin (border token)
 */
export default function ChatTimeline({
  messages = [
    {
      id: "m1",
      role: "assistant",
      content: `你好啊！👋\n\n有什么我可以帮你的吗？无论是写代码、查资料、分析数据，随时告诉我！`,
      time: "18:12",
      thinking: `I'll respond with a friendly greeting and offer assistance.`,
    },
    {
      id: "m2",
      role: "user",
      content: "你好，帮我看看这段代码",
      time: "18:13",
    },
    {
      id: "m3",
      role: "assistant",
      content: `当然！请把代码发给我，我来帮你分析。😊`,
      time: "18:13",
      thinking: `User wants help with code. I should acknowledge and ask them to share it.`,
    },
  ],
  maxWidth = 672,
}: ChatTimelineProps) {
  const [expandedThinking, setExpandedThinking] = useState<Record<string, boolean>>({});

  const toggleThinking = (id: string) => {
    setExpandedThinking((prev) => ({ ...prev, [id]: !prev[id] }));
    console.log("ChatTimeline: toggle thinking for message", id);
  };

  return (
    <div
      data-cmp="ChatTimeline"
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
