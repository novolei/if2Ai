/**
 * Bottom-right "设备识别码: XXXX-XXXX" line with click-to-copy.
 */

import { useState } from 'react'

interface DeviceIndicatorProps {
  indicator: string
  /** Full installation_id; copied on click for support purposes. */
  fullId?: string
}

export function DeviceIndicator({ indicator, fullId }: DeviceIndicatorProps) {
  const [copied, setCopied] = useState(false)
  const onCopy = () => {
    if (!fullId) return
    void navigator.clipboard?.writeText(fullId).then(() => {
      setCopied(true)
      window.setTimeout(() => setCopied(false), 1400)
    })
  }
  return (
    <button
      type="button"
      onClick={onCopy}
      title={fullId ? '点击复制完整 installation_id' : undefined}
      className="select-text text-[12px] text-zinc-500 transition-colors hover:text-zinc-700"
    >
      设备识别码：<span className="font-mono">{indicator}</span>
      {copied && <span className="ml-1 text-emerald-600">已复制</span>}
    </button>
  )
}
