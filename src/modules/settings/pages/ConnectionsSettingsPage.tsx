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
  // High priority
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
  // Low priority (other channels)
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

function StatusBadge({ status }: { status: ChannelStatus }) {
  const styles: Record<ChannelStatus, { bg: string; text: string; dot: string; label: string }> = {
    connected: {
      bg: 'bg-emerald-50',
      text: 'text-emerald-700',
      dot: 'bg-emerald-500',
      label: '已连接',
    },
    disconnected: {
      bg: 'bg-muted',
      text: 'text-muted-foreground',
      dot: 'bg-muted-foreground/40',
      label: '未连接',
    },
    pending: {
      bg: 'bg-amber-50',
      text: 'text-amber-700',
      dot: 'bg-amber-500',
      label: '待配置',
    },
  }
  const s = styles[status]
  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-[11px] font-medium tracking-wide ${s.bg} ${s.text}`}
    >
      <span className={`h-1.5 w-1.5 rounded-full ${s.dot}`} />
      {s.label}
    </span>
  )
}

function ChannelCard({
  channel,
  onConfigure,
}: {
  channel: Channel
  onConfigure: (id: string) => void
}) {
  const isConnected = channel.status === 'connected'

  return (
    <div className="group relative flex items-center justify-between rounded-xl border border-border/50 bg-surface-raised px-4 py-3.5 shadow-token-xs transition-all hover:shadow-token-sm hover:border-primary/20">
      <div className="flex items-center gap-3.5">
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-muted text-lg">
          {channel.icon}
        </div>
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <span className="text-[13px] font-semibold tracking-tight">{channel.name}</span>
            <StatusBadge status={channel.status} />
          </div>
          <p className="mt-0.5 truncate text-[11px] leading-4 text-muted-foreground">
            {channel.description}
          </p>
        </div>
      </div>

      <button
        type="button"
        onClick={() => onConfigure(channel.id)}
        className={`window-no-drag shrink-0 rounded-lg px-4 py-1.5 text-[12px] font-medium transition
          ${
            isConnected
              ? 'border border-border bg-white/60 text-foreground hover:bg-white'
              : 'bg-primary text-primary-foreground hover:bg-primary/90'
          }`}
      >
        {isConnected ? '管理' : '接入'}
      </button>
    </div>
  )
}

// ── Page ──────────────────────────────────────────────────────────────────────

export function ConnectionsSettingsPage({}: SettingsPageProps) {
  const handleConfigure = (id: string) => {
    console.log(`[Connections] Configure channel: ${id}`)
    // TODO: open channel config modal or navigate to channel detail
  }

  return (
    <div className="flex flex-col gap-4">
      {/* Priority channels */}
      <div>
        <div className="mb-3 flex items-center gap-2">
          <div className="h-px flex-1 bg-border/50" />
          <span className="text-[11px] font-medium uppercase tracking-widest text-muted-foreground">
            优先选项
          </span>
          <div className="h-px flex-1 bg-border/50" />
        </div>
        <div className="grid grid-cols-1 gap-2.5 sm:grid-cols-2 lg:grid-cols-3">
          {highPriority.map((ch) => (
            <ChannelCard key={ch.id} channel={ch} onConfigure={handleConfigure} />
          ))}
        </div>
      </div>

      {/* Other channels */}
      <div>
        <div className="mb-3 flex items-center gap-2">
          <div className="h-px flex-1 bg-border/50" />
          <span className="text-[11px] font-medium uppercase tracking-widest text-muted-foreground">
            其他渠道
          </span>
          <div className="h-px flex-1 bg-border/50" />
        </div>
        <div className="grid grid-cols-1 gap-2.5 sm:grid-cols-2 lg:grid-cols-3">
          {lowPriority.map((ch) => (
            <ChannelCard key={ch.id} channel={ch} onConfigure={handleConfigure} />
          ))}
        </div>
      </div>

      {/* Info card */}
      <SettingsSurface className="px-5 py-4">
        <div className="flex items-start gap-3">
          <div className="mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-primary/10 text-[10px] font-bold text-primary">
            i
          </div>
          <p className="text-[12px] leading-5 text-muted-foreground">
            所有连接能力都会优先保持在本地工作区内，后续可以再逐步接入更细的权限控制和审计记录。
          </p>
        </div>
      </SettingsSurface>
    </div>
  )
}
