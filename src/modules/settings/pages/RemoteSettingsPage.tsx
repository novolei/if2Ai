import { useEffect } from 'react'
import { toast } from 'sonner'
import { Copy, RotateCw } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { SettingsSurface } from '../components/SettingsSurface'
import { SettingsToggleRow } from '../components/SettingsToggleRow'
import type { SettingsPageProps } from '../types'
import { cn } from '@/lib/utils'

const PAIRING_CODE = '7G2-4QK'

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="mb-3 text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
      {children}
    </div>
  )
}

function Divider() {
  return <div className="my-0.5 border-t border-black/[0.05]" />
}

export function RemoteSettingsPage({ state, actions }: SettingsPageProps) {
  useEffect(() => {
    if (state.autoScroll) {
      toast.info('远控通道已开启', { description: '配对码将在本地网络中生效' })
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
      {/* ── Remote collaboration toggle ── */}
      <SettingsSurface className="px-5 py-4">
        <div className="mb-3 flex items-center justify-between">
          <SectionLabel>远程协作</SectionLabel>
          <Badge className="mb-3 rounded-xl border-jade/20 bg-jade/[0.07] text-[10px] font-semibold text-jade/80">
            本地安全优先
          </Badge>
        </div>
        <p className="mb-3 text-[11.5px] leading-5 text-muted-foreground">
          主动开启远控入口，受控方式让桌面工作台被协作访问
        </p>
        <div className="flex flex-col">
          <SettingsToggleRow
            title="允许远程协作"
            description="仅在你主动开启时开放远控入口"
            checked={state.autoScroll}
            onCheckedChange={actions.setAutoScroll}
            inline
          />
        </div>
      </SettingsSurface>

      {/* ── Pairing code + status ── */}
      <div className="grid gap-3 lg:grid-cols-2">
        <SettingsSurface className="px-5 py-4">
          <SectionLabel>配对码</SectionLabel>
          <div className="flex items-center gap-2.5">
            <div className="flex-1 rounded-xl border border-black/[0.09] bg-black/[0.025] px-4 py-2 text-center font-mono text-[20px] font-bold tracking-[0.18em]">
              {PAIRING_CODE}
            </div>
            <Button
              variant="outline"
              size="icon"
              className="h-9 w-9 shrink-0 rounded-xl border-black/[0.09] bg-black/[0.025] hover:bg-black/[0.05] shadow-none"
              onClick={handleCopyCode}
            >
              <Copy className="h-3.5 w-3.5" />
            </Button>
          </div>
          <p className="mt-2 text-[11px] text-muted-foreground">
            仅在本地网络环境中有效，过期后自动失效
          </p>
        </SettingsSurface>

        <SettingsSurface className="px-5 py-4">
          <SectionLabel>访问状态</SectionLabel>
          <div className="flex flex-col">
            {(
              [
                ['局域网访问', '未开启', 'text-muted-foreground bg-black/[0.04]'],
                ['设备授权', '待确认', 'text-amber-700 bg-amber-50'],
                ['审计日志', '本地保存', 'text-emerald-700 bg-emerald-50'],
              ] as const
            ).map(([label, value, cls], i, arr) => (
              <div key={label}>
                <div className="flex items-center justify-between py-2.5">
                  <div className="text-[12.5px] font-medium">{label}</div>
                  <span className={cn('rounded-lg px-2 py-0.5 text-[11px] font-semibold', cls)}>
                    {value}
                  </span>
                </div>
                {i < arr.length - 1 && <Divider />}
              </div>
            ))}
          </div>
        </SettingsSurface>
      </div>

      {/* ── Actions ── */}
      <SettingsSurface className="px-5 py-3.5">
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            className="h-7 rounded-xl border-black/[0.09] bg-black/[0.025] px-3.5 text-[11.5px] font-medium shadow-none hover:bg-black/[0.05]"
            onClick={handleGenerateKey}
          >
            <RotateCw className="mr-1.5 h-3 w-3" />
            生成新密钥
          </Button>
        </div>
      </SettingsSurface>
    </div>
  )
}
