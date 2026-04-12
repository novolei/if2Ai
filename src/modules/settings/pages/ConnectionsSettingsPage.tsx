import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import { SettingsSurface } from '../components/SettingsSurface'
import type { SettingsPageProps } from '../types'

const CONNECTION_ITEMS = [
  {
    title: '微信小程序集成',
    description: '接入微信小程序，让用户可通过小程序与 AI 对话。',
    status: '已启用',
    primaryAction: '配置',
    secondaryAction: '指南',
  },
  {
    title: '企业微信集成',
    description: '通过企业微信接收和回复消息。',
    status: '已连接',
    primaryAction: '解绑',
    secondaryAction: '配置',
  },
  {
    title: '客服号集成',
    description: '注册客服号以接收和回复消息。',
    status: '待配置',
    primaryAction: '配置',
    secondaryAction: '指南',
  },
]

export function ConnectionsSettingsPage({}: SettingsPageProps) {
  return (
    <div className="flex flex-col gap-3.5">
      <SettingsSurface className="p-5">
        <div className="space-y-2">
          <div className="text-[14px] font-semibold tracking-tight">连接应用</div>
          <p className="text-[12px] leading-5 text-muted-foreground">
            按需接入外部应用，保持消息收发、任务回调和工具流的统一入口。
          </p>
        </div>
      </SettingsSurface>

      <div className="flex flex-col gap-3.5">
        {CONNECTION_ITEMS.map((item) => (
          <SettingsSurface key={item.title} className="p-5">
            <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
              <div className="space-y-2.5">
                <div className="flex flex-wrap items-center gap-2">
                  <div className="text-[15px] font-semibold tracking-tight">{item.title}</div>
                  <Badge variant="secondary">{item.status}</Badge>
                </div>
                <p className="max-w-2xl text-[12px] leading-5 text-muted-foreground">{item.description}</p>
                <div className="text-[12px] text-primary">配置指南</div>
              </div>

              <div className="flex items-center gap-2.5">
                <Button variant="outline" className="window-no-drag h-9 rounded-[18px] border border-black/10 bg-white/84 px-4 text-[13px] shadow-none hover:bg-white">
                  {item.secondaryAction}
                </Button>
                <Button className="window-no-drag h-9 rounded-[18px] px-4 text-[13px] shadow-none">{item.primaryAction}</Button>
              </div>
            </div>
          </SettingsSurface>
        ))}
      </div>

      <SettingsSurface className="p-5">
        <div className="space-y-4">
          <div className="text-[14px] font-semibold tracking-tight">连接说明</div>
          <Separator className="bg-white/60" />
          <p className="text-[12px] leading-6 text-muted-foreground">
            所有连接能力都会优先保持在本地工作区内，后续可以再逐步接入更细的权限控制和审计记录。
          </p>
        </div>
      </SettingsSurface>
    </div>
  )
}
