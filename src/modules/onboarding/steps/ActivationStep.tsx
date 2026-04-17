/**
 * ActivationStep — Step 6 (final) of the 6-step Onboarding flow.
 *
 * Displays the configuration summary checklist, security badges, and
 * a large "Wake if2AI Agent" CTA button. Triggers the activation flow:
 * validate → start (sends real LLM greeting) → complete.
 *
 * Design reference: docs/references/onboarding-steps/Activation.png
 */

import type { MouseEvent as ReactMouseEvent } from 'react';
import { useState, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Loader2, Check, Shield, Zap, Globe, Brain, AlertTriangle, Sparkles } from 'lucide-react';
import { cn } from '@/lib/utils';
import { InfoPanel, InfoCard } from '../components/InfoPanel';
import { OnboardingLayout } from '../components/OnboardingLayout';
import { StepNavigation } from '../components/StepNavigation';
import { StepProgressBar } from '../components/StepProgressBar';
import { ActivationChecklist } from '../components/ActivationChecklist';
import { SecurityBadge } from '../components/SecurityBadge';
import { useOnboarding } from '../hooks/useOnboarding';
import type { ActivationChecklist as ActivationChecklistType, ActivationResult } from '../types';

interface ActivationStepProps {
  onNext: () => void | Promise<void>;
  onPrev: () => void;
  onWindowDrag?: (event: ReactMouseEvent<HTMLElement>) => void;
}

/** Ceremony phase for the activation success overlay. */
type CeremonyPhase = 'idle' | 'greeting' | 'navigating';

