import { Activity, Clock3, MessagesSquare, TrendingUp, Workflow } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { SettingsSurface } from '../components/SettingsSurface'
import { SettingsMetricCard } from '../components/SettingsMetricCard'
import { SettingsToggleRow } from '../components/SettingsToggleRow'
import type { SettingsPageProps } from '../types'

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

export function UsageSettingsPage({ state, actions }: SettingsPageProps) {
  return (
    <div className="flex flex-col gap-3">
      {/* ── Metric cards ── */}
      <div className="grid gap-2.5 sm:grid-cols-2 xl:grid-cols-4">
        <SettingsMetricCard icon={MessagesSquare} label="今日会话" value="12" detail="最近 24h 聊天轮次" />
        <SettingsMetricCard icon={Workflow} label="工具调用" value="38" detail="自动化与工具执行" />
        <SettingsMetricCard icon={Activity} label="消息总量" value="154" detail="工作区内消息累计" />
        <SettingsMetricCard icon={Clock3} label="累计时长" value="3h 18m" detail="本地处理消耗" />
      </div>

      {/* ── Trends ── */}
      <SettingsSurface className="px-5 py-4">
        <div className="mb-3 flex items-center justify-between">
          <SectionLabel>最近趋势</SectionLabel>
          <Button
            variant="ghost"
            className="window-no-drag -mt-3 h-7 rounded-xl px-2.5 text-[11.5px] text-muted-foreground hover:bg-black/[0.04] hover:text-foreground"
          >
            <TrendingUp className="mr-1.5 h-3.5 w-3.5" />
            刷新统计
          </Button>
        </div>
        <div className="flex flex-col">
          {[
            { label: '今日消息', value: '74' },
            { label: '本周消息', value: '418' },
            { label: '本月消息', value: '1,932' },
          ].map((item, i, arr) => (
            <div key={item.label}>
              <div className="flex items-center justify-between py-2.5">
                <div className="text-[12.5px] font-medium">{item.label}</div>
                <div className="font-mono text-[13px] font-semibold tabular-nums text-foreground/70">
                  {item.value}
                </div>
              </div>
              {i < arr.length - 1 && <div className="border-t border-black/[0.05]" />}
            </div>
          ))}
        </div>
      </SettingsSurface>

      {/* ── Preferences ── */}
      <SettingsSurface className="px-5 py-4">
        <SectionLabel>统计偏好</SectionLabel>
        <div className="flex flex-col">
          <SettingsToggleRow
            title="自动滚动到最新"
            description="保持消息列表跟随输出进度"
            checked={state.autoScroll}
            onCheckedChange={actions.setAutoScroll}
            inline
          />
          <Divider />
          <SettingsToggleRow
            title="完成时桌面通知"
            description="长任务完成后弹出系统通知"
            checked={state.notifications}
            onCheckedChange={actions.setNotifications}
            inline
          />
        </div>
      </SettingsSurface>
    </div>
  )
}
