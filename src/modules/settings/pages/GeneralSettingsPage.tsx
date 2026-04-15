import { Avatar, AvatarFallback } from '@/components/ui/avatar'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { SettingsSurface } from '../components/SettingsSurface'
import { SettingsToggleRow } from '../components/SettingsToggleRow'
import type { SettingsPageProps } from '../types'

const compactControlClass =
  'h-8 rounded-xl border border-border/60 bg-white/60 px-3 text-[12px] shadow-none focus-visible:ring-[3px] focus-visible:ring-primary/40 focus:border-primary/40'

const compactButtonClass =
  'window-no-drag h-8 rounded-xl border border-border/60 bg-white/60 px-4 text-[12px] shadow-none hover:bg-white/80'

export function GeneralSettingsPage({ state, actions }: SettingsPageProps) {
  const initials = (state.username || 'RL')
    .split(/\s+/)
    .map((part) => part[0])
    .join('')
    .slice(0, 2)
    .toUpperCase()

  return (
    <div className="flex flex-col gap-3">
      {/* Profile */}
      <SettingsSurface className="px-5 py-4">
        <div className="flex items-center gap-4">
          <Avatar className="size-12 rounded-2xl border border-border/50 shadow-sm">
            <AvatarFallback className="rounded-2xl bg-primary/10 text-sm font-semibold text-primary">
              {initials}
            </AvatarFallback>
          </Avatar>
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <div className="text-[15px] font-semibold tracking-tight">{state.username || '本地桌面账户'}</div>
              <Badge variant="secondary" className="text-[10px]">If2Ai</Badge>
              <Badge variant="outline" className="text-[10px]">Local</Badge>
            </div>
            <div className="text-[12px] leading-5 text-muted-foreground truncate">
              {state.email || '桌面 AI 智能体工作台的个人配置'}
            </div>
          </div>
          <Button variant="outline" className={compactButtonClass}>
            更换头像
          </Button>
        </div>
      </SettingsSurface>

      {/* Account + Preferences merged */}
      <SettingsSurface className="px-5 py-4">
        <div className="text-[13px] font-semibold tracking-tight">账户信息</div>
        <div className="mt-3 grid gap-3 lg:grid-cols-2">
          <div className="grid gap-1.5">
            <Label htmlFor="username" className="text-[12px] text-muted-foreground">用户名</Label>
            <Input
              id="username"
              value={state.username}
              onChange={(event) => actions.setUsername(event.target.value)}
              placeholder="输入用户名"
              className={compactControlClass}
            />
          </div>
          <div className="grid gap-1.5">
            <Label htmlFor="email" className="text-[12px] text-muted-foreground">邮箱</Label>
            <Input
              id="email"
              value={state.email}
              onChange={(event) => actions.setEmail(event.target.value)}
              placeholder="输入邮箱"
              className={compactControlClass}
            />
          </div>
        </div>
      </SettingsSurface>

      {/* Preferences — two-column compact list */}
      <SettingsSurface className="px-5 py-4">
        <div className="text-[13px] font-semibold tracking-tight">偏好设置</div>
        <div className="mt-3 grid gap-2.5 sm:grid-cols-2">
          {[
            {
              title: '界面语言',
              desc: '影响界面文本和提示语言',
              select: (
                <Select value={state.language} onValueChange={actions.setLanguage}>
                  <SelectTrigger className={compactControlClass}>
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
              select: (
                <Select value={state.startupMode} onValueChange={actions.setStartupMode}>
                  <SelectTrigger className={compactControlClass}>
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
              select: (
                <Select value={state.theme} onValueChange={actions.setTheme}>
                  <SelectTrigger className={compactControlClass}>
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
              select: (
                <Select value={state.density} onValueChange={actions.setDensity}>
                  <SelectTrigger className={compactControlClass}>
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
              select: (
                <Select value={state.fontMode} onValueChange={actions.setFontMode}>
                  <SelectTrigger className={compactControlClass}>
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
            <div key={item.title} className="flex items-center justify-between rounded-2xl border border-border/50 bg-white/60 px-4 py-2.5">
              <div className="space-y-0.5">
                <div className="text-[12px] font-medium">{item.title}</div>
                <div className="text-[11px] leading-4 text-muted-foreground">{item.desc}</div>
              </div>
              {item.select}
            </div>
          ))}
        </div>
      </SettingsSurface>

      {/* Toggles */}
      <SettingsSurface className="px-5 py-4">
        <div className="text-[13px] font-semibold tracking-tight">交互行为</div>
        <div className="mt-3 grid gap-2">
          <SettingsToggleRow
            title="自动滚动"
            description="AI 输出时自动滚动到最新内容"
            checked={state.autoScroll}
            onCheckedChange={actions.setAutoScroll}
          />
          <SettingsToggleRow
            title="桌面通知"
            description="长时间任务完成后显示系统通知"
            checked={state.notifications}
            onCheckedChange={actions.setNotifications}
          />
        </div>
      </SettingsSurface>
    </div>
  )
}
