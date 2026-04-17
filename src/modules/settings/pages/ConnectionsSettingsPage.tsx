import { cn } from '@/lib/utils'
import { SettingsSurface } from '../components/SettingsSurface'
import type { SettingsPageProps } from '../types'

// ── Channel catalog ──────────────────────────────────────────────────────────

type ChannelStatus = 'connected' | 'disconnected' | 'pending'

interface Channel {
  id: string
  name: string
  description: string
  icon: string
  status: ChannelStatus
  priority: 'high' | 'low'
}

const CHANNELS: Channel[] = [
  {
    id: 'feishu',
    name: '飞书 / Lark',
    description: '飞书机器人消息收发与互动',
    icon: '🪶',
    status: 'connected',
    priority: 'high',
  },
  {
    id: 'wechat',
    name: '微信',
    description: '微信服务号与企业微信消息通道',
    icon: '💬',
    status: 'disconnected',
    priority: 'high',
  },
  {
    id: 'qq',
    name: 'QQ Bot',
    description: 'QQ 频道机器人消息收发',
    icon: '🐧',
    status: 'disconnected',
    priority: 'high',
  },
  {
    id: 'dingtalk',
    name: '钉钉',
    description: '钉钉机器人回调与消息推送',
    icon: '📌',
    status: 'disconnected',
    priority: 'high',
  },
  {
    id: 'telegram',
    name: 'Telegram',
    description: 'Telegram Bot API 消息通道',
    icon: '✈️',
    status: 'disconnected',
    priority: 'high',
  },
  {
    id: 'discord',
    name: 'Discord',
    description: 'Discord Bot 消息与事件',
    icon: '🎮',
    status: 'disconnected',
    priority: 'low',
  },
  {
    id: 'slack',
    name: 'Slack',
    description: 'Slack Bot 消息收发',
    icon: '📣',
    status: 'disconnected',
    priority: 'low',
  },
  {
    id: 'teams',
    name: 'Microsoft Teams',
    description: 'Teams 连接器消息推送',
    icon: '🔵',
    status: 'disconnected',
    priority: 'low',
  },
  {
    id: 'line',
    name: 'LINE',
    description: 'LINE Messaging API 消息通道',
    icon: '🟢',
    status: 'disconnected',
    priority: 'low',
  },
  {
    id: 'signal',
    name: 'Signal',
    description: 'Signal CLI 消息收发',
    icon: '📡',
    status: 'disconnected',
    priority: 'low',
  },
  {
    id: 'imessage',
    name: 'iMessage',
    description: 'iMessage 消息桥接',
    icon: '💙',
    status: 'disconnected',
    priority: 'low',
  },
  {
    id: 'whatsapp',
    name: 'WhatsApp',
    description: 'WhatsApp Business API',
    icon: '📱',
    status: 'disconnected',
    priority: 'low',
  },
  {
    id: 'mattermost',
    name: 'Mattermost',
    description: 'Mattermost 插件消息',
    icon: '🟣',
    status: 'disconnected',
    priority: 'low',
  },
  {
    id: 'matrix',
    name: 'Matrix',
    description: 'Matrix 协议消息桥接',
    icon: '🟠',
    status: 'disconnected',
    priority: 'low',
  },
]

const highPriority = CHANNELS.filter((c) => c.priority === 'high')
const lowPriority = CHANNELS.filter((c) => c.priority === 'low')

function StatusDot({ status }: { status: ChannelStatus }) {
  const cls: Record<ChannelStatus, string> = {
    connected: 'bg-emerald-500',
    disconnected: 'bg-black/15',
    pending: 'bg-amber-400',
  }
  return <span className={cn('inline-block h-1.5 w-1.5 rounded-full', cls[status])} />
}

function ChannelCard({ channel, onConfigure }: { channel: Channel; onConfigure: (id: string) => void }) {
  const isConnected = channel.status === 'connected'

  return (
    <div className="group flex items-center justify-between rounded-xl border border-black/[0.06] bg-black/[0.016] px-3.5 py-3 transition-colors hover:bg-black/[0.03]">
      <div className="flex items-center gap-3">
        <div className="flex size-9 shrink-0 items-center justify-center rounded-xl bg-white text-base shadow-sm border border-black/[0.07]">
          {channel.icon}
        </div>
        <div className="min-w-0">
          <div className="flex items-center gap-1.5">
            <span className="text-[12.5px] font-semibold tracking-tight">{channel.name}</span>
            <StatusDot status={channel.status} />
          </div>
          <p className="mt-0.5 truncate text-[10.5px] text-muted-foreground">{channel.description}</p>
        </div>
      </div>

      <button
        type="button"
        onClick={() => onConfigure(channel.id)}
        className={cn(
          'window-no-drag ml-3 shrink-0 rounded-xl px-3 py-1 text-[11.5px] font-semibold transition-colors',
          isConnected
            ? 'border border-black/[0.09] bg-white text-foreground/70 hover:bg-black/[0.03]'
            : 'bg-jade text-white hover:bg-jade/90',
        )}
      >
        {isConnected ? '管理' : '接入'}
      </button>
    </div>
  )
}

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="mb-3 text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
      {children}
    </div>
  )
}

// ── Page ──────────────────────────────────────────────────────────────────────

export function ConnectionsSettingsPage({}: SettingsPageProps) {
  const handleConfigure = (id: string) => {
    console.log(`[Connections] Configure channel: ${id}`)
  }

  return (
    <div className="flex flex-col gap-3">
      {/* Priority channels */}
      <SettingsSurface className="px-5 py-4">
        <SectionLabel>优先推荐</SectionLabel>
        <div className="flex flex-col gap-2">
          {highPriority.map((ch) => (
            <ChannelCard key={ch.id} channel={ch} onConfigure={handleConfigure} />
          ))}
        </div>
      </SettingsSurface>

      {/* Other channels */}
      <SettingsSurface className="px-5 py-4">
        <SectionLabel>其他渠道</SectionLabel>
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
          {lowPriority.map((ch) => (
            <ChannelCard key={ch.id} channel={ch} onConfigure={handleConfigure} />
          ))}
        </div>
      </SettingsSurface>

      {/* Info */}
      <SettingsSurface className="px-5 py-3.5">
        <div className="flex items-start gap-2.5">
          <div className="mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full bg-jade/10 text-[9px] font-bold text-jade">
            i
          </div>
          <p className="text-[11.5px] leading-5 text-muted-foreground">
            所有连接能力都会优先保持在本地工作区内，后续可以逐步接入更细的权限控制和审计记录。
          </p>
        </div>
      </SettingsSurface>
    </div>
  )
}
