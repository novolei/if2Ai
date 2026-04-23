/**
 * ActivationGate — full-screen activation modal mounted by the boot
 * shell when the canonical projection says the main shell is blocked.
 *
 * 1:1 visual port of UClaw's `ActivationGateView.swift`:
 *
 *   - frosted backdrop ([`GateBackdrop`])
 *   - red/orange diagonal-stripe card on a light base
 *     ([`GateCardStripes`])
 *   - centered Fibonacci digit sphere ([`DigitSphere`])
 *   - 8-cell OTP with five visual phases ([`OtpCells`])
 *   - status text + dots loader + primary "开始激活" CTA + manual
 *     input toggle ([`DotsLoading`])
 *   - top-left logo, top-right beta-invite badge
 *     ([`BetaInviteBadge`])
 *   - bottom-right device indicator ([`DeviceIndicator`])
 *
 * State machine logic lives in [`useActivationGate`].  This file
 * stays presentation-only.
 */

import { useEffect, useRef } from 'react'

import { Wand2 } from 'lucide-react'

import appIconUrl from '@/assets/app-icon.png'

import { useActivationGate } from './useActivationGate'

import { BetaInviteBadge } from './components/BetaInviteBadge'
import { DeviceIndicator } from './components/DeviceIndicator'
import { DigitSphere } from './components/DigitSphere'
import { DotsLoading } from './components/DotsLoading'
import { GateBackdrop } from './components/GateBackdrop'
import { GateCardStripes } from './components/GateCardStripes'
import { OtpCells } from './components/OtpCells'

const ACTIVATION_CODE_LENGTH = 8
const APP_VERSION_LABEL = 'v0.3.0'

interface ActivationGateProps {
  onCloseRequested?: () => void
}

