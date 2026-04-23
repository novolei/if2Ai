/**
 * useActivationGate — state-machine hook driving the boot activation
 * modal.  React-port of UClaw's `ActivationGateViewModel.swift` with
 * the if2Ai backend wired in (`src/lib/tauri.ts` activation IPCs).
 *
 * State-machine surface:
 *
 *   stage     : idle | checkingLocalLicense | requesting |
 *               waitingApproval | redeeming | temporaryUnavailable |
 *               activated | offlineGrace
 *   otpPhase  : idle | filling | verifying | success | error
 *
 * Side effects (in order of typical run):
 *
 *   1. `activation_get_installation_id` → store the indicator.
 *   2. `activation_get_status`           → branch on local license:
 *        - activated     → close modal immediately.
 *        - offlineGrace  → idle screen with "继续离线" CTA.
 *        - else          → wait for user to click "开始激活".
 *   3. On "开始激活":
 *        - `activation_request_license` → if `status==='approved'`,
 *          short-circuit to OTP fill animation + redeem.
 *        - else `activation_poll_request_status` every 2s until
 *          `can_redeem` → OTP fill + redeem.
 *   4. On "手动输入激活码" + 8 chars → `activation_redeem_by_invite_code`.
 *   5. On success → `refreshActivationSnapshot()` (so projection
 *      moves to allowsMainShell=true), close modal after ~2s pause.
 *
 * Retry status comes in via the `activation_retry_status` Tauri
 * event and is mapped to status text (429 / 503 / network).
 */

import { useCallback, useEffect, useRef, useState } from 'react'

import {
  activationGetInstallationId,
  activationGetStatus,
  activationPollRequestStatus,
  activationRedeemByInviteCode,
  activationRedeemWithRequestId,
  activationRequestLicense,
  listenActivationRetryStatus,
  type ActivationErrorDto,
  type ActivationRetryStatusEvent,
} from '@/lib/tauri'
import { refreshActivationSnapshot } from '@/runtime-projection'

import type { OtpPhase } from './components/OtpCells'

export type ActivationStage =
  | 'idle'
  | 'checkingLocalLicense'
  | 'requesting'
  | 'waitingApproval'
  | 'redeeming'
  | 'temporaryUnavailable'
  | 'activated'
  | 'offlineGrace'

export interface ActivationGateState {
  stage: ActivationStage
  otpPhase: OtpPhase
  otpDisplayCode: string
  statusText: string
  isWorking: boolean
  canRetry: boolean
  retryButtonTitle: string
  isManualInputMode: boolean
  manualCodeInput: string
  installationId: string
  deviceIndicator: string
  /** Set after a successful activation so the modal can fade out. */
  closeRequested: boolean
}

export interface ActivationGateActions {
  startAutoActivation: () => void
  retryActivation: () => void
  toggleManualInput: () => void
  setManualCodeInput: (value: string) => void
  submitManualCode: () => void
  dismissForOfflineGrace: () => void
}

const POLL_INTERVAL_MS = 2000
const FILL_PER_CHAR_MS = 140
const VERIFYING_HOLD_MS = 420
const SUCCESS_HOLD_MS = 2000

const isErrorDto = (err: unknown): err is ActivationErrorDto =>
  typeof err === 'object' && err !== null && 'code' in err && 'message' in err

