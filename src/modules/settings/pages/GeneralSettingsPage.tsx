import { Avatar, AvatarFallback } from '@/components/ui/avatar'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { SettingsSurface } from '../components/SettingsSurface'
import { SettingsToggleRow } from '../components/SettingsToggleRow'
import type { SettingsPageProps } from '../types'

const controlClass =
  'h-8 rounded-xl border border-black/[0.09] bg-black/[0.025] px-3 text-[12px] font-medium shadow-none transition-all focus:border-jade/40 focus:ring-[3px] focus:ring-jade/15 focus-visible:ring-[3px] focus-visible:ring-jade/15'

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="mb-3 text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
      {children}
    </div>
  )
}

function Divider() {
  return <div className="my-0.5 border-t border-black/[0.05]" />
}

export function GeneralSettingsPage({ state, actions }: SettingsPageProps) {
  const initials = (state.username || 'RL')
    .split(/\s+/)
    .map((part) => part[0])
    .join('')
    .slice(0, 2)
    .toUpperCase()

  return (
    <div className="flex flex-col gap-3">
      {/* ── Profile ── */}
      <SettingsSurface className="px-5 py-4">
        <div className="flex items-center gap-4">
          <Avatar className="size-11 rounded-2xl border border-black/[0.07]">
            <AvatarFallback className="rounded-2xl bg-jade/10 text-[13px] font-bold text-jade">
              {initials}
            </AvatarFallback>
          </Avatar>
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-1.5">
              <span className="text-[14.5px] font-semibold tracking-tight">
                {state.username || '本地桌面账户'}
              </span>
              <Badge
                variant="secondary"
                className="rounded-lg bg-jade/10 px-1.5 py-0 text-[9.5px] font-semibold text-jade"
              >
                If2Ai
              </Badge>
              <Badge
                variant="outline"
                className="rounded-lg border-black/10 px-1.5 py-0 text-[9.5px] text-black/40"
              >
                Local
              </Badge>
            </div>
            <div className="mt-0.5 truncate text-[11.5px] text-muted-foreground">
              {state.email || '桌面 AI 智能体工作台的个人配置'}
            </div>
          </div>
          <Button
            variant="outline"
            className="window-no-drag h-7 rounded-xl border-black/[0.09] bg-black/[0.025] px-3 text-[11.5px] shadow-none hover:bg-black/[0.05]"
          >
            更换头像
          </Button>
        </div>
      </SettingsSurface>

      {/* ── Account ── */}
      <SettingsSurface className="px-5 py-4">
        <SectionLabel>账户信息</SectionLabel>
        <div className="grid gap-3 sm:grid-cols-2">
          <div className="grid gap-1.5">
            <Label htmlFor="username" className="text-[11px] font-medium text-black/40">
              用户名
            </Label>
            <Input
              id="username"
              value={state.username}
              onChange={(e) => actions.setUsername(e.target.value)}
              placeholder="输入用户名"
              className={controlClass}
            />
          </div>
          <div className="grid gap-1.5">
            <Label htmlFor="email" className="text-[11px] font-medium text-black/40">
              邮箱
            </Label>
            <Input
              id="email"
              value={state.email}
              onChange={(e) => actions.setEmail(e.target.value)}
              placeholder="输入邮箱"
              className={controlClass}
            />
          </div>
        </div>
      </SettingsSurface>

      {/* ── Preferences ── */}
      <SettingsSurface className="px-5 py-4">
        <SectionLabel>偏好设置</SectionLabel>
        <div className="grid gap-2 sm:grid-cols-2">
          {[
            {
              title: '界面语言',
              desc: '影响界面文本和提示语言',
              node: (
                <Select value={state.language} onValueChange={actions.setLanguage}>
                  <SelectTrigger className={controlClass}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="zh-CN">简体中文</SelectItem>
                    <SelectItem value="en">English</SelectItem>
                  </SelectContent>
                </Select>
              ),
            },
            {
              title: '默认启动',
              desc: '启动后进入的工作区策略',
              node: (
                <Select value={state.startupMode} onValueChange={actions.setStartupMode}>
                  <SelectTrigger className={controlClass}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="last">恢复上次会话</SelectItem>
                    <SelectItem value="workspace">打开工作区首页</SelectItem>
                  </SelectContent>
                </Select>
              ),
            },
            {
              title: '主题模式',
              desc: '控制系统外观基调',
              node: (
                <Select value={state.theme} onValueChange={actions.setTheme}>
                  <SelectTrigger className={controlClass}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="system">跟随系统</SelectItem>
                    <SelectItem value="light">浅色</SelectItem>
                    <SelectItem value="dark">深色</SelectItem>
                  </SelectContent>
                </Select>
              ),
            },
            {
              title: '界面密度',
              desc: '控制列表与表单间距',
              node: (
                <Select value={state.density} onValueChange={actions.setDensity}>
                  <SelectTrigger className={controlClass}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="compact">紧凑</SelectItem>
                    <SelectItem value="comfortable">舒适</SelectItem>
                    <SelectItem value="spacious">宽松</SelectItem>
                  </SelectContent>
                </Select>
              ),
            },
            {
              title: '聊天字体',
              desc: '正文所使用的字体风格',
              node: (
                <Select value={state.fontMode} onValueChange={actions.setFontMode}>
                  <SelectTrigger className={controlClass}>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="sans">非衬线</SelectItem>
                    <SelectItem value="serif">衬线</SelectItem>
                  </SelectContent>
                </Select>
              ),
            },
          ].map((item) => (
            <div
              key={item.title}
              className="flex items-center justify-between gap-3 rounded-xl border border-black/[0.06] bg-black/[0.016] px-3.5 py-2.5"
            >
              <div className="min-w-0">
                <div className="text-[12.5px] font-medium tracking-tight">{item.title}</div>
                <div className="mt-0.5 text-[11px] leading-4 text-muted-foreground">{item.desc}</div>
              </div>
              <div className="w-[130px] shrink-0">{item.node}</div>
            </div>
          ))}
        </div>
      </SettingsSurface>

      {/* ── Behavior toggles ── */}
      <SettingsSurface className="px-5 py-4">
        <SectionLabel>交互行为</SectionLabel>
        <div className="flex flex-col">
          <SettingsToggleRow
            title="自动滚动"
            description="AI 输出时自动滚动到最新内容"
            checked={state.autoScroll}
            onCheckedChange={actions.setAutoScroll}
            inline
          />
          <Divider />
          <SettingsToggleRow
            title="桌面通知"
            description="长时间任务完成后显示系统通知"
            checked={state.notifications}
            onCheckedChange={actions.setNotifications}
            inline
          />
        </div>
      </SettingsSurface>
    </div>
  )
}
