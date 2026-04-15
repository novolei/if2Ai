import { useEffect } from 'react'
import { toast } from 'sonner'
import { Copy, RotateCw } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { SettingsSurface } from '../components/SettingsSurface'
import { SettingsToggleRow } from '../components/SettingsToggleRow'
import type { SettingsPageProps } from '../types'

const PAIRING_CODE = '7G2-4QK'

export function RemoteSettingsPage({ state, actions }: SettingsPageProps) {
  // Show toast when remote collaboration is toggled
  useEffect(() => {
    if (state.autoScroll) {
      toast.info('远控通道已开启', {
        description: '配对码将在本地网络中生效',
      })
    }
  }, [state.autoScroll])

  const handleCopyCode = () => {
    navigator.clipboard.writeText(PAIRING_CODE).catch(() => {
      toast.error('复制失败，请手动复制')
    })
    toast.success('配对码已复制')
  }

  const handleGenerateKey = () => {
    toast.info('正在生成新密钥...', { duration: 2000 })
  }

  return (
    <div className="flex flex-col gap-3">
      {/* Connection status + toggle */}
      <SettingsSurface className="px-5 py-4">
        <div className="flex items-center justify-between">
          <div>
            <div className="text-[13px] font-semibold tracking-tight">远程协作</div>
            <p className="mt-0.5 text-[12px] leading-5 text-muted-foreground">
              主动开启远控入口，受控方式让桌面工作台被协作访问
            </p>
          </div>
          <Badge variant="secondary" className="shrink-0 text-[10px]">本地安全优先</Badge>
        </div>
        <div className="mt-3">
          <SettingsToggleRow
            title="允许远程协作"
            description="仅在你主动开启时开放远控入口"
            checked={state.autoScroll}
            onCheckedChange={actions.setAutoScroll}
          />
        </div>
      </SettingsSurface>

      {/* Pairing code + status side by side */}
      <div className="grid gap-2.5 lg:grid-cols-2">
        {/* Pairing code */}
        <SettingsSurface className="px-5 py-4">
          <div className="text-[13px] font-semibold tracking-tight">配对码</div>
          <div className="mt-3 flex items-center gap-3">
            <div className="flex-1 rounded-2xl border border-border/50 bg-muted/50 px-4 py-2.5 text-center font-mono text-[20px] font-semibold tracking-[0.14em] text-foreground">
              {PAIRING_CODE}
            </div>
            <Button
              variant="outline"
              size="icon"
              className="shrink-0 h-9 w-9 rounded-xl border border-border/60 bg-white/60 hover:bg-white/80"
              onClick={handleCopyCode}
            >
              <Copy className="h-4 w-4" />
            </Button>
          </div>
          <p className="mt-2 text-[11px] leading-4 text-muted-foreground">
            仅在本地网络环境中有效，过期后自动失效
          </p>
        </SettingsSurface>

        {/* Access status */}
        <SettingsSurface className="px-5 py-4">
          <div className="text-[13px] font-semibold tracking-tight">访问状态</div>
          <div className="mt-3 grid gap-2">
            {[
              ['局域网访问', '未开启', 'bg-muted text-muted-foreground'],
              ['设备授权', '待确认', 'bg-amber-50 text-amber-700'],
              ['审计日志', '本地保存', 'bg-emerald-50 text-emerald-700'],
            ].map(([label, value, cls]) => (
              <div
                key={label as string}
                className="flex items-center justify-between rounded-2xl border border-border/50 bg-white/60 px-4 py-2.5"
              >
                <div className="text-[12px] font-medium">{label as string}</div>
                <span className={`inline-flex rounded-full px-2 py-0.5 text-[11px] font-medium ${cls}`}>
                  {value as string}
                </span>
              </div>
            ))}
          </div>
        </SettingsSurface>
      </div>

      {/* Action buttons */}
      <SettingsSurface className="px-5 py-4">
        <div className="flex items-center gap-2">
          <Button variant="outline" className={buttonClass} onClick={handleGenerateKey}>
            <RotateCw className="mr-1.5 h-3.5 w-3.5" />
            生成新密钥
          </Button>
        </div>
      </SettingsSurface>
    </div>
  )
}

const buttonClass =
  'window-no-drag h-8 rounded-xl border border-border/60 bg-white/60 px-3.5 text-[12px] shadow-none hover:bg-white/80'
