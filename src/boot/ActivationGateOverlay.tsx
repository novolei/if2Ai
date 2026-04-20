// Phase M2.5 — minimal activation-gate overlay.
//
// Renders a full-screen overlay **only** when the canonical
// activation projection says the main shell is currently blocked.
// Honest scope (mirrors `ActivationService::current_snapshot`):
//
//   - `snapshot.activation === null` → unknown (initial fetch in
//     flight or failed). Render nothing — the existing splash /
//     onboarding flow stays in charge.
//   - `snapshot.activation.allowsMainShell === true` → render
//     nothing.
//   - Otherwise → render the gate overlay with a short status
//     message + explicit `Reload` action (re-fetches via the bridge).
//
// What this slice deliberately does NOT do:
//
//   - It does not redesign the activation UX. The overlay is a
//     deliberate placeholder so M2.6+ has a single mount point to
//     replace.
//   - It does not start onboarding. Onboarding is still gated by
//     the existing `App.tsx` splash/onboarding logic; this overlay
//     only catches the post-onboarding "needs activation" /
//     "revoked" / "expired" / "deactivated" cases.
//   - It does not attempt remote license actions
//     (`request_license` / `redeem` / `refresh` / `revoke_check` /
//     `deactivate`) — those IPC commands do not exist yet.

import { useCallback } from 'react'

import {
  refreshActivationSnapshot,
  useRuntimeProjectionSelector,
  type ActivationProjection,
} from '@/runtime-projection'

const MESSAGE_BY_KIND: Record<ActivationProjection['statusKind'], string> = {
  checking_local: '正在检查本地激活状态…',
  needs_activation: '当前账户尚未激活，请完成激活后再使用主界面。',
  requesting_activation: '激活请求处理中，请稍候…',
  pending_approval: '激活请求已提交，正在等待审批。',
  redeeming: '正在登记激活凭据…',
  activated: '已激活。', // never rendered: allowsMainShell=true
  offline_grace: '当前处于离线宽限期，部分能力可能受限。', // allowsMainShell=true; not blocked
  expired: '激活已过期，请重新激活后继续使用。',
  revoked: '激活已被远端撤销，请联系管理员或重新激活。',
  deactivated: '本设备已被停用。',
}

export function ActivationGateOverlay() {
  const activation = useRuntimeProjectionSelector((s) => s.activation)

  const handleReload = useCallback(() => {
    void refreshActivationSnapshot()
  }, [])

  if (!activation) {
    return null
  }
  if (activation.allowsMainShell) {
    return null
  }

  const message = MESSAGE_BY_KIND[activation.statusKind]
  return (
    <div
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="activation-gate-title"
      className="fixed inset-0 z-[1000] flex items-center justify-center bg-black/60 backdrop-blur-sm"
    >
      <div className="mx-4 max-w-md rounded-2xl bg-white p-6 shadow-2xl dark:bg-zinc-900">
        <h2
          id="activation-gate-title"
          className="text-base font-semibold text-zinc-900 dark:text-zinc-100"
        >
          激活检查
        </h2>
        <p className="mt-2 text-sm leading-relaxed text-zinc-700 dark:text-zinc-300">
          {message}
        </p>
        <p className="mt-3 text-xs text-zinc-500 dark:text-zinc-400">
          状态：<code>{activation.statusKind}</code>
        </p>
        <div className="mt-5 flex justify-end">
          <button
            type="button"
            onClick={handleReload}
            className="rounded-lg bg-zinc-900 px-3 py-1.5 text-sm font-medium text-white hover:bg-zinc-700 dark:bg-zinc-100 dark:text-zinc-900 dark:hover:bg-zinc-300"
          >
            重新检测
          </button>
        </div>
      </div>
    </div>
  )
}