export function useActivationGate(): [ActivationGateState, ActivationGateActions] {
  const [stage, setStage] = useState<ActivationStage>('idle')
  const [otpPhase, setOtpPhase] = useState<OtpPhase>('idle')
  const [otpDisplayCode, setOtpDisplayCode] = useState('')
  const [statusText, setStatusText] = useState('正在初始化…')
  const [isWorking, setIsWorking] = useState(false)
  const [canRetry, setCanRetry] = useState(false)
  const [retryButtonTitle, setRetryButtonTitle] = useState('重试')
  const [isManualInputMode, setManualMode] = useState(false)
  const [manualCodeInput, setManualCodeInput] = useState('')
  const [installationId, setInstallationId] = useState('')
  const [deviceIndicator, setDeviceIndicator] = useState('')
  const [closeRequested, setCloseRequested] = useState(false)

  const pollAbortRef = useRef<AbortController | null>(null)
  const startedRef = useRef(false)

  // Subscribe to retry events
  useEffect(() => {
    let unlisten: (() => void) | null = null
    void listenActivationRetryStatus((event: ActivationRetryStatusEvent) => {
      const { statusCode, attempt, maxAttempts } = event
      if (statusCode === 429) {
        setStatusText(`请求被限流，正在排队重试…(${attempt}/${maxAttempts})`)
      } else if (statusCode === 503) {
        setStatusText(`服务繁忙，正在重试…(${attempt}/${maxAttempts})`)
      } else {
        setStatusText(`网络抖动，正在重试…(${attempt}/${maxAttempts})`)
      }
    }).then((un) => {
      unlisten = un
    })
    return () => {
      unlisten?.()
    }
  }, [])

  const failureText = useCallback((err: unknown) => {
    if (isErrorDto(err)) {
      switch (err.code) {
        case 'queueing':
          return '当前请求量较大，正在排队…'
        case 'busy':
          return '服务正忙，请稍后重试'
        case 'invalid_app_id':
          return '当前应用未在激活服务白名单中（请联系管理员）'
        case 'transport':
          return '网络连接失败，请检查网络后重试'
        case 'decoding':
          return '服务端响应格式异常，请稍后再试'
        case 'not_configured':
          return '激活服务端点未就绪（可设置 IF2AI_ACTIVATION_BASE_URL 指向自建服务）'
        default:
          return err.message || '激活暂不可用，请稍后重试'
      }
    }
    return '激活暂不可用，请稍后重试'
  }, [])

  const failAsTemporary = useCallback(
    (err: unknown) => {
      pollAbortRef.current?.abort()
      pollAbortRef.current = null
      setStage('temporaryUnavailable')
      setOtpPhase('error')
      setIsWorking(false)
      setCanRetry(true)
      setRetryButtonTitle('重试')
      setStatusText(failureText(err))
    },
    [failureText],
  )

  // Auto OTP fill animation derived from server-issued device_request_code
  const playAutoOtp = useCallback(async (code: string) => {
    setOtpPhase('filling')
    setStage('redeeming')
    setStatusText('正在自动填入激活码…')
    setOtpDisplayCode('')
    for (let i = 1; i <= code.length; i++) {
      setOtpDisplayCode(code.slice(0, i))
      await new Promise((r) => setTimeout(r, FILL_PER_CHAR_MS))
    }
    setOtpPhase('verifying')
    setStatusText('正在校验激活码…')
    await new Promise((r) => setTimeout(r, VERIFYING_HOLD_MS))
  }, [])

  const finishSuccess = useCallback(async () => {
    setOtpPhase('success')
    setStage('activated')
    setStatusText('激活成功，正在进入主界面…')
    setCanRetry(false)
    setIsWorking(false)

    // IMPORTANT: hold the modal *before* publishing the new snapshot.
    // `refreshActivationSnapshot()` flips `allowsMainShell` to true, and
    // `ActivationGateOverlay` immediately unmounts <ActivationGate/>,
    // killing the success ✓ animation mid-frame.  Play the ceremony
    // first, *then* hand off to the main shell.
    await new Promise((r) => setTimeout(r, SUCCESS_HOLD_MS))
    await refreshActivationSnapshot()
    setCloseRequested(true)
  }, [])

  const ensureRequestAndPoll = useCallback(
    async (currentInstallationId: string) => {
      setStage('requesting')
      setStatusText('正在创建激活请求…')
      setCanRetry(false)
      try {
        const req = await activationRequestLicense(currentInstallationId)
        if (req.status === 'approved') {
          await playAutoOtp(req.deviceRequestCode)
          const snap = await activationRedeemWithRequestId(
            req.requestId,
            currentInstallationId,
          )
          if (snap.allowsMainShell) {
            await finishSuccess()
          } else {
            failAsTemporary({
              code: 'server_error',
              message: '服务端拒绝兑现，请稍后重试',
            })
          }
          return
        }

        setStage('waitingApproval')
        setStatusText('请求已提交，正在等待审批…')
        setIsWorking(true)
        setOtpPhase('idle')
        setOtpDisplayCode('')

        const ctrl = new AbortController()
        pollAbortRef.current = ctrl
        while (!ctrl.signal.aborted) {
          await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS))
          if (ctrl.signal.aborted) return
          try {
            const status = await activationPollRequestStatus(req.requestId)
            if (status.status === 'approved' || status.canRedeem) {
              await playAutoOtp(req.deviceRequestCode)
              const snap = await activationRedeemWithRequestId(
                req.requestId,
                currentInstallationId,
              )
              if (snap.allowsMainShell) {
                await finishSuccess()
              } else {
                failAsTemporary({
                  code: 'server_error',
                  message: '服务端拒绝兑现，请稍后重试',
                })
              }
              return
            }
            setStatusText('等待管理员审批中…')
          } catch (err) {
            failAsTemporary(err)
            return
          }
        }
      } catch (err) {
        failAsTemporary(err)
      }
    },
    [failAsTemporary, finishSuccess, playAutoOtp],
  )

  const bootstrap = useCallback(async () => {
    if (startedRef.current) return
    startedRef.current = true
    setIsWorking(true)
    setStage('checkingLocalLicense')
    setStatusText('正在检查本地激活状态…')

    let identity: { installationId: string; deviceIndicator: string } | null = null
    try {
      identity = await activationGetInstallationId()
      setInstallationId(identity.installationId)
      setDeviceIndicator(identity.deviceIndicator)
    } catch (err) {
      // Non-fatal — UI keeps "-" for the indicator.
      console.error('[activation-gate] installation_id failed', err)
    }

    try {
      const snap = await activationGetStatus()
      if (snap.allowsMainShell && snap.status.kind === 'activated') {
        setStage('activated')
        setStatusText('已激活，正在进入主界面…')
        setIsWorking(false)
        setCloseRequested(true)
        return
      }
      if (snap.status.kind === 'offline_grace') {
        setStage('offlineGrace')
        setStatusText('当前处于离线宽限期，部分能力可能受限')
        setIsWorking(false)
        setCanRetry(true)
        return
      }
    } catch (err) {
      console.error('[activation-gate] activation_get_status failed', err)
    }

    setStage('idle')
    setIsWorking(false)
    setCanRetry(false)
    setOtpDisplayCode('')
    setOtpPhase('idle')
    setStatusText('准备就绪，点击开始激活')
  }, [])

  // Fire bootstrap once on first mount.
  useEffect(() => {
    void bootstrap()
    return () => {
      pollAbortRef.current?.abort()
    }
  }, [bootstrap])

  // Actions
  const startAutoActivation = useCallback(() => {
    if (isWorking || !installationId) return
    setCanRetry(false)
    void ensureRequestAndPoll(installationId)
  }, [ensureRequestAndPoll, installationId, isWorking])

  const retryActivation = useCallback(() => {
    if (!installationId) return
    setCanRetry(false)
    void ensureRequestAndPoll(installationId)
  }, [ensureRequestAndPoll, installationId])

  const toggleManualInput = useCallback(() => {
    setManualMode((m) => !m)
    setOtpPhase('idle')
    setStatusText((prev) => {
      const next = !isManualInputMode
      return next ? '请粘贴或输入 8 位激活码…' : prev
    })
    if (isManualInputMode) {
      setManualCodeInput('')
    }
  }, [isManualInputMode])

  const submitManualCode = useCallback(() => {
    const code = manualCodeInput.trim()
    if (code.length !== 8) {
      setStatusText('激活码必须是 8 位字母或数字')
      return
    }
    if (!installationId) return
    setIsWorking(true)
    setStage('redeeming')
    setOtpPhase('verifying')
    setOtpDisplayCode(code.toUpperCase())
    setStatusText('正在登记激活凭据…')
    void (async () => {
      try {
        const snap = await activationRedeemByInviteCode(code, installationId)
        if (snap.allowsMainShell) {
          await finishSuccess()
        } else {
          failAsTemporary({
            code: 'server_error',
            message: '服务端拒绝兑现，请稍后重试',
          })
        }
      } catch (err) {
        failAsTemporary(err)
      }
    })()
  }, [failAsTemporary, finishSuccess, installationId, manualCodeInput])

  const dismissForOfflineGrace = useCallback(() => {
    if (stage !== 'offlineGrace' && stage !== 'temporaryUnavailable') return
    setCloseRequested(true)
  }, [stage])

  return [
    {
      stage,
      otpPhase,
      otpDisplayCode,
      statusText,
      isWorking,
      canRetry,
      retryButtonTitle,
      isManualInputMode,
      manualCodeInput,
      installationId,
      deviceIndicator,
      closeRequested,
    },
    {
      startAutoActivation,
      retryActivation,
      toggleManualInput,
      setManualCodeInput,
      submitManualCode,
      dismissForOfflineGrace,
    },
  ]
}
