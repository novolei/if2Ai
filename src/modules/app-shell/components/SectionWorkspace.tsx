import { Sparkles } from 'lucide-react'
import { Button } from '@/components/ui/button'
import type { AppSection } from '../types'

export function SectionWorkspace({
  section,
  onBackToChat,
}: {
  section: Exclude<AppSection, 'chat'>
  onBackToChat: () => void
}) {
  const titles = {
    skills: '技能和应用',
    automation: '自动化',
    memory: '记忆',
  } as const

  return (
    <div className="flex h-full min-h-0 items-center justify-center px-8">
      <div className="w-full max-w-2xl rounded-[28px] border border-black/5 bg-white/44 px-8 py-10 text-center shadow-[0_20px_60px_rgba(0,0,0,0.04)] backdrop-blur-[2px]">
        <div className="mx-auto mb-5 flex size-14 items-center justify-center rounded-2xl bg-black/5">
          <Sparkles className="h-7 w-7 text-black/60" />
        </div>
        <h2 className="text-[22px] font-semibold tracking-tight">{titles[section]}</h2>
        <p className="mt-3 text-[13px] leading-6 text-muted-foreground">
          这一页我们先保留成独立视图，后续可以继续按你的需求补技能、应用和自动化内容。
        </p>
        <div className="mt-6 flex justify-center">
          <Button
            type="button"
            onClick={onBackToChat}
            className="h-10 rounded-full bg-black px-4 text-[13px] font-semibold text-white hover:bg-black/90"
          >
            返回 Chat
          </Button>
        </div>
      </div>
    </div>
  )
}
