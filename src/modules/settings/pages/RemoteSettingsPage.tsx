import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { SettingsSurface } from '../components/SettingsSurface'
import { SettingsToggleRow } from '../components/SettingsToggleRow'
import type { SettingsPageProps } from '../types'

export function RemoteSettingsPage({ state, actions }: SettingsPageProps) {
  return (
    <div className="flex flex-col gap-3.5">
      <SettingsSurface className="p-5">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
          <div>
            <div className="text-[14px] font-semibold tracking-tight">远控通道</div>
            <p className="mt-2 max-w-2xl text-[12px] leading-5 text-muted-foreground">
              在需要时可以开启远程协作通道，让桌面工作台以受控方式被访问。
            </p>
          </div>
          <Badge variant="secondary">本地安全优先</Badge>
        </div>
      </SettingsSurface>

      <SettingsSurface className="p-5">
        <SettingsToggleRow
          title="允许远程协作"
          description="仅在你主动开启时开放远控入口。"
          checked={state.autoScroll}
          onCheckedChange={actions.setAutoScroll}
        />
      </SettingsSurface>

      <div className="grid gap-3.5 lg:grid-cols-2">
        <SettingsSurface className="p-5">
          <div className="space-y-3">
            <div className="text-[14px] font-semibold tracking-tight">配对码</div>
            <div className="rounded-[18px] border border-black/5 bg-white/72 px-4 py-3.5 text-[18px] font-medium tracking-[0.14em]">
              7G2-4QK
            </div>
            <p className="text-[12px] leading-5 text-muted-foreground">
              仅在本地网络环境中有效，过期后自动失效。
            </p>
          </div>
        </SettingsSurface>

        <SettingsSurface className="p-5">
          <div className="space-y-3">
            <div className="text-[14px] font-semibold tracking-tight">访问状态</div>
            <div className="grid gap-2.5">
              {[
                ['局域网访问', '未开启'],
                ['设备授权', '待确认'],
                ['审计日志', '本地保存'],
              ].map(([label, value]) => (
                <div
                  key={label}
                  className="flex items-center justify-between rounded-[18px] border border-black/5 bg-white/72 px-4 py-3.5"
                >
                  <div className="text-[13px] font-medium">{label}</div>
                  <div className="text-[13px] text-muted-foreground">{value}</div>
                </div>
              ))}
            </div>
          </div>
        </SettingsSurface>
      </div>

      <SettingsSurface className="p-5">
        <div className="flex flex-wrap items-center gap-2.5">
          <Button variant="outline" className="window-no-drag h-9 rounded-[18px] border border-black/10 bg-white/84 px-4 text-[13px] shadow-none hover:bg-white">
            复制配对信息
          </Button>
          <Button className="window-no-drag h-9 rounded-[18px] px-4 text-[13px] shadow-none">生成新密钥</Button>
        </div>
      </SettingsSurface>
    </div>
  )
}
