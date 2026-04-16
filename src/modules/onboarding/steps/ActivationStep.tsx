/**
 * ActivationStep — Step 6 (final) of the 6-step Onboarding flow.
 *
 * Displays the configuration summary checklist, security badges, and
 * a large "Wake if2AI Agent" CTA button. Triggers the activation flow:
 * validate → start → test_message → complete.
 *
 * Design reference: docs/references/onboarding-steps/Activation.png
 */

import { useState, useCallback } from 'react';
import { Loader2, Check, Shield, Zap, Globe, Brain } from 'lucide-react';
import { cn } from '@/lib/utils';
import { InfoPanel, InfoCard } from '../components/InfoPanel';
import { OnboardingLayout } from '../components/OnboardingLayout';
import { StepHeader } from '../components/StepHeader';
import { StepNavigation } from '../components/StepNavigation';
import { ActivationChecklist } from '../components/ActivationChecklist';
import { SecurityBadge } from '../components/SecurityBadge';
import { useOnboarding } from '../hooks/useOnboarding';
import type { ActivationChecklist as ActivationChecklistType } from '../types';

interface ActivationStepProps {
  onNext: () => void;
  onPrev: () => void;
}

export function ActivationStep({ onNext, onPrev }: ActivationStepProps) {
  const {
    activationChecklist,
    wakeAgent,
    systemReport,
  } = useOnboarding();

  // Local activation state
  const [isActivating, setIsActivating] = useState(false);
  const [activationError, setActivationError] = useState<string | null>(null);
  const [activationSuccess, setActivationSuccess] = useState(false);

  // Build a synthetic checklist from available state if backend checklist is null
  const checklist: ActivationChecklistType | null = activationChecklist ?? {
    system_check: !!systemReport && systemReport.overall.status === 'Pass',
    security_confirmed: true,
    provider_configured: false,
    channels_configured: false,
  };

  const allChecklistPass =
    checklist.system_check &&
    checklist.security_confirmed &&
    checklist.provider_configured &&
    checklist.channels_configured;

  const handleWakeAgent = useCallback(async () => {
    setIsActivating(true);
    setActivationError(null);
    setActivationSuccess(false);

    try {
      await wakeAgent();
      setActivationSuccess(true);
      // Delay to show success animation before navigating away
      setTimeout(() => {
        onNext();
      }, 1500);
    } catch (err) {
      setActivationError(err instanceof Error ? err.message : '唤醒失败，请重试');
    } finally {
      setIsActivating(false);
    }
  }, [wakeAgent, onNext]);

  return (
    <OnboardingLayout
      rightPanel={
        <InfoPanel
          stepLabel="STEP 6: 激活"
          title="唤醒 if2AI Agent，并完成渠道配对。"
          bullets={[
            '确认配置无误后一键唤醒',
            '多端连接已就绪',
            '记忆系统已就绪',
          ]}
        >
          {/* Completion status card */}
          <div className="mt-4">
            <InfoCard className="flex flex-col gap-2.5">
              <div className="flex items-center gap-2">
                <div className="flex h-7 w-7 items-center justify-center rounded-full bg-status-success text-white">
                  <Check className="h-4 w-4" />
                </div>
                <span className="text-token-sm font-bold text-white">
                  一切就绪，等你唤醒！
                </span>
              </div>
              <div className="flex flex-col gap-2 text-token-xs" style={{ color: 'rgba(255,255,255,0.7)' }}>
                <div className="flex items-center gap-2">
                  <Shield className="h-3.5 w-3.5 text-status-success" />
                  <span>健康状态: 所有指标正常</span>
                </div>
                <div className="flex items-center gap-2">
                  <Globe className="h-3.5 w-3.5 text-status-success" />
                  <span>多端连接: 已配置渠道</span>
                </div>
                <div className="flex items-center gap-2">
                  <Zap className="h-3.5 w-3.5 text-status-success" />
                  <span>可定制 Agent: 已配置模型</span>
                </div>
                <div className="flex items-center gap-2">
                  <Brain className="h-3.5 w-3.5 text-status-success" />
                  <span>持续进化: 记忆系统就绪</span>
                </div>
              </div>
            </InfoCard>
          </div>
        </InfoPanel>
      }
    >
      <StepHeader currentStep={6} title="唤醒 if2AI Agent" />

      {/* Left content area */}
      <div className="flex-1 overflow-y-auto px-8 pb-6">
        <p className="text-token-sm text-muted-foreground mb-5 leading-relaxed">
          完成所有步骤的设置，现在点击唤醒，让 if2AI Agent 上线。
        </p>

        {/* Configuration summary */}
        <div className="mb-6">
          <ActivationChecklist checklist={checklist} />
        </div>

        {/* Security badge */}
        <div className="mb-6">
          <SecurityBadge allSecure={allChecklistPass} />
        </div>

        {/* Wake button */}
        <div className="flex justify-center">
          <button
            type="button"
            onClick={handleWakeAgent}
            disabled={!allChecklistPass || isActivating}
            className={cn(
              'flex items-center gap-3 rounded-lg px-8 py-4 text-token-md font-bold transition-all',
              'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-orange',
              allChecklistPass && !isActivating
                ? 'bg-brand-orange text-white hover:bg-brand-orange-dark hover:shadow-token-sm active:translate-y-0.5'
                : 'bg-brand-orange/30 text-white/50 cursor-not-allowed',
            )}
          >
            {isActivating ? (
              <>
                <Loader2 className="h-5 w-5 animate-spin" />
                正在唤醒 if2AI Agent...
              </>
            ) : activationSuccess ? (
              <>
                <Check className="h-5 w-5" />
                已就绪！
              </>
            ) : (
              <>
                <Zap className="h-5 w-5" />
                唤醒 if2AI Agent
              </>
            )}
          </button>
        </div>

        {/* Error message */}
        {activationError && (
          <div className="mt-4 rounded-lg border border-status-error/30 bg-status-error-bg/20 px-4 py-3">
            <p className="text-token-sm text-status-error font-medium">
              {activationError}
            </p>
            <button
              type="button"
              onClick={handleWakeAgent}
              className="mt-2 text-token-sm text-brand-orange hover:underline"
            >
              重试唤醒 →
            </button>
          </div>
        )}

        {/* Success overlay */}
        {activationSuccess && (
          <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm">
            <div className="flex flex-col items-center gap-4 rounded-2xl bg-background px-10 py-8 shadow-token-sm">
              <div className="flex h-16 w-16 items-center justify-center rounded-full bg-status-success text-white animate-pulse">
                <Check className="h-8 w-8" />
              </div>
              <span className="text-token-md font-bold text-foreground">
                if2AI Agent 已上线！
              </span>
              <span className="text-token-sm text-muted-foreground">
                正在跳转到主界面...
              </span>
            </div>
          </div>
        )}
      </div>

      {/* Bottom navigation — no Next button on activation step */}
      <StepNavigation
        currentStep={6}
        onNext={onNext}
        onPrev={onPrev}
        canGoNext={false}
        canGoPrev
        nextLabel="已激活"
      />
    </OnboardingLayout>
  );
}
