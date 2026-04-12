import { useEffect, useState } from 'react'
import {
  Settings,
  Palette,
  User,
  Key,
  Info,
  Sparkles,
  Globe,
  BellRing,
  ShieldCheck,
  Cpu,
  Server,
  WandSparkles,
  CheckCircle2,
  ExternalLink,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { Textarea } from '@/components/ui/textarea'
import { Badge } from '@/components/ui/badge'
import { Separator } from '@/components/ui/separator'
import { AgentOrb } from '../AgentOrb'
import type { ComponentType } from 'react'

interface SettingsAppProps {
  onClose: () => void
}

const SETTINGS_TABS = [
  { id: 'general', label: '通用', icon: Settings },
  { id: 'appearance', label: '外观', icon: Palette },
  { id: 'account', label: '账户', icon: User },
  { id: 'api', label: 'API', icon: Key },
  { id: 'about', label: '关于', icon: Info },
] as const

type TabId = (typeof SETTINGS_TABS)[number]['id']

export function SettingsApp({ onClose }: SettingsAppProps) {
  const [activeTab, setActiveTab] = useState<TabId>('general')
  const [theme, setTheme] = useState<'system' | 'light' | 'dark'>('system')
  const [fontMode, setFontMode] = useState<'serif' | 'sans'>('sans')
  const [language, setLanguage] = useState('zh-CN')
  const [startupMode, setStartupMode] = useState('last')
  const [density, setDensity] = useState('comfortable')
  const [autoScroll, setAutoScroll] = useState(true)
  const [notifications, setNotifications] = useState(true)
  const [username, setUsername] = useState('')
  const [email, setEmail] = useState('')
  const [apiKey, setApiKey] = useState('')
  const [baseUrl, setBaseUrl] = useState('')

  useEffect(() => {
    const savedFontMode = localStorage.getItem('fontMode') as 'serif' | 'sans' | null
    if (savedFontMode) {
      setFontMode(savedFontMode)
    }
  }, [])

  useEffect(() => {
    localStorage.setItem('fontMode', fontMode)
    document.body.classList.toggle('font-serif-mode', fontMode === 'serif')
  }, [fontMode])

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <header className="flex h-16 items-center justify-between border-b border-border/80 px-6">
        <div className="flex items-center gap-3">
          <div className="flex h-9 w-9 items-center justify-center rounded-2xl bg-primary/10 text-primary">
            <Sparkles className="h-4 w-4" />
          </div>
          <div>
            <div className="text-sm font-semibold">设置</div>
            <div className="text-xs text-muted-foreground">调整 If2Ai 的外观与行为</div>
          </div>
        </div>

        <Button variant="outline" onClick={onClose}>
          关闭
        </Button>
      </header>

      <div className="flex min-h-0 flex-1">
        <aside className="hidden w-72 shrink-0 border-r border-border/80 bg-muted/20 p-4 lg:block">
          <div className="rounded-3xl border border-border/70 bg-background/80 p-4 shadow-sm backdrop-blur">
            <AgentOrb status="idle" size="lg" showLabel label="If2Ai" />
            <div className="mt-4 space-y-2 text-center">
              <div className="text-sm font-semibold">工作区设置中心</div>
              <p className="text-sm leading-6 text-muted-foreground">
                统一管理视觉风格、账户信息与 API 连接，保持桌面应用的一致性和精致度。
              </p>
            </div>
          </div>

          <div className="mt-4 grid gap-3">
            <div className="rounded-2xl border border-border/70 bg-background p-4 shadow-sm">
              <div className="flex items-center gap-2 text-sm font-medium">
                <ShieldCheck className="h-4 w-4 text-primary" />
                安全优先
              </div>
              <p className="mt-2 text-sm text-muted-foreground">
                API Key 仅保留在本地环境中，不会在 UI 中明文展示。
              </p>
            </div>
            <div className="rounded-2xl border border-border/70 bg-background p-4 shadow-sm">
              <div className="flex items-center gap-2 text-sm font-medium">
                <Cpu className="h-4 w-4 text-primary" />
                桌面优先
              </div>
              <p className="mt-2 text-sm text-muted-foreground">
                窗口层次、拖拽区域和交互反馈都针对 Tauri 桌面场景设计。
              </p>
            </div>
          </div>
        </aside>

        <main className="min-w-0 flex-1 overflow-y-auto p-4 lg:p-6">
          <Tabs value={activeTab} onValueChange={(value) => setActiveTab(value as TabId)}>
            <TabsList className="grid h-auto w-full grid-cols-2 gap-2 rounded-2xl bg-muted p-2 lg:grid-cols-5">
              {SETTINGS_TABS.map((tab) => {
                const Icon = tab.icon
                return (
                  <TabsTrigger
                    key={tab.id}
                    value={tab.id}
                    className="justify-center gap-2 rounded-xl py-2.5"
                  >
                    <Icon className="h-4 w-4" />
                    <span>{tab.label}</span>
                  </TabsTrigger>
                )
              })}
            </TabsList>

            <TabsContent value="general">
              <Card className="border-border/70 shadow-sm">
                <CardHeader>
                  <CardTitle>通用设置</CardTitle>
                  <CardDescription>控制语言、通知和自动滚动等基础行为。</CardDescription>
                </CardHeader>
                <CardContent className="grid gap-6">
                  <div className="grid gap-2">
                    <Label>界面语言</Label>
                    <Select value={language} onValueChange={setLanguage}>
                      <SelectTrigger>
                        <SelectValue placeholder="选择语言" />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="zh-CN">简体中文</SelectItem>
                        <SelectItem value="en">English</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>

                  <div className="grid gap-2">
                    <Label>默认启动项目</Label>
                    <Select value={startupMode} onValueChange={setStartupMode}>
                      <SelectTrigger>
                        <SelectValue placeholder="选择启动策略" />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="last">恢复上次会话</SelectItem>
                        <SelectItem value="workspace">打开工作区首页</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>

                  <Separator />

                  <SettingRow
                    title="自动滚动"
                    description="当 AI 输出新消息时自动滚动到最新内容。"
                    checked={autoScroll}
                    onCheckedChange={setAutoScroll}
                  />
                  <SettingRow
                    title="桌面通知"
                    description="在长时间思考或任务完成时显示系统通知。"
                    checked={notifications}
                    onCheckedChange={setNotifications}
                  />
                </CardContent>
              </Card>
            </TabsContent>

            <TabsContent value="appearance">
              <div className="grid gap-4 xl:grid-cols-[1.4fr_0.9fr]">
                <Card className="border-border/70 shadow-sm">
                  <CardHeader>
                    <CardTitle>外观设置</CardTitle>
                    <CardDescription>控制主题、字体和密度，让桌面观感更稳定、更高级。</CardDescription>
                  </CardHeader>
                  <CardContent className="grid gap-6">
                    <div className="grid gap-2">
                      <Label>主题</Label>
                      <Select value={theme} onValueChange={(value) => setTheme(value as typeof theme)}>
                        <SelectTrigger>
                          <SelectValue placeholder="选择主题" />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="system">跟随系统</SelectItem>
                          <SelectItem value="light">浅色</SelectItem>
                          <SelectItem value="dark">深色</SelectItem>
                        </SelectContent>
                      </Select>
                    </div>

                    <div className="grid gap-2">
                      <Label>聊天字体</Label>
                      <Select value={fontMode} onValueChange={(value) => setFontMode(value as typeof fontMode)}>
                        <SelectTrigger>
                          <SelectValue placeholder="选择字体" />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="sans">非衬线</SelectItem>
                          <SelectItem value="serif">衬线</SelectItem>
                        </SelectContent>
                      </Select>
                    </div>

                    <div className="grid gap-2">
                      <Label>界面密度</Label>
                      <Select value={density} onValueChange={setDensity}>
                        <SelectTrigger>
                          <SelectValue placeholder="选择密度" />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="compact">紧凑</SelectItem>
                          <SelectItem value="comfortable">舒适</SelectItem>
                          <SelectItem value="spacious">宽松</SelectItem>
                        </SelectContent>
                      </Select>
                    </div>
                  </CardContent>
                </Card>

                <Card className="border-border/70 shadow-sm">
                  <CardHeader>
                    <CardTitle>视觉预览</CardTitle>
                    <CardDescription>当前配色与字体模式的即时感知。</CardDescription>
                  </CardHeader>
                  <CardContent className="space-y-4">
                    <div className="rounded-3xl border border-border/60 bg-muted/30 p-4">
                      <div className="flex items-center gap-3">
                        <div className="rounded-2xl bg-primary/10 p-3 text-primary">
                          <WandSparkles className="h-5 w-5" />
                        </div>
                        <div>
                          <div className="text-sm font-semibold">精致桌面感</div>
                          <div className="text-sm text-muted-foreground">软边框、低对比阴影、冷暖平衡。</div>
                        </div>
                      </div>
                      <div className="mt-4 flex flex-wrap gap-2">
                        <Badge variant="secondary">Neutral</Badge>
                        <Badge variant="outline">Glass</Badge>
                        <Badge>Elegant</Badge>
                      </div>
                    </div>
                    <div className="grid gap-2 rounded-3xl border border-border/60 bg-background p-4">
                      <div className="text-xs uppercase tracking-[0.2em] text-muted-foreground">Typography</div>
                      <div className="text-lg font-semibold">If2Ai</div>
                      <p className="text-sm leading-6 text-muted-foreground">
                        用更克制的字号、间距和光影，让 AI 智能体桌面应用看起来更像一款正式工作台，而不是原型。
                      </p>
                    </div>
                  </CardContent>
                </Card>
              </div>
            </TabsContent>

            <TabsContent value="account">
              <Card className="border-border/70 shadow-sm">
                <CardHeader>
                  <CardTitle>账户设置</CardTitle>
                  <CardDescription>用于显示本地用户信息、备注和个性化字段。</CardDescription>
                </CardHeader>
                <CardContent className="grid gap-5">
                  <div className="grid gap-2">
                    <Label htmlFor="username">用户名</Label>
                    <Input
                      id="username"
                      value={username}
                      onChange={(e) => setUsername(e.target.value)}
                      placeholder="输入用户名"
                    />
                  </div>
                  <div className="grid gap-2">
                    <Label htmlFor="email">邮箱</Label>
                    <Input
                      id="email"
                      value={email}
                      onChange={(e) => setEmail(e.target.value)}
                      placeholder="输入邮箱"
                    />
                  </div>
                  <div className="grid gap-2">
                    <Label htmlFor="profile-note">个人备注</Label>
                    <Textarea
                      id="profile-note"
                      value=""
                      readOnly
                      placeholder="这里可以放入更完整的账户或偏好说明"
                      className="min-h-28"
                    />
                  </div>
                </CardContent>
              </Card>
            </TabsContent>

            <TabsContent value="api">
              <div className="grid gap-4 xl:grid-cols-[1.2fr_0.8fr]">
                <Card className="border-border/70 shadow-sm">
                  <CardHeader>
                    <CardTitle>API 配置</CardTitle>
                    <CardDescription>统一管理模型提供方、Base URL 和认证信息。</CardDescription>
                  </CardHeader>
                  <CardContent className="grid gap-5">
                    <div className="rounded-2xl border border-border/60 bg-muted/20 p-4">
                      <div className="flex items-center justify-between gap-3">
                        <div>
                          <div className="text-sm font-medium">Anthropic / OpenAI 兼容接口</div>
                          <div className="mt-1 text-sm text-muted-foreground">
                            推荐在本地环境中维护 API Key 与模型名称。
                          </div>
                        </div>
                        <Badge variant="secondary" className="gap-1.5">
                          <CheckCircle2 className="h-3.5 w-3.5" />
                          Local
                        </Badge>
                      </div>
                    </div>

                    <div className="grid gap-2">
                      <Label htmlFor="api-key">API Key</Label>
                      <Input
                        id="api-key"
                        type="password"
                        value={apiKey}
                        onChange={(e) => setApiKey(e.target.value)}
                        placeholder="sk-..."
                      />
                    </div>

                    <div className="grid gap-2">
                      <Label htmlFor="base-url">Base URL</Label>
                      <Input
                        id="base-url"
                        value={baseUrl}
                        onChange={(e) => setBaseUrl(e.target.value)}
                        placeholder="https://api.example.com"
                      />
                    </div>

                    <div className="grid gap-2">
                      <Label htmlFor="model-name">模型名称</Label>
                      <Input
                        id="model-name"
                        placeholder="claude-3-5-sonnet-latest"
                      />
                    </div>

                    <div className="flex flex-wrap gap-3">
                      <Button>保存配置</Button>
                      <Button variant="outline">测试连接</Button>
                    </div>
                  </CardContent>
                </Card>

                <Card className="border-border/70 shadow-sm">
                  <CardHeader>
                    <CardTitle>连接状态</CardTitle>
                    <CardDescription>快速查看本地 API 是否已准备好。</CardDescription>
                  </CardHeader>
                  <CardContent className="space-y-3">
                    <StatusRow icon={Server} label="服务地址" value="已配置" />
                    <StatusRow icon={Cpu} label="模型可用性" value="待检测" />
                    <StatusRow icon={ShieldCheck} label="本地存储" value="仅本机" />

                    <Separator className="my-4" />
                    <Button variant="secondary" className="w-full gap-2">
                      <ExternalLink className="h-4 w-4" />
                      打开 API 文档
                    </Button>
                  </CardContent>
                </Card>
              </div>
            </TabsContent>

            <TabsContent value="about">
              <Card className="border-border/70 shadow-sm">
                <CardContent className="grid gap-6 p-6 lg:grid-cols-[0.9fr_1.1fr]">
                  <div className="flex flex-col items-center justify-center rounded-3xl border border-border/70 bg-muted/20 p-8 text-center">
                    <AgentOrb status="idle" size="hero" showLabel label="If2Ai" />
                    <div className="mt-8 space-y-2">
                      <div className="text-2xl font-semibold">If2Ai</div>
                      <div className="text-sm text-muted-foreground">桌面 AI 智能体工作台</div>
                      <Badge variant="secondary">Version 0.1.0</Badge>
                    </div>
                  </div>

                  <div className="space-y-4">
                    <div>
                      <h3 className="text-lg font-semibold">关于这个应用</h3>
                      <p className="mt-2 text-sm leading-7 text-muted-foreground">
                        If2Ai 是一个基于 Tauri + Rust + React 的桌面智能体工作台，强调项目分组、会话流式输出、可观测性和本地化执行体验。
                      </p>
                    </div>

                    <div className="grid gap-3">
                      <FeatureLine icon={Sparkles} title="设计原则" text="Agent 优先、层次克制、信息密度清晰。" />
                      <FeatureLine icon={Globe} title="跨平台" text="统一的桌面壳，适合 macOS / Windows / Linux。" />
                      <FeatureLine icon={BellRing} title="可扩展" text="未来可以继续接入插件、工具面板与评估系统。" />
                    </div>
                  </div>
                </CardContent>
              </Card>
            </TabsContent>
          </Tabs>
        </main>
      </div>
    </div>
  )
}