export function ActivationGate({ onCloseRequested }: ActivationGateProps) {
  const [state, actions] = useActivationGate()
  const manualInputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (state.closeRequested) {
      onCloseRequested?.()
    }
  }, [state.closeRequested, onCloseRequested])

  useEffect(() => {
    if (state.isManualInputMode) {
      manualInputRef.current?.focus()
    }
  }, [state.isManualInputMode])

  // Sphere pauses while OTP is filling / verifying for legibility.
  const sphereAnimating = !(
    state.otpPhase === 'filling' || state.otpPhase === 'verifying'
  )

  return (
    <div
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="activation-gate-title"
      className="fixed inset-0 z-[1000] flex items-center justify-center"
    >
      <GateBackdrop />

      <div
        className="relative mx-6 w-full max-w-[720px] overflow-hidden rounded-[26px] shadow-2xl"
        style={{ minHeight: 520 }}
      >
        <GateCardStripes />

        {/* Top-left app icon (matches macOS dock icon).  Falls back to
            a soft tile if the asset ever fails to load so the layout
            never collapses. */}
        <img
          src={appIconUrl}
          alt="if2AI"
          draggable={false}
          className="pointer-events-none absolute left-6 top-6 h-[80px] w-[80px] select-none rounded-2xl bg-white/60 object-cover shadow-md ring-1 ring-zinc-300/40"
          onError={(e) => {
            ;(e.currentTarget as HTMLImageElement).style.visibility = 'hidden'
          }}
        />

        {/* Top-right beta-invite badge */}
        <div className="absolute right-6 top-10">
          <BetaInviteBadge />
        </div>

        {/* Top-right version chip (UClaw shows "vX.Y.Z" in white at the very corner) */}
        <div
          className="pointer-events-none absolute right-8 top-2 text-[11px] font-medium"
          style={{ color: 'rgba(255,255,255,0.7)' }}
        >
          {APP_VERSION_LABEL}
        </div>

        {/* Main column */}
        <div className="relative flex flex-col items-center gap-[18px] px-7 pb-6 pt-5">
          <h2 id="activation-gate-title" className="sr-only">
            激活检查
          </h2>

          <div style={{ height: 24 }} />

          <DigitSphere isAnimating={sphereAnimating} />

          <div style={{ height: 24 }} />

          {!state.isManualInputMode && (
            <OtpCells
              code={state.otpDisplayCode}
              digitsCount={ACTIVATION_CODE_LENGTH}
              phase={state.otpPhase}
            />
          )}

          <p
            className="max-w-[560px] text-center text-[15px] leading-relaxed text-zinc-600"
            aria-live="polite"
          >
            {state.statusText}
          </p>

          <DotsLoading visible={state.isWorking} />

          {/* Action row */}
          <div className="flex min-h-[44px] items-center gap-[10px]">
            {state.stage === 'idle' && (
              <button
                type="button"
                onClick={actions.startAutoActivation}
                disabled={state.isWorking || !state.installationId}
                className="inline-flex h-11 w-[196px] items-center justify-center gap-2 rounded-xl text-[14px] font-semibold text-white shadow-md transition-transform active:translate-y-0.5 disabled:opacity-50"
                style={{
                  background:
                    'linear-gradient(135deg, rgb(255,135,51), rgb(242,77,26))',
                  boxShadow: '0 8px 18px rgba(243,89,31,0.32)',
                }}
              >
                <Wand2 className="h-4 w-4" />
                开始激活
              </button>
            )}
            {state.stage !== 'idle' && state.canRetry && (
              <button
                type="button"
                onClick={actions.retryActivation}
                className="inline-flex h-10 items-center rounded-xl bg-orange-500 px-4 text-[14px] font-semibold text-white hover:bg-orange-600"
              >
                {state.retryButtonTitle}
              </button>
            )}
            {state.stage === 'offlineGrace' && (
              <button
                type="button"
                onClick={actions.dismissForOfflineGrace}
                className="inline-flex h-10 items-center rounded-xl bg-white/85 px-4 text-[14px] font-medium text-zinc-700 hover:bg-white"
              >
                继续离线使用
              </button>
            )}
          </div>

          {/* Manual input toggle */}
          <button
            type="button"
            onClick={actions.toggleManualInput}
            className="text-[12px] font-medium text-orange-600 hover:text-orange-700"
          >
            {state.isManualInputMode ? '使用自动激活' : '手动输入激活码'}
          </button>

          {state.isManualInputMode && (
            <ManualCodeInput
              code={state.manualCodeInput.toUpperCase()}
              phase={state.otpPhase}
              onChange={(v) => actions.setManualCodeInput(v)}
              onSubmit={actions.submitManualCode}
              inputRef={manualInputRef}
            />
          )}

          <div className="flex w-full justify-end pr-2">
            <DeviceIndicator
              indicator={state.deviceIndicator || '----'}
              fullId={state.installationId}
            />
          </div>
        </div>
      </div>
    </div>
  )
}

interface ManualCodeInputProps {
  code: string
  phase: import('./components/OtpCells').OtpPhase
  onChange: (value: string) => void
  onSubmit: () => void
  inputRef: React.RefObject<HTMLInputElement | null>
}

function ManualCodeInput({
  code,
  phase,
  onChange,
  onSubmit,
  inputRef,
}: ManualCodeInputProps) {
  return (
    <div
      className="relative inline-block"
      onClick={() => inputRef.current?.focus()}
    >
      <OtpCells code={code} digitsCount={ACTIVATION_CODE_LENGTH} phase={phase} />
      <input
        ref={inputRef}
        type="text"
        inputMode="text"
        autoComplete="off"
        autoCorrect="off"
        spellCheck={false}
        value={code}
        onChange={(e) => {
          const sanitized = e.target.value
            .replace(/[^a-zA-Z0-9]/g, '')
            .toUpperCase()
            .slice(0, ACTIVATION_CODE_LENGTH)
          onChange(sanitized)
          if (sanitized.length === ACTIVATION_CODE_LENGTH) {
            onSubmit()
          }
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter' && code.length === ACTIVATION_CODE_LENGTH) {
            e.preventDefault()
            onSubmit()
          }
        }}
        className="absolute inset-0 h-full w-full opacity-0"
        aria-label="激活码"
      />
    </div>
  )
}
