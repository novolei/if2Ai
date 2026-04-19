import { Shield, Globe, Sparkles, ExternalLink, Rocket, RotateCcw } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { AgentOrb } from '@/components/AgentOrb'
import { SettingsSurface } from '../components/SettingsSurface'
import type { SettingsPageProps } from '../types'
import { configResetOnboarding } from '@/lib/tauri'
import { toast } from 'sonner'
import { broadcastChange } from '@/lib/crossWindowSync'

const btnOutline =
  'window-no-drag h-7 rounded-xl border border-black/[0.09] bg-black/[0.025] px-3 text-[11.5px] font-medium shadow-none hover:bg-black/[0.05]'

const btnPrimary =
  'window-no-drag h-7 rounded-xl bg-jade px-3 text-[11.5px] font-medium text-white shadow-none hover:bg-jade/90'

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="mb-3 text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
      {children}
    </div>
  )
}

interface FeatureCardProps {
  icon: typeof Shield
  title: string
  text: string
  accent: string
}

function FeatureCard({ icon: Icon, title, text, accent }: FeatureCardProps) {
  return (
    <div className="flex flex-col gap-2 rounded-xl border border-black/[0.06] bg-black/[0.016] px-4 py-3">
      <div
        className="flex size-8 items-center justify-center rounded-xl"
        style={{ background: `${accent}14` }}
      >
        <Icon className="h-4 w-4" style={{ color: accent }} />
      </div>
      <div>
        <div className="text-[12.5px] font-semibold tracking-tight">{title}</div>
        <p className="mt-0.5 text-[11px] leading-4 text-muted-foreground">{text}</p>
      </div>
    </div>
  )
}

export function AboutSettingsPage({}: SettingsPageProps) {
  return (
    <div className="flex flex-col gap-3">
      {/* ── Hero ── */}
      <SettingsSurface className="px-6 py-6">
        <div className="flex flex-col items-center gap-5 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex items-center gap-4">
            <AgentOrb status="idle" size="hero" />
            <div className="space-y-0.5">
              <div className="text-[19px] font-bold tracking-tight">If2Ai</div>
              <div className="text-[11.5px] text-muted-foreground">桌面 AI 智能体工作台</div>
              <div className="font-mono text-[10px] text-black/25">v0.1.0</div>
            </div>
          </div>

          <div className="flex flex-col gap-2.5 sm:items-end">
            <p className="max-w-[280px] text-[12px] leading-5 text-muted-foreground sm:text-right">
              基于 Tauri + Rust + React 的桌面智能体工作台，强调项目分组、会话流式输出、可观测性和本地化执行体验。
            </p>
            <div className="flex gap-2">
              <Button variant="outline" className={btnOutline}>
                <ExternalLink className="mr-1.5 h-3 w-3" />
                查看文档
              </Button>
              <Button className={btnPrimary}>
                <Rocket className="mr-1.5 h-3 w-3" />
                检查更新
              </Button>
            </div>
          </div>
        </div>
      </SettingsSurface>

      {/* ── Design principles ── */}
      <SettingsSurface className="px-5 py-4">
        <SectionLabel>设计原则</SectionLabel>
        <div className="grid gap-2 sm:grid-cols-3">
          <FeatureCard
            icon={Sparkles}
            title="Agent 优先"
            text="层次克制、信息密度清晰"
            accent="var(--jade)"
          />
          <FeatureCard
            icon={Globe}
            title="跨平台"
            text="macOS / Windows / Linux 统一桌面壳"
            accent="#3b82f6"
          />
          <FeatureCard
            icon={Shield}
            title="可扩展"
            text="未来接入插件、工具面板与评估系统"
            accent="#8b5cf6"
          />
        </div>
      </SettingsSurface>

      {/* ── Danger zone ── */}
      <SettingsSurface className="px-5 py-3.5">
        <div className="flex items-center justify-between">
          <div>
            <div className="text-[12.5px] font-medium">重置 Onboarding</div>
            <div className="mt-0.5 text-[11px] text-muted-foreground">
              清除配置并重新进入引导流程
            </div>
          </div>
          <Button
            variant="outline"
            className={btnOutline}
            onClick={async () => {
              if (!window.confirm('确定要重置 Onboarding 吗？所有配置将被清除。')) return
              try {
                await configResetOnboarding()
                // 跨窗口通知主窗口：立即重新加载 app state，进入 Onboarding 而不需要重启
                void broadcastChange('cross:onboarding-reset', {})
                toast.success('Onboarding 已重置', { description: '主窗口将立即重新进入引导流程' })
              } catch (error) {
                toast.error('重置失败', { description: String(error) })
              }
            }}
          >
            <RotateCcw className="mr-1.5 h-3 w-3" />
            重置
          </Button>
        </div>
      </SettingsSurface>
    </div>
  )
}
