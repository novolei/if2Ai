import { Shield, Globe, Sparkles, ExternalLink, Rocket } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { AgentOrb } from '@/components/AgentOrb'
import { SettingsSurface } from '../components/SettingsSurface'
import type { SettingsPageProps } from '../types'

interface FeatureCardProps {
  icon: typeof Shield
  title: string
  text: string
}

function FeatureCard({ icon: Icon, title, text }: FeatureCardProps) {
  return (
    <div className="flex items-start gap-3 rounded-2xl border border-border/50 bg-white/60 px-4 py-3">
      <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-2xl bg-primary/10 text-primary">
        <Icon className="h-4 w-4" />
      </div>
      <div>
        <div className="text-[12px] font-semibold tracking-tight">{title}</div>
        <p className="mt-0.5 text-[11px] leading-4 text-muted-foreground">{text}</p>
      </div>
    </div>
  )
}

export function AboutSettingsPage({}: SettingsPageProps) {
  return (
    <div className="flex flex-col gap-3">
      {/* Hero */}
      <SettingsSurface className="px-6 py-6">
        <div className="flex flex-col items-center gap-5 lg:flex-row lg:items-center lg:justify-between">
          <div className="flex flex-col items-center gap-3">
            <AgentOrb status="idle" size="hero" showLabel label="If2Ai" />
            <div className="space-y-1 text-center">
              <div className="text-[20px] font-semibold tracking-tight">If2Ai</div>
              <div className="text-[12px] text-muted-foreground">桌面 AI 智能体工作台</div>
            </div>
          </div>

          <div className="flex flex-col items-center gap-3 lg:items-start">
            <p className="text-[12px] leading-5 text-muted-foreground max-w-sm text-center lg:text-left">
              基于 Tauri + Rust + React 的桌面智能体工作台，强调项目分组、
              会话流式输出、可观测性和本地化执行体验。
            </p>
            <div className="flex flex-wrap gap-2">
              <Button variant="outline" className={compactButtonClass}>
                <ExternalLink className="mr-1.5 h-3.5 w-3.5" />
                查看文档
              </Button>
              <Button className={compactButtonPrimary}>
                <Rocket className="mr-1.5 h-3.5 w-3.5" />
                检查更新
              </Button>
            </div>
          </div>
        </div>
      </SettingsSurface>

      {/* Features */}
      <SettingsSurface className="px-5 py-4">
        <div className="text-[13px] font-semibold tracking-tight">设计原则</div>
        <div className="mt-3 grid gap-2 sm:grid-cols-3">
          <FeatureCard icon={Sparkles} title="Agent 优先" text="层次克制、信息密度清晰" />
          <FeatureCard icon={Globe} title="跨平台" text="macOS / Windows / Linux 统一桌面壳" />
          <FeatureCard icon={Shield} title="可扩展" text="未来接入插件、工具面板与评估系统" />
        </div>
      </SettingsSurface>

      {/* Version info */}
      <SettingsSurface className="px-5 py-3">
        <div className="flex items-center justify-between">
          <div className="text-[12px] text-muted-foreground">
            当前版本
          </div>
          <div className="font-mono text-[12px] font-medium tabular-nums">v0.1.0</div>
        </div>
      </SettingsSurface>
    </div>
  )
}

const compactButtonClass =
  'window-no-drag h-8 rounded-xl border border-border/60 bg-white/60 px-3.5 text-[12px] shadow-none hover:bg-white/80'

const compactButtonPrimary =
  'window-no-drag h-8 rounded-xl bg-primary px-3.5 text-[12px] text-primary-foreground shadow-none hover:bg-primary/90'
