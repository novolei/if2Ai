import { Cloud, Download, Hand, ShieldAlert, Terminal, WifiOff } from 'lucide-react'

import { cn } from '@/lib/utils'
import type { BrowserEntry } from '@/stores/browser-slice'

interface SmartBrowserCockpitProps {
  entry: BrowserEntry
  compact?: boolean
  onRequestCloudEscalation?: () => void
  escalationPending?: boolean
}

const backendLabel: Record<BrowserEntry['backend'], string> = {
  local_rust_cdp: '本地浏览器',
  browser_use_mcp: '智能代理',
  browser_use_cloud: '云端浏览器',
}

const escalationLabel: Record<BrowserEntry['escalationState'], string> = {
  none: '正常',
  suggested: '建议升级',
  approval_required: '等待批准',
  approved: '已批准',
  active: '云端运行',
  blocked: '已阻止',
}

export function SmartBrowserCockpit({
  entry,
  compact = false,
  onRequestCloudEscalation,
  escalationPending = false,
}: SmartBrowserCockpitProps) {
  const hasEscalation = entry.escalationState !== 'none'
  const hasDiagnostics =
    entry.diagnostics.downloads > 0 ||
    entry.diagnostics.console > 0 ||
    entry.diagnostics.networkErrors > 0
  const canRequestCloud =
    !!onRequestCloudEscalation &&
    entry.backend !== 'browser_use_cloud' &&
    entry.escalationState !== 'approved' &&
    entry.escalationState !== 'active'

  return (
    <div
      className={cn(
        'flex flex-wrap items-center gap-1.5 text-[10px]',
        compact ? 'justify-end' : 'justify-between',
      )}
    >
      <span className="inline-flex items-center gap-1 rounded-full bg-black/6 px-2 py-0.5 text-foreground/65">
        <Cloud className="h-2.5 w-2.5" />
        {backendLabel[entry.backend]}
      </span>
      {entry.takenOver && (
        <span className="inline-flex items-center gap-1 rounded-full bg-amber-100 px-2 py-0.5 text-amber-700">
          <Hand className="h-2.5 w-2.5" />
          接管中
        </span>
      )}
      {hasEscalation && (
        <span className="inline-flex items-center gap-1 rounded-full bg-sky-100 px-2 py-0.5 text-sky-700">
          <ShieldAlert className="h-2.5 w-2.5" />
          {escalationLabel[entry.escalationState]}
        </span>
      )}
      {entry.lastAction && (
        <span className="max-w-[96px] truncate rounded-full bg-black/5 px-2 py-0.5 text-foreground/55">
          {entry.lastAction}
        </span>
      )}
      {hasDiagnostics && (
        <span className="inline-flex items-center gap-1 rounded-full bg-black/5 px-2 py-0.5 text-foreground/55">
          <Download className="h-2.5 w-2.5" />
          {entry.diagnostics.downloads}
          <Terminal className="h-2.5 w-2.5" />
          {entry.diagnostics.console}
          <WifiOff className="h-2.5 w-2.5" />
          {entry.diagnostics.networkErrors}
        </span>
      )}
      {canRequestCloud && (
        <button
          type="button"
          onClick={onRequestCloudEscalation}
          disabled={escalationPending}
          className="inline-flex items-center gap-1 rounded-full bg-sky-50 px-2 py-0.5 text-sky-700 transition-colors hover:bg-sky-100 disabled:cursor-wait disabled:opacity-60"
          aria-label="申请云端浏览器"
          title="申请云端浏览器"
        >
          <ShieldAlert className="h-2.5 w-2.5" />
          云端
        </button>
      )}
    </div>
  )
}
