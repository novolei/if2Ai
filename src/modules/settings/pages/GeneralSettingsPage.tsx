import { Avatar, AvatarFallback } from '@/components/ui/avatar'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { SettingsSurface } from '../components/SettingsSurface'
import { SettingsRow } from '../components/SettingsRow'
import { SettingsToggleRow } from '../components/SettingsToggleRow'
import type { SettingsPageProps } from '../types'

const compactControlClass =
  'h-9 rounded-[18px] border border-black/10 bg-white/84 px-3.5 text-[13px] shadow-none focus-visible:ring-1 focus-visible:ring-ring'

const compactButtonClass =
  'window-no-drag h-9 rounded-[18px] border border-black/10 bg-white/84 px-4 text-[13px] shadow-none hover:bg-white'

export function GeneralSettingsPage({ state, actions }: SettingsPageProps) {
  const initials = (state.username || 'RL')
    .split(/\s+/)
    .map((part) => part[0])
    .join('')
    .slice(0, 2)
    .toUpperCase()

  return (
    <div className="space-y-3.5">
      <SettingsSurface>
        <div className="flex flex-col gap-5 p-5 lg:flex-row lg:items-center lg:justify-between">
          <div className="flex items-center gap-3.5">
            <Avatar className="size-14 rounded-[20px] border border-black/5 shadow-sm">
              <AvatarFallback className="rounded-[20px] bg-black/5 text-base font-semibold text-black/72">
                {initials}
              </AvatarFallback>
            </Avatar>

            <div className="space-y-1">
              <div className="text-[17px] font-semibold tracking-tight">{state.username || '本地桌面账户'}</div>
              <div className="text-[12px] leading-5 text-muted-foreground">{state.email || '桌面 AI 智能体工作台的个人配置'}</div>
              <div className="flex flex-wrap gap-2 pt-0.5">
                <Badge variant="secondary">If2Ai</Badge>
                <Badge variant="outline">Local</Badge>
              </div>
            </div>
          </div>

          <Button variant="outline" className={compactButtonClass}>
            更换头像
          </Button>
        </div>
      </SettingsSurface>

      <SettingsSurface className="p-5">
        <div className="grid gap-3.5 xl:grid-cols-2">
          <SettingsRow
            title="界面语言"
            description="影响界面文本、提示和默认交互语言。"
          >
            <div className="min-w-[220px]">
              <Select value={state.language} onValueChange={actions.setLanguage}>
                <SelectTrigger className={compactControlClass}>
                  <SelectValue placeholder="选择语言" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="zh-CN">简体中文</SelectItem>
                  <SelectItem value="en">English</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </SettingsRow>

          <SettingsRow
            title="默认启动项目"
            description="启动后默认进入的工作区或恢复策略。"
          >
            <div className="min-w-[220px]">
              <Select value={state.startupMode} onValueChange={actions.setStartupMode}>
                <SelectTrigger className={compactControlClass}>
                  <SelectValue placeholder="选择启动策略" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="last">恢复上次会话</SelectItem>
                  <SelectItem value="workspace">打开工作区首页</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </SettingsRow>

          <SettingsRow title="主题模式" description="控制系统主题和页面基调。">
            <div className="min-w-[220px]">
              <Select value={state.theme} onValueChange={actions.setTheme}>
                <SelectTrigger className={compactControlClass}>
                  <SelectValue placeholder="选择主题" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="system">跟随系统</SelectItem>
                  <SelectItem value="light">浅色</SelectItem>
                  <SelectItem value="dark">深色</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </SettingsRow>

          <SettingsRow title="界面密度" description="收紧或放松列表、卡片和表单间距。">
            <div className="min-w-[220px]">
              <Select value={state.density} onValueChange={actions.setDensity}>
                <SelectTrigger className={compactControlClass}>
                  <SelectValue placeholder="选择密度" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="compact">紧凑</SelectItem>
                  <SelectItem value="comfortable">舒适</SelectItem>
                  <SelectItem value="spacious">宽松</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </SettingsRow>

          <SettingsRow title="聊天字体" description="控制聊天正文所使用的字体风格。">
            <div className="min-w-[220px]">
              <Select value={state.fontMode} onValueChange={actions.setFontMode}>
                <SelectTrigger className={compactControlClass}>
                  <SelectValue placeholder="选择字体" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="sans">非衬线</SelectItem>
                  <SelectItem value="serif">衬线</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </SettingsRow>
        </div>
      </SettingsSurface>

      <SettingsSurface className="p-5">
        <div className="grid gap-3.5">
          <SettingsToggleRow
            title="自动滚动"
            description="当 AI 输出新消息时自动滚动到最新内容。"
            checked={state.autoScroll}
            onCheckedChange={actions.setAutoScroll}
          />
          <SettingsToggleRow
            title="桌面通知"
            description="在长时间思考或任务完成时显示系统通知。"
            checked={state.notifications}
            onCheckedChange={actions.setNotifications}
          />
        </div>
      </SettingsSurface>

      <SettingsSurface className="p-5">
        <div className="grid gap-4 lg:grid-cols-[1fr_auto] lg:items-center">
          <div>
            <div className="text-[14px] font-semibold tracking-tight">本地信息</div>
            <p className="mt-2 text-[12px] leading-5 text-muted-foreground">
              当前设置会优先保存在本地，不会主动上传到云端。
            </p>
          </div>
          <Button variant="outline" className={compactButtonClass}>
            打开本地目录
          </Button>
        </div>
      </SettingsSurface>

      <SettingsSurface className="p-5">
        <div className="grid gap-3.5">
          <div className="text-[14px] font-semibold tracking-tight">账户信息</div>
          <div className="grid gap-4 lg:grid-cols-2">
            <div className="grid gap-2">
              <Label htmlFor="username">用户名</Label>
              <Input
                id="username"
                value={state.username}
                onChange={(event) => actions.setUsername(event.target.value)}
                placeholder="输入用户名"
                className={compactControlClass}
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="email">邮箱</Label>
              <Input
                id="email"
                value={state.email}
                onChange={(event) => actions.setEmail(event.target.value)}
                placeholder="输入邮箱"
                className={compactControlClass}
              />
            </div>
          </div>
        </div>
      </SettingsSurface>
    </div>
  )
}
