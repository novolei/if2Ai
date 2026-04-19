import {
  BarChart3,
  Brain,
  Cpu,
  Globe,
  Info,
  Link2,
  Mic,
  Radio,
  Search,
  Settings,
  Sparkles,
  Volume2,
} from 'lucide-react'
import type { SettingsSectionMeta } from './types'

export const SETTINGS_SECTIONS: SettingsSectionMeta[] = [
  {
    id: 'general',
    label: '通用设置',
    description: '语言、启动、字体与基础行为。',
    icon: Settings,
  },
  {
    id: 'usage',
    label: '用量统计',
    description: '查看会话、消息和资源消耗概览。',
    icon: BarChart3,
  },
  {
    id: 'skills',
    label: '技能管理',
    description: '查看状态并执行 enable/disable，含 quarantine/active 冲突说明。',
    icon: Sparkles,
  },
  {
    id: 'tools',
    label: '工具设置',
    description: '配置 AI 可用工具的运行参数（浏览器 profile / cookies、未来更多工具）。',
    icon: Globe,
  },
  {
    id: 'web-search',
    label: 'Web Search',
    description: '配置 web_search 工具的搜索服务商及 API Key，支持 Tavily、Brave、Serper、SearXNG。',
    icon: Search,
  },
  {
    id: 'memory',
    label: '记忆配置',
    description: '管理记忆 Token 预算分配与轨迹导出。',
    icon: Brain,
  },
  {
    id: 'model',
    label: '模型配置',
    description: '配置本地向量化模型与 LLM 服务商设置。',
    icon: Cpu,
  },
  {
    id: 'connections',
    label: '连接应用',
    description: '接入外部社交渠道与消息平台。',
    icon: Link2,
  },
  {
    id: 'remote',
    label: '远控通道',
    description: '管理远程通道与桌面协作。',
    icon: Radio,
  },
  {
    id: 'about',
    label: '关于我们',
    description: '版本、理念与项目说明。',
    icon: Info,
  },
  {
    id: 'tts-test',
    label: 'TTS 测试',
    description: 'MOSS-TTS-Nano 语音合成测试面板。',
    icon: Volume2,
  },
  {
    id: 'stt-config',
    label: 'STT 语音输入',
    description: 'Whisper 本地 / Groq 云端语音转文字配置。',
    icon: Mic,
  },
] as const