export function ActivationStep({ onNext, onPrev, onWindowDrag }: ActivationStepProps) {
  const {
    activationChecklist,
    loadActivationChecklist,
    loadAppState,
    systemReport,
  } = useOnboarding();

  // Load checklist from backend on mount
  useEffect(() => {
    void loadActivationChecklist();
  }, [loadActivationChecklist]);

  // Local activation state
  const [isActivating, setIsActivating] = useState(false);
  const [activationError, setActivationError] = useState<string | null>(null);
  const [ceremonyPhase, setCeremonyPhase] = useState<CeremonyPhase>('idle');
  const [aiResponse, setAiResponse] = useState<string | null>(null);

  // Build a synthetic checklist from available state if backend checklist is null
  const checklist: ActivationChecklistType | null = activationChecklist ?? {
    system_check: !!systemReport && systemReport.overall.status === 'Pass',
    security_confirmed: true,
    provider_configured: false,
    channels_configured: false,
  };

  // Gating: system_check + security + provider (channels are optional)
  const canWake =
    checklist.system_check &&
    checklist.security_confirmed &&
    checklist.provider_configured;

  // Channels are not gating — show info chip if not configured
  const channelsSkipped = !checklist.channels_configured;

  const handleWakeAgent = useCallback(async () => {
    setIsActivating(true);
    setActivationError(null);
    setCeremonyPhase('idle');
    setAiResponse(null);

    try {
      // Phase 1: Send real greeting to the configured chat model.
      // Backend has a 30s timeout on the greeting — won't hang forever.
      console.log('[activation] Calling activation_start...');
      const result = await invoke<ActivationResult>('activation_start');
      console.log('[activation] activation_start result:', result);
      setAiResponse(result.ai_response ?? null);

      if (!result.success) {
        setActivationError(result.message || '唤醒失败，请重试');
        return;
      }

      // Phase 2: Mark activation as complete and sync to AppConfig
      await invoke('activation_complete');
      await loadAppState();

      // Phase 3: Show ceremony overlay — user manually proceeds to main interface
      setCeremonyPhase('greeting');
    } catch (err) {
      console.error('[activation] Error:', err);
      setActivationError(err instanceof Error ? err.message : '唤醒失败，请重试');
    } finally {
      setIsActivating(false);
    }
  }, [loadAppState]);

  const handleEnterMain = useCallback(async () => {
    setCeremonyPhase('navigating');
    // onNext() is OnboardingApp's loadAppState — awaiting it ensures the
    // backend Ready state is fetched before the parent re-renders.
    await onNext();
  }, [onNext]);

  const showCeremony = ceremonyPhase !== 'idle';

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
      onWindowDrag={onWindowDrag}
    >
      <StepProgressBar currentStep={6} />

      {/* Inline step badge + heading */}
      <div className="flex items-center gap-3 px-8 pb-2">
        <div className="flex h-8 w-8 items-center justify-center rounded-full bg-brand-orange text-token-md font-bold text-white shrink-0">
          6
        </div>
        <h1 className="text-token-3xl font-bold text-foreground font-sans tracking-tight">
          唤醒 if2AI Agent
        </h1>
      </div>

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
          <SecurityBadge allSecure={canWake} />
        </div>

        {/* Channel not-configured info chip */}
        {channelsSkipped && (
          <div className="mb-4 flex items-center gap-2 rounded-lg border border-amber-500/20 bg-amber-500/5 px-3 py-2">
            <AlertTriangle className="h-3.5 w-3.5 shrink-0 text-amber-500" />
            <span className="text-token-xs text-amber-500/80">
              未配置通讯渠道 — Agent 仅支持 Web 端对话，可在设置中后续添加。
            </span>
          </div>
        )}

        {/* Wake button */}
        <div className="flex justify-center">
          <button
            type="button"
            onClick={handleWakeAgent}
            disabled={!canWake || isActivating}
            className={cn(
              'flex items-center gap-3 rounded-lg px-8 py-4 text-token-md font-bold transition-all',
              'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-orange',
              canWake && !isActivating
                ? 'bg-brand-orange text-white hover:bg-brand-orange-dark hover:shadow-token-sm active:translate-y-0.5'
                : 'bg-brand-orange/30 text-white/50 cursor-not-allowed',
            )}
          >
            {isActivating ? (
              <>
                <Loader2 className="h-5 w-5 animate-spin" />
                正在唤醒 if2AI Agent...
              </>
            ) : showCeremony ? (
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

        {/* Success ceremony overlay — AI response appears on the modal */}
        {showCeremony && (
          <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-md animate-in fade-in duration-500">
            <div className="flex flex-col items-center gap-4 rounded-2xl bg-surface-raised px-10 py-10 shadow-2xl max-w-lg mx-4 border border-border/30">
              {/* Sparkle icon with pulse */}
              <div className="relative">
                <div className="absolute inset-0 rounded-full bg-status-success/20 animate-ping" />
                <div className="relative flex h-20 w-20 items-center justify-center rounded-full bg-linear-to-br from-brand-orange to-amber-500 text-white">
                  <Sparkles className="h-10 w-10" />
                </div>
              </div>

              {/* Title */}
              <span className="text-token-lg font-bold text-foreground">
                if2AI Agent 已上线！
              </span>

              {/* AI response displayed directly on the modal */}
              {aiResponse ? (
                <div className="w-full">
                  <div className="flex items-center gap-2 mb-2">
                    <Sparkles className="h-3.5 w-3.5 text-brand-orange" />
                    <span className="text-token-xs font-semibold text-muted-foreground">
                      Agent 的初次问候
                    </span>
                  </div>
                  <div className="rounded-xl border border-border/40 bg-muted/20 px-5 py-4">
                    <p className="text-token-sm text-foreground leading-relaxed whitespace-pre-wrap">
                      {aiResponse}
                    </p>
                  </div>
                </div>
              ) : (
                <div className="w-full rounded-xl border border-border/40 bg-muted/20 px-5 py-4 text-center">
                  <p className="text-token-sm text-muted-foreground">
                    Agent 已唤醒，随时可以开始对话。
                  </p>
                </div>
              )}

              {/* Enter main interface button */}
              {ceremonyPhase === 'greeting' && (
                <button
                  type="button"
                  onClick={handleEnterMain}
                  className={cn(
                    'flex items-center gap-2 rounded-lg px-6 py-3 text-token-sm font-bold transition-all',
                    'bg-brand-orange text-white hover:bg-brand-orange-dark hover:shadow-token-sm active:translate-y-0.5',
                    'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-orange',
                  )}
                >
                  <Zap className="h-4 w-4" />
                  进入主界面
                </button>
              )}
              {ceremonyPhase === 'navigating' && (
                <div className="flex items-center gap-2 text-token-xs text-muted-foreground">
                  <Loader2 className="h-3 w-3 animate-spin" />
                  <span>跳转中...</span>
                </div>
              )}
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
