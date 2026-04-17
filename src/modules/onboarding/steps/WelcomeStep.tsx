/**
 * WelcomeStep — Step 1 of the 6-step Onboarding flow.
 *
 * Pixel-accurate reproduction of the Welcome step layout:
 * - Full-width 6-step progress bar at top
 * - Orange step badge + title
 * - Welcome heading + subtitle
 * - Info bar, stats bar, feature cards
 *
 * Design reference: docs/references/onboarding-steps/Welcome.png
 */

import { type MouseEvent as ReactMouseEvent, useState } from 'react';
import { Terminal, LayoutGrid, Package, Smartphone } from 'lucide-react';
import { cn } from '@/lib/utils';
import { InfoPanel, InfoCard } from '../components/InfoPanel';
import { OnboardingLayout } from '../components/OnboardingLayout';
import { StepNavigation } from '../components/StepNavigation';
import { StepProgressBar } from '../components/StepProgressBar';

interface WelcomeStepProps {
  onNext: () => void;
  onWindowDrag?: (event: ReactMouseEvent<HTMLElement>) => void;
}

/** 4 feature highlights. */
const FEATURES = [
  {
    icon: <Terminal className="h-4 w-4" />,
    title: '无需命令行',
    description: '无需命令行，把安装步骤折叠进清单补充，适合第一次接触 if2AI 的用户',
  },
  {
    icon: <LayoutGrid className="h-4 w-4" />,
    title: '可视化管理',
    description: '模型、渠道、Hooks 与运行状态都在统一界面配置',
  },
  {
    icon: <Package className="h-4 w-4" />,
    title: '一键安装能力',
    description: '官方 Skills，推荐项与基础架构由向导直接完成初始配置',
  },
  {
    icon: <Smartphone className="h-4 w-4" />,
    title: '双端一体体验',
    description: 'macOS 与 iOS 目标风格保持一致，减少跨设备学习成本',
  },
];

