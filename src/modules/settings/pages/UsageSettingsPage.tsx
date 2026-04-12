import { Activity, Clock3, MessagesSquare, Workflow } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { SettingsSurface } from '../components/SettingsSurface'
import { SettingsMetricCard } from '../components/SettingsMetricCard'
import { SettingsToggleRow } from '../components/SettingsToggleRow'
import type { SettingsPageProps } from '../types'

export function UsageSettingsPage({ state, actions }: SettingsPageProps) {
  return (
    <div className="flex flex-col gap-3.5">
      <SettingsSurface className="p-5">
        <div className="grid gap-3.5 sm:grid-cols-2 xl:grid-cols-4">
          <SettingsMetricCard icon={MessagesSquare} label="今日会话" value="12" detail="最近 24 小时的聊天轮次。" />
          <SettingsMetricCard icon={Workflow} label="工具调用" value="38" detail="自动化与工具的总执行次数。" />
          <SettingsMetricCard icon={Activity} label="消息总量" value="154" detail="当前工作区内的消息累计。" />
          <SettingsMetricCard icon={Clock3} label="累计时长" value="3h 18m" detail="本地处理和对话消耗。" />
        </div>
      </SettingsSurface>

      <SettingsSurface className="p-5">
        <div className="flex flex-col gap-3.5">
          <div className="flex items-center justify-between gap-4">
            <div>
              <div className="text-[14px] font-semibold tracking-tight">统计偏好</div>
              <p className="mt-1 text-[12px] leading-5 text-muted-foreground">
                这些偏好会影响聊天记录呈现和统计粒度。
              </p>
            </div>
            <Button variant="outline" className="window-no-drag h-9 rounded-[18px] border border-black/10 bg-white/84 px-4 text-[13px] shadow-none hover:bg-white">
              刷新统计
            </Button>
          </div>

          <SettingsToggleRow
            title="自动滚动到最新"
            description="保持消息列表跟随当前输出进度。"
            checked={state.autoScroll}
            onCheckedChange={actions.setAutoScroll}
          />
          <SettingsToggleRow
            title="完成时桌面通知"
            description="当长时间任务完成后弹出系统通知。"
            checked={state.notifications}
            onCheckedChange={actions.setNotifications}
          />
        </div>
      </SettingsSurface>

      <SettingsSurface className="p-5">
        <div className="flex flex-col gap-3.5">
          <div className="text-[14px] font-semibold tracking-tight">最近趋势</div>
          <div className="grid gap-2.5">
            {[
              { label: '今日消息', value: '74' },
              { label: '本周消息', value: '418' },
              { label: '本月消息', value: '1,932' },
            ].map((item) => (
              <div
                key={item.label}
                className="flex items-center justify-between rounded-[18px] border border-black/5 bg-white/72 px-4 py-3.5"
              >
                <div className="text-[13px] font-medium">{item.label}</div>
                <div className="text-[13px] text-muted-foreground">{item.value}</div>
              </div>
            ))}
          </div>
        </div>
      </SettingsSurface>
    </div>
  )
}
