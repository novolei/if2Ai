import { Avatar, AvatarFallback } from '@/components/ui/avatar'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Check } from 'lucide-react'
import { cn } from '@/lib/utils'
import { SettingsSurface } from '../components/SettingsSurface'
import { SettingsToggleRow } from '../components/SettingsToggleRow'
import type { SettingsPageProps, ThemeMode } from '../types'

const controlClass =
  'h-8 rounded-xl border border-border/70 bg-muted/30 px-3 text-[12px] font-medium shadow-none transition-all focus:border-jade/40 focus:ring-[3px] focus:ring-jade/15 focus-visible:ring-[3px] focus-visible:ring-jade/15'

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="mb-3 text-[10.5px] font-semibold uppercase tracking-widest text-muted-foreground/70">
      {children}
    </div>
  )
}

function Divider() {
  return <div className="my-0.5 border-t border-border/55" />
}

const themeCards: Array<{
  id: Exclude<ThemeMode, 'system'>
  title: string
  subtitle: string
  className: string
  preview: React.ReactNode
}> = [
  {
    id: 'current',
    title: '当前',
    subtitle: '已有主题',
    className: 'bg-[#f4faf9] text-[#243332]',
    preview: (
      <>
        <span className="bg-[#e8eeee]" />
        <span className="bg-[#dbecea]" />
        <span className="bg-[#21b57b]" />
      </>
    ),
  },
  {
    id: 'warm-paper',
    title: '新暖纸',
    subtitle: '纸感白天',
    className: 'bg-[#f4efe4] text-[#2c2924]',
    preview: (
      <>
        <span className="bg-[#e7decc]" />
        <span className="bg-[#fbf7ed]" />
        <span className="bg-[#4e7f9b]" />
      </>
    ),
  },
  {
    id: 'qingye',
    title: '青夜',
    subtitle: '柔和夜间',
    className: 'bg-[#30464e] text-[#c8d6dc]',
    preview: (
      <>
        <span className="bg-[#263941]" />
        <span className="bg-[#3c535b]" />
        <span className="bg-[#c67d95]" />
      </>
    ),
  },
  {
    id: 'black',
    title: '黑色',
    subtitle: '深色专注',
    className: 'bg-[#18191d] text-[#dedee3]',
    preview: (
      <>
        <span className="bg-[#24252b]" />
        <span className="bg-[#2f2e35]" />
        <span className="bg-[#c19a3b]" />
      </>
    ),
  },
  {
    id: 'theFinals',
    title: 'THE FINALS',
    subtitle: '竞技赛场',
    className:
      'bg-[#d91f3c] bg-[linear-gradient(90deg,rgba(12,12,14,0.58),rgba(217,31,60,0.70)),url("/src/assets/themes/the-finals/s10-keyart-bkg.png")] bg-cover bg-center text-white shadow-[0_14px_34px_rgba(217,31,60,0.22)] before:absolute before:inset-0 before:bg-[linear-gradient(135deg,rgba(255,255,255,0.18)_0_16%,transparent_16%_36%,rgba(0,0,0,0.30)_36%_62%,transparent_62%)] before:opacity-80 after:absolute after:right-3 after:top-3 after:h-7 after:w-24 after:bg-[url("/src/assets/themes/the-finals/s10-logo.png")] after:bg-contain after:bg-right after:bg-no-repeat after:opacity-90',
    preview: (
      <>
        <span className="bg-[#fff4df]" />
        <span className="bg-[#ffd23f]" />
        <span className="bg-[#171719]" />
      </>
    ),
  },
]

function ThemePreviewCard({
  selected,
  onSelect,
  title,
  subtitle,
  className,
  preview,
}: {
  selected: boolean
  onSelect: () => void
  title: string
  subtitle: string
  className: string
  preview: React.ReactNode
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      className={cn(
        'group relative flex h-[104px] flex-col justify-between overflow-hidden rounded-[8px] border px-4 py-3 text-left shadow-[0_10px_24px_rgba(0,0,0,0.045)] transition-all hover:-translate-y-0.5 hover:shadow-[0_16px_30px_rgba(0,0,0,0.075)]',
        selected ? 'border-foreground ring-2 ring-foreground/85' : 'border-border/40',
        className,
      )}
      aria-pressed={selected}
    >
      <div className="flex justify-end gap-1.5">
        <div className="flex gap-1.5 [&>span]:h-2 [&>span]:w-7 [&>span]:rounded-full">
          {preview}
        </div>
      </div>
      <div>
        <div className="text-[18px] font-semibold leading-tight tracking-tight">{title}</div>
        <div className="mt-1 text-[12.5px] opacity-70">{subtitle}</div>
      </div>
      {selected ? (
        <span className="absolute right-2.5 top-2.5 flex size-5 items-center justify-center rounded-full bg-card/95 text-foreground shadow-sm">
          <Check className="size-3.5" strokeWidth={2.4} />
        </span>
      ) : null}
    </button>
  )
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
          <Avatar className="size-11 rounded-2xl border border-border/70">
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
                className="rounded-lg border-border/70 px-1.5 py-0 text-[9.5px] text-muted-foreground"
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
            className="window-no-drag h-7 rounded-xl border-border/70 bg-muted/30 px-3 text-[11.5px] shadow-none hover:bg-muted/50"
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
            <Label htmlFor="username" className="text-[11px] font-medium text-muted-foreground">
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
            <Label htmlFor="email" className="text-[11px] font-medium text-muted-foreground">
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

      {/* ── Theme picker ── */}
      <SettingsSurface className="px-5 py-4">
        <div className="mb-4 flex items-end justify-between gap-3">
          <div>
            <SectionLabel>主题</SectionLabel>
            <div className="text-[12px] text-muted-foreground">
              选择后立即应用到当前窗口，并会保存到下次启动。
            </div>
          </div>
          <button
            type="button"
            onClick={() => actions.setTheme('system')}
            className={cn(
              'h-8 rounded-xl border px-3 text-[11.5px] font-medium transition-colors',
              state.theme === 'system'
                ? 'border-jade/40 bg-jade/10 text-jade'
                : 'border-border/70 bg-muted/30 text-muted-foreground hover:bg-muted/50',
            )}
          >
            跟随系统
          </button>
        </div>
        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-5">
          {themeCards.map((theme) => (
            <ThemePreviewCard
              key={theme.id}
              selected={state.theme === theme.id}
              onSelect={() => actions.setTheme(theme.id)}
              title={theme.title}
              subtitle={theme.subtitle}
              className={theme.className}
              preview={theme.preview}
            />
          ))}
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
              className="flex items-center justify-between gap-3 rounded-xl border border-border/60 bg-muted/25 px-3.5 py-2.5"
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
