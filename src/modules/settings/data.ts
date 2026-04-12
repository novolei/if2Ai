import { BarChart3, Info, Link2, Radio, Settings, Sparkles } from 'lucide-react'
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
    description: '启用、关闭和排序可用技能。',
    icon: Sparkles,
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

