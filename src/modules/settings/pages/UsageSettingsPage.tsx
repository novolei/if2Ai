import { Activity, Clock3, MessagesSquare, Workflow } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { SettingsSurface } from '../components/SettingsSurface'
import { SettingsMetricCard } from '../components/SettingsMetricCard'
import { SettingsToggleRow } from '../components/SettingsToggleRow'
import type { SettingsPageProps } from '../types'

export function UsageSettingsPage({ state, actions }: SettingsPageProps) {
  return (
    <div className="flex flex-col gap-3">
      {/* Metric cards — standalone, not in a surface */}
      <div className="grid gap-2.5 sm:grid-cols-2 xl:grid-cols-4">
        <SettingsMetricCard icon={MessagesSquare} label="今日会话" value="12" detail="最近 24h 聊天轮次" />
        <SettingsMetricCard icon={Workflow} label="工具调用" value="38" detail="自动化与工具执行" />
        <SettingsMetricCard icon={Activity} label="消息总量" value="154" detail="工作区内消息累计" />
        <SettingsMetricCard icon={Clock3} label="累计时长" value="3h 18m" detail="本地处理消耗" />
      </div>

      {/* Preferences */}
      <SettingsSurface className="px-5 py-4">
        <div className="flex items-center justify-between">
          <div className="text-[13px] font-semibold tracking-tight">统计偏好</div>
          <Button variant="outline" className="window-no-drag h-8 rounded-xl border border-border/60 bg-white/60 px-3.5 text-[12px] shadow-none hover:bg-white/80">
            刷新统计
          </Button>
        </div>
        <div className="mt-3 grid gap-2">
          <SettingsToggleRow
            title="自动滚动到最新"
            description="保持消息列表跟随输出进度"
            checked={state.autoScroll}
            onCheckedChange={actions.setAutoScroll}
          />
          <SettingsToggleRow
            title="完成时桌面通知"
            description="长任务完成后弹出系统通知"
            checked={state.notifications}
            onCheckedChange={actions.setNotifications}
          />
        </div>
      </SettingsSurface>

      {/* Recent trends */}
      <SettingsSurface className="px-5 py-4">
        <div className="text-[13px] font-semibold tracking-tight">最近趋势</div>
        <div className="mt-3 grid gap-2">
          {[
            { label: '今日消息', value: '74' },
            { label: '本周消息', value: '418' },
            { label: '本月消息', value: '1,932' },
          ].map((item) => (
            <div
              key={item.label}
              className="flex items-center justify-between rounded-2xl border border-border/50 bg-white/60 px-4 py-2.5"
            >
              <div className="text-[12px] font-medium">{item.label}</div>
              <div className="text-[13px] font-semibold tabular-nums text-muted-foreground">{item.value}</div>
            </div>
          ))}
        </div>
      </SettingsSurface>
    </div>
  )
}
