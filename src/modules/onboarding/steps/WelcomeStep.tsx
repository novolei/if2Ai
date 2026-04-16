/**
 * WelcomeStep — Step 1 of the 6-step Onboarding flow.
 *
 * Displays a welcome message, 4 feature highlight cards, and a CTA
 * to begin the onboarding wizard.
 *
 * Design reference: docs/references/onboarding-steps/Welcome.png
 */

import { Shield, Eye, Zap, Rocket } from 'lucide-react';
import { FeatureCard } from '../components/FeatureCard';
import { InfoPanel, InfoCard } from '../components/InfoPanel';
import { OnboardingLayout } from '../components/OnboardingLayout';
import { StepHeader } from '../components/StepHeader';
import { StepNavigation } from '../components/StepNavigation';

interface WelcomeStepProps {
  onNext: () => void;
}

/** 4 feature highlights shown on the welcome page. */
const FEATURES = [
  {
    icon: <Shield className="h-5 w-5" />,
    title: '无隐私争议',
    description: '把服务部署到本地服务器中，这是一次真正的 UClaw 初验！',
  },
  {
    icon: <Eye className="h-5 w-5" />,
    title: '可视化管理',
    description: '模型、渠道、Hooks 与运行状态都会一件配置',
  },
  {
    icon: <Zap className="h-5 w-5" />,
    title: '一键安装能力',
    description: '支持 Skills，所有的自动化能力由你来定义和配置',
  },
  {
    icon: <Rocket className="h-5 w-5" />,
    title: '四选一体验场',
    description: 'xMCl S 和 SDK 四种配置选项，随心体验每个版本',
  },
];

/** Right panel step overview items. */
const OVERVIEW_STEPS = [
  { num: 1, label: '欢迎向导', desc: '了解 UClaw 安装流程' },
  { num: 2, label: '系统预检', desc: '检查并安装 Embedded 模型' },
  { num: 3, label: '安全确认', desc: '了解所有安全注意事项' },
  { num: 4, label: '模型服务商', desc: '选择并验证 AI 模型来源' },
  { num: 5, label: '通讯渠道', desc: '接入社交/消息平台' },
  { num: 6, label: '激活', desc: '唤醒 if2AI Agent 并完成配置' },
];

export function WelcomeStep({ onNext }: WelcomeStepProps) {
  return (
    <OnboardingLayout
      rightPanel={
        <InfoPanel
          stepLabel="WELCOME"
          title="让 if2AI 在几分钟内就绪。"
          bullets={[
            '每一步都有实时反馈，不让你猜',
            '可随时退出重来，不会丢失任何操作',
            '配置完成后随时可在主界面调整',
          ]}
        >
          {/* 6-step overview list */}
          <div className="mt-4 flex flex-col gap-2.5">
            <span className="text-token-md font-bold">6 个步骤</span>
            <span
              className="text-token-xs"
              style={{ color: 'rgba(255,255,255,0.6)' }}
            >
              约 3 分钟即可完成
            </span>
            {OVERVIEW_STEPS.map((s) => (
              <InfoCard key={s.num} className="flex items-start gap-2.5">
                <div
                  className="flex-shrink-0 mt-0.5 flex h-5 w-5 items-center justify-center rounded-full text-token-xs font-bold"
                  style={{
                    background: 'rgba(255,255,255,0.2)',
                    color: '#FFFFFF',
                  }}
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
    >
      <StepHeader currentStep={1} title="欢迎来到 if2AI" />

      {/* Left content area */}
      <div className="flex-1 overflow-y-auto px-8 pb-6">
        {/* Subtitle */}
        <p className="text-token-md text-muted-foreground mb-2">
          准备好你的 AI 助手了吗？
        </p>

        {/* Info bar */}
        <div className="mb-4 rounded-lg bg-status-success-bg/50 border border-status-success/20 px-4 py-2.5">
          <p className="text-token-sm text-foreground/80">
            向导全程有实时反馈，完成后所有配置随时可在主界面调整。
          </p>
        </div>

        {/* Stats bar */}
        <div className="mb-6 flex gap-6">
          <div className="flex items-center gap-2">
            <span className="text-token-xs text-muted-foreground">安装方式</span>
            <span className="text-token-sm font-medium text-foreground">6 步引导</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-token-xs text-muted-foreground">预计耗时</span>
            <span className="text-token-sm font-medium text-foreground">约 3 分钟</span>
          </div>
        </div>

        {/* Feature cards — 2×2 grid */}
        <div className="grid grid-cols-2 gap-4">
          {FEATURES.map((f, i) => (
            <FeatureCard
              key={i}
              icon={f.icon}
              title={f.title}
              description={f.description}
            />
          ))}
        </div>
      </div>

      {/* Bottom navigation */}
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
