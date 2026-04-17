/**
 * SecurityConfirmStep — Step 3 of the 6-step Onboarding flow.
 *
 * Displays 7 security risk disclosures in a unified card, plus a final
 * confirmation checkbox. User must check the box before proceeding.
 *
 * Design reference: docs/references/onboarding-steps/SecurityConfirm.png
 */

import type { MouseEvent as ReactMouseEvent } from 'react';
import { useState } from 'react';
import { Check } from 'lucide-react';
import { cn } from '@/lib/utils';
import { InfoPanel } from '../components/InfoPanel';
import { OnboardingLayout } from '../components/OnboardingLayout';
import { StepNavigation } from '../components/StepNavigation';
import { StepProgressBar } from '../components/StepProgressBar';

interface SecurityConfirmStepProps {
  onConfirm: () => void;
  onPrev: () => void;
  onWindowDrag?: (event: ReactMouseEvent<HTMLElement>) => void;
}

/** Risk disclosure items (1-7, informational only). */
const RISK_ITEMS = [
  'if2AI 默认以个人模式运行，所有 AI 请求都会使用你配置的账号。',
  '安装过程在 ~/.if2ai/ 下创建并修改配置文件。',
  '安装完成后会自动启动各服务进程（LocalServer / Gateway）。',
  '本应用会读取并存储你的 AI 凭据（API Key / OAuth Token）。',
  '通讯 Bot 配置需要 App ID、App Secret 等敏感权限。',
  'if2AI 需要 Node.js 18+ 环境。',
  '首次安装可能需要额外权限，或等待后台服务初始化。',
];

export function SecurityConfirmStep({ onConfirm, onPrev, onWindowDrag }: SecurityConfirmStepProps) {
  const [confirmed, setConfirmed] = useState(false);

  return (
    <OnboardingLayout
      rightPanel={
        <InfoPanel
          stepLabel="STEP 3: 安全确认"
          title="先讲清规则，再开跑。"
          bullets={[
            '先确认权限，操作可继续',
            '权限安全审查',
            '开发者通道',
          ]}
        >
          {/* Summary card — semi-transparent on orange bg, matching Step 4 style */}
          <div className="mt-4">
            <div
              className="rounded-2xl overflow-hidden"
              style={{
                background: 'rgba(255, 255, 255, 0.12)',
                backdropFilter: 'blur(8px)',
                border: '1px solid rgba(255, 255, 255, 0.15)',
              }}
            >
              <div className={cn(
                'flex items-center gap-2 px-4 py-3',
                confirmed ? 'bg-white/20' : 'bg-white/10',
              )}>
                <div
                  className={cn(
                    'flex h-5 w-5 items-center justify-center rounded-full',
                    confirmed ? 'bg-[#10B981]' : 'bg-white/20',
                  )}
                >
                  {confirmed ? (
                    <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
                      <path d="M2 5L4.5 7.5L8 3" stroke="white" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
                    </svg>
                  ) : (
                    <span className="text-[10px] font-bold" style={{ color: 'rgba(255,255,255,0.6)' }}>…</span>
                  )}
                </div>
                <span className="text-[13px] font-semibold" style={{ color: 'rgba(255,255,255,0.95)' }}>
                  {confirmed ? '已确认，继续 →' : '请阅读并确认'}
                </span>
              </div>
              <div className="px-4 py-2.5 space-y-1.5">
                <div className="text-[12px]" style={{ color: 'rgba(255,255,255,0.9)' }}>• 可控权限，零滥用</div>
                <div className="text-[12px]" style={{ color: 'rgba(255,255,255,0.9)' }}>• 可控部署</div>
                <div className="text-[12px]" style={{ color: 'rgba(255,255,255,0.9)' }}>• 本地数据，可完整卸载</div>
              </div>
              <div className="px-4 py-2 border-t" style={{ borderColor: 'rgba(255,255,255,0.1)' }}>
                <span className="text-[10px]" style={{ color: 'rgba(255,255,255,0.5)' }}>
                  {confirmed
                    ? '全部风险提示已确认，可以进入下一步'
                    : '请阅读左侧 7 项风险提示并勾选确认'}
                </span>
              </div>
            </div>
          </div>
        </InfoPanel>
      }
      onWindowDrag={onWindowDrag}
    >
      <StepProgressBar currentStep={3} />

      {/* Inline step badge + heading */}
      <div className="flex items-center gap-3 px-8 pb-2">
        <div className="flex h-8 w-8 items-center justify-center rounded-full bg-brand-orange text-token-md font-bold text-white shrink-0">
          3
        </div>
        <h1 className="text-token-3xl font-bold text-foreground font-sans tracking-tight">
          请确认安全说明
        </h1>
      </div>

      {/* Left content area */}
      <div className="flex-1 overflow-y-auto px-8 pb-6">
        <p className="text-token-sm text-muted-foreground mb-5 leading-relaxed">
          AI 配置涉及权限和第三方接入，请仔细阅读以下说明，勾选确认后再继续。
        </p>

        {/* Unified risk disclosure card */}
        <div className="rounded-xl overflow-hidden border border-border bg-[#FAF6F1]">
          {/* Card header */}
          <div className="px-4 py-3 border-b border-border/60">
            <h2 className="text-token-sm font-semibold text-foreground font-sans">
              风险说明
            </h2>
          </div>

          {/* Risk items 1-7 */}
          {RISK_ITEMS.map((desc, i) => (
            <div
              key={i}
              className={cn(
                'flex items-start gap-3 px-4 py-2.5',
                i < RISK_ITEMS.length - 1 && 'border-b border-border/40',
              )}
            >
              {/* Orange number badge */}
              <div className="shrink-0 mt-0.5 flex h-5 w-5 items-center justify-center rounded-full bg-[#E8733A] text-[10px] font-bold text-white">
                {i + 1}
              </div>

              {/* Description */}
              <p className="text-token-sm text-foreground/80 leading-relaxed">
                {desc}
              </p>
            </div>
          ))}
        </div>

        {/* Confirmation checkbox row */}
        <div className="mt-3 flex items-center gap-3 px-2">
          <label className="flex items-center gap-2.5 cursor-pointer">
            <div
              className={cn(
                'flex h-5 w-5 items-center justify-center rounded transition-colors',
                confirmed
                  ? 'bg-blue-500 text-white'
                  : 'border-2 border-border bg-background',
              )}
            >
              {confirmed && <Check className="h-3.5 w-3.5" />}
            </div>
            <input
              type="checkbox"
              checked={confirmed}
              onChange={(e) => setConfirmed(e.target.checked)}
              className="sr-only"
              aria-label="确认已阅读并理解风险提示"
            />
            <span className={cn(
              'text-token-sm',
              confirmed ? 'text-blue-600 font-medium' : 'text-foreground/70',
            )}>
              我已阅读并理解以上说明，同意继续配置
            </span>
          </label>
        </div>
      </div>

      {/* Bottom navigation */}
      <StepNavigation
        currentStep={3}
        onNext={onConfirm}
        onPrev={onPrev}
        canGoNext={confirmed}
        canGoPrev
        nextLabel="确认并继续 →"
      />
    </OnboardingLayout>
  );
}