export function WelcomeStep({ onNext, onWindowDrag }: WelcomeStepProps) {
  const [currentStep] = useState(1);

  return (
    <OnboardingLayout
      rightPanel={
        <InfoPanel
          stepLabel="STEP 1: WELCOME"
          title="让 if2AI 在几分钟内就绪。"
          bullets={[
            '每一步都有实时反馈，不让你猜',
            '可随时退出重来，不会丢失任何操作',
            '配置完成后随时可在主界面调整',
          ]}
        >
          <div className="mt-4 flex flex-col gap-2.5">
            <span className="text-token-md font-bold">6 个步骤</span>
            <span
              className="text-token-xs"
              style={{ color: 'rgba(255,255,255,0.6)' }}
            >
              约 3 分钟即可完成
            </span>
            {[
              { num: 1, label: '欢迎向导', desc: '了解 if2AI 安装流程' },
              { num: 2, label: '系统预检', desc: '检查并安装 Embedded 模型' },
              { num: 3, label: '安全确认', desc: '了解所有安全注意事项' },
              { num: 4, label: '模型服务商', desc: '选择并验证 AI 模型来源' },
              { num: 5, label: '通讯渠道', desc: '接入社交/消息平台' },
              { num: 6, label: '激活', desc: '唤醒 if2AI Agent 并完成配置' },
            ].map((s) => (
              <InfoCard key={s.num} className="flex items-start gap-2.5">
                <div
                  className={cn(
                    'shrink-0 mt-0.5 flex h-5 w-5 items-center justify-center rounded-full text-token-xs font-bold',
                    s.num === 1
                      ? 'bg-white/30 text-white'
                      : 'bg-white/20 text-white/70',
                  )}
                >
                  {s.num}
                </div>
                <div>
                  <div className="text-token-sm font-medium text-white">
                    {s.label}
                  </div>
                  <div
                    className="text-token-xs"
                    style={{ color: 'rgba(255,255,255,0.7)' }}
                  >
                    {s.desc}
                  </div>
                </div>
              </InfoCard>
            ))}
          </div>
        </InfoPanel>
      }
      onWindowDrag={onWindowDrag}
    >
      {/* ── Full-width progress bar ── */}
      <StepProgressBar currentStep={currentStep} />

      {/* ── Inline step badge + heading ── */}
      <div className="flex items-center gap-3 px-8 pb-2">
        <div className="flex h-8 w-8 items-center justify-center rounded-full bg-brand-orange text-token-md font-bold text-white shrink-0">
          {currentStep}
        </div>
        <h1 className="text-token-3xl font-bold text-foreground font-sans tracking-tight">
          欢迎来到 if2AI
        </h1>
      </div>

      {/* ── Subtitle ── */}
      <div className="px-8 pb-2">
        <p className="text-token-sm text-muted-foreground">
          准备好养一只属于你的 AI 小宠仔了吗？
        </p>
      </div>

      {/* ── Left content area ── */}
      <div className="flex-1 overflow-y-auto px-8 pb-6">
        {/* Info bar */}
        <div className="mb-4 rounded-lg bg-status-success-bg/40 border border-status-success/20 px-4 py-3">
          <div className="flex items-start gap-2">
            <div className="mt-0.5 text-status-success">
              <svg
                width="16"
                height="16"
                viewBox="0 0 16 16"
                fill="none"
                xmlns="http://www.w3.org/2000/svg"
              >
                <path
                  d="M8 1.5C4.41 1.5 1.5 4.41 1.5 8s2.91 6.5 6.5 6.5 6.5-2.91 6.5-6.5S11.59 1.5 8 1.5Zm0 11c-2.48 0-4.5-2.02-4.5-4.5S5.52 3.5 8 3.5s4.5 2.02 4.5 4.5-2.02 4.5-4.5 4.5Z"
                  fill="currentColor"
                />
                <path d="M8 5.5a.75.75 0 0 0-.75.75v3a.75.75 0 0 0 1.5 0v-3A.75.75 0 0 0 8 5.5Z" fill="currentColor" />
                <circle cx="8" cy="11" r="0.75" fill="currentColor" />
              </svg>
            </div>
            <p className="text-token-xs text-foreground/80 leading-relaxed">
              向导全程有实时反馈，完成后所有配置随时可在主界面调整。
            </p>
          </div>
        </div>

        {/* Stats bar */}
        <div className="mb-5 rounded-lg border border-border bg-muted/30 px-4 py-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <span className="text-token-xs text-muted-foreground">安装方式</span>
              <span className="text-token-sm font-medium text-foreground">6 步引导</span>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-token-xs text-muted-foreground">预计耗时</span>
              <span className="text-token-sm font-medium text-foreground">约 3 分钟</span>
            </div>
          </div>
        </div>

        {/* Why recommend section */}
        <div className="mb-4">
          <span className="text-token-xs text-muted-foreground">
            为什么推荐使用向导开始
          </span>
        </div>

        {/* Feature cards — grouped in one rounded container */}
        <div className="rounded-xl border border-border bg-card overflow-hidden">
          {FEATURES.map((f, i) => (
            <div
              key={i}
              className={cn(
                'flex items-start gap-3 px-4 py-3.5',
                i < FEATURES.length - 1 && 'border-b border-border',
              )}
            >
              {/* Icon container */}
              <div
                className="shrink-0 mt-0.5 flex h-7 w-7 items-center justify-center text-brand-orange"
                aria-hidden="true"
              >
                {f.icon}
              </div>

              {/* Text content */}
              <div className="flex flex-col gap-0.5 min-w-0">
                <h3 className="text-token-sm font-semibold text-foreground font-sans">
                  {f.title}
                </h3>
                <p className="text-token-xs text-muted-foreground leading-relaxed">
                  {f.description}
                </p>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* ── Bottom navigation ── */}
      <StepNavigation
        currentStep={1}
        onNext={onNext}
        onPrev={() => {}}
        canGoNext
        canGoPrev={false}
        nextLabel="开始 6 步安装向导 →"
      />
    </OnboardingLayout>
  );
}
