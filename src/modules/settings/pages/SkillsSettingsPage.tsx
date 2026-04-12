import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { SettingsSurface } from '../components/SettingsSurface'
import { SettingsToggleRow } from '../components/SettingsToggleRow'
import type { SettingsPageProps } from '../types'

const SKILL_ITEMS = [
  {
    title: '代码助手',
    description: '生成、解释和优化代码片段。',
    enabled: true,
  },
  {
    title: '文件管理',
    description: '读取、写入和整理本地工作区文件。',
    enabled: true,
  },
  {
    title: '浏览器任务',
    description: '进行网页访问、抓取和验证。',
    enabled: false,
  },
  {
    title: '终端执行',
    description: '通过命令行完成受控自动化任务。',
    enabled: true,
  },
]

export function SkillsSettingsPage({}: SettingsPageProps) {
  return (
    <div className="flex flex-col gap-3.5">
      <SettingsSurface className="p-5">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
          <div>
            <div className="flex flex-wrap items-center gap-2">
              <div className="text-[14px] font-semibold tracking-tight">技能管理</div>
              <Badge variant="secondary">BETA</Badge>
            </div>
            <p className="mt-2 max-w-2xl text-[12px] leading-5 text-muted-foreground">
              管理当前可用技能模块。开启后，主窗口会按能力自动提供更合适的任务入口。
            </p>
          </div>
          <Button variant="outline" className="window-no-drag h-9 rounded-[18px] border border-black/10 bg-white/84 px-4 text-[13px] shadow-none hover:bg-white">
            配置指南
          </Button>
        </div>
      </SettingsSurface>

      <SettingsSurface className="p-5">
        <div className="grid gap-3.5">
          {SKILL_ITEMS.map((item) => (
            <SettingsToggleRow
              key={item.title}
              title={item.title}
              description={item.description}
              checked={item.enabled}
              onCheckedChange={() => {}}
            />
          ))}
        </div>
      </SettingsSurface>

      <SettingsSurface className="p-5">
        <div className="grid gap-2.5 md:grid-cols-2">
          {['代码执行', '文件整理', '任务拆解', '工具编排'].map((label) => (
            <div
              key={label}
              className="rounded-[18px] border border-black/5 bg-white/72 px-4 py-3.5 text-[13px] font-medium"
            >
              {label}
            </div>
          ))}
        </div>
      </SettingsSurface>
    </div>
  )
}
