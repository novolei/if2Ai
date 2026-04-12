import type { ComponentType } from 'react'
import { BellRing, Globe, Sparkles } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { AgentOrb } from '@/components/AgentOrb'
import { SettingsSurface } from '../components/SettingsSurface'
import type { SettingsPageProps } from '../types'

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
    <div className="flex gap-3 rounded-[24px] border border-white/60 bg-white/70 p-4">
      <div className="mt-0.5 flex h-9 w-9 items-center justify-center rounded-2xl bg-primary/10 text-primary">
        <Icon className="h-4 w-4" />
      </div>
      <div className="space-y-1">
        <div className="text-sm font-medium tracking-tight">{title}</div>
        <div className="text-sm leading-6 text-muted-foreground">{text}</div>
      </div>
    </div>
  )
}

export function AboutSettingsPage({}: SettingsPageProps) {
  return (
    <div className="flex flex-col gap-3.5">
      <SettingsSurface className="p-5">
        <div className="grid gap-5 lg:grid-cols-[0.95fr_1.05fr] lg:items-center">
          <div className="flex flex-col items-center justify-center rounded-[24px] border border-black/5 bg-white/72 p-6 text-center">
            <AgentOrb status="idle" size="hero" showLabel label="If2Ai" />
            <div className="mt-6 space-y-2">
              <div className="text-[24px] font-semibold tracking-tight">If2Ai</div>
              <div className="text-[12px] text-muted-foreground">桌面 AI 智能体工作台</div>
              <Badge variant="secondary">Version 0.1.0</Badge>
            </div>
          </div>

          <div className="space-y-4">
            <div>
              <div className="text-[14px] font-semibold tracking-tight">关于这个应用</div>
              <p className="mt-2 text-[12px] leading-6 text-muted-foreground">
                If2Ai 是一个基于 Tauri + Rust + React 的桌面智能体工作台，强调项目分组、
                会话流式输出、可观测性和本地化执行体验。
              </p>
            </div>

            <div className="grid gap-2.5">
              <FeatureLine icon={Sparkles} title="设计原则" text="Agent 优先、层次克制、信息密度清晰。" />
              <FeatureLine icon={Globe} title="跨平台" text="统一的桌面壳，适合 macOS / Windows / Linux。" />
              <FeatureLine icon={BellRing} title="可扩展" text="未来可以继续接入插件、工具面板与评估系统。" />
            </div>

            <div className="flex flex-wrap gap-2.5 pt-1">
              <Button variant="outline" className="window-no-drag h-9 rounded-[18px] border border-black/10 bg-white/84 px-4 text-[13px] shadow-none hover:bg-white">
                查看文档
              </Button>
              <Button className="window-no-drag h-9 rounded-[18px] px-4 text-[13px] shadow-none">检查更新</Button>
            </div>
          </div>
        </div>
      </SettingsSurface>
    </div>
  )
}
