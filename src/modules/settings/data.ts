import { BarChart3, Info, Link2, Radio, Search, Settings, Sparkles } from 'lucide-react'
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
    id: 'web-search',
    label: 'Web Search',
    description: '配置 web_search 工具的搜索服务商及 API Key，支持 Tavily、Brave、Serper、SearXNG。',
    icon: Search,
  },
  {
    id: 'connections',
    label: '连接应用',
    description: '配置外部应用与服务连接。',
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
] as const