function SettingRow({
  title,
  description,
  checked,
  onCheckedChange,
}: {
  title: string
  description: string
  checked: boolean
  onCheckedChange: (checked: boolean) => void
}) {
  return (
    <div className="flex items-center justify-between gap-6 rounded-2xl border border-border/60 bg-muted/20 p-4">
      <div className="space-y-1">
        <div className="text-sm font-medium">{title}</div>
        <div className="text-sm leading-6 text-muted-foreground">{description}</div>
      </div>
      <Switch checked={checked} onCheckedChange={onCheckedChange} />
    </div>
  )
}

function StatusRow({
  icon: Icon,
  label,
  value,
}: {
  icon: ComponentType<{ className?: string }>
  label: string
  value: string
}) {
  return (
    <div className="flex items-center justify-between rounded-2xl border border-border/60 bg-muted/10 px-4 py-3">
      <div className="flex items-center gap-2 text-sm font-medium">
        <Icon className="h-4 w-4 text-primary" />
        {label}
      </div>
      <div className="text-sm text-muted-foreground">{value}</div>
    </div>
  )
}

function FeatureLine({
  icon: Icon,
  title,
  text,
}: {
  icon: ComponentType<{ className?: string }>
  title: string
  text: string
}) {
  return (
    <div className="flex gap-3 rounded-2xl border border-border/60 bg-muted/10 p-4">
      <div className="mt-0.5 flex h-9 w-9 items-center justify-center rounded-xl bg-primary/10 text-primary">
        <Icon className="h-4 w-4" />
      </div>
      <div className="space-y-1">
        <div className="text-sm font-medium">{title}</div>
        <div className="text-sm leading-6 text-muted-foreground">{text}</div>
      </div>
    </div>
  )
}
