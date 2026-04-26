import {
  BarChart3,
  Bot,
  Brain,
  Cpu,
  Gauge,
  Globe,
  Info,
  Link2,
  Mic,
  Plug,
  Radio,
  Search,
  Settings,
  Sparkles,
  Volume2,
} from "lucide-react";
import type { SettingsSectionMeta } from "./types";

export const SETTINGS_SECTIONS: SettingsSectionMeta[] = [
  {
    id: "general",
    label: "通用设置",
    description: "语言、启动、字体与基础行为。",
    icon: Settings,
  },
  {
    id: "agent-identity",
    label: "Agent Identity",
    description:
      "选择默认 Soul / Persona，并查看当前身份定义如何投影到助手行为。",
    icon: Bot,
  },
  {
    id: "usage",
    label: "用量统计",
    description: "查看会话、消息和资源消耗概览。",
    icon: BarChart3,
  },
  {
    id: "agent-limits",
    label: "治理与成本",
    description: "CostGuard、智能路由、脱敏与撤销相关环境变量说明。",
    icon: Gauge,
  },
  {
    id: "skills",
    label: "技能管理",
    description:
      "查看状态并执行 enable/disable，含 quarantine/active 冲突说明。",
    icon: Sparkles,
  },
  {
    id: "tools",
    label: "工具设置",
    description:
      "配置 AI 可用工具的运行参数（浏览器 profile / cookies、未来更多工具）。",
    icon: Globe,
  },
  {
    id: "mcp-services",
    label: "MCP 服务",
    description: "管理 stdio / remote MCP 服务，作为 agentic tool runtime 的外部能力层。",
    icon: Plug,
  },
  {
    id: "web-search",
    label: "Web Search",
    description:
      "配置 web_search 工具的搜索服务商及 API Key，支持 Tavily、Brave、Serper、SearXNG。",
    icon: Search,
  },
  {
    id: "memory",
    label: "记忆配置",
    description: "管理记忆 Token 预算分配与轨迹导出。",
    icon: Brain,
  },
  {
    id: "model",
    label: "模型配置",
    description: "配置本地向量化模型与 LLM 服务商设置。",
    icon: Cpu,
  },
  {
    id: "providers",
    label: "服务商管理",
    description:
      "OAuth / Coding Plan / API 三段式管理 LLM 服务商，按 openhanako 模式查看 Base URL、API 类型、推理能力与已添加模型。",
    icon: Cpu,
  },
  {
    id: "connections",
    label: "连接应用",
    description: "接入外部社交渠道与消息平台。",
    icon: Link2,
  },
  {
    id: "remote",
    label: "远控通道",
    description: "管理远程通道与桌面协作。",
    icon: Radio,
  },
  {
    id: "strategy-diagnostics",
    label: "策略治理诊断",
    description:
      "M5 candidate registry / active strategy / rollback 审计面（治理诊断，非营销面板）。",
    icon: BarChart3,
  },
  {
    id: "prompt-diagnostics",
    label: "Prompt Diagnostics",
    description:
      "查看最近一轮 prompt control plane 的 lane / entry / reason 摘要。",
    icon: Sparkles,
  },
  {
    id: "about",
    label: "关于我们",
    description: "版本、理念与项目说明。",
    icon: Info,
  },
  {
    id: "tts-settings",
    label: "语音合成 (TTS)",
    description: "语速、音质预设、采样参数。所有 AI 朗读 / 消息播报均生效。",
    icon: Volume2,
  },
  {
    id: "tts-profiles",
    label: "TTS Profiles",
    description: '一键切换"音色 + 语速 + 文本润色"组合；聊天侧仅显示 Profile。',
    icon: Volume2,
  },
  {
    id: "tts-test",
    label: "TTS 测试",
    description: "MOSS-TTS-Nano 合成调试面板（暴露完整参数）。",
    icon: Volume2,
  },
  {
    id: "stt-config",
    label: "STT 语音输入",
    description: "SenseVoice 本地语音转文字模型管理。",
    icon: Mic,
  },
] as const;
