import { ArrowLeft, ScrollText } from 'lucide-react'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { toast } from 'sonner'

import { Button } from '@/components/ui/button'

import { JiaochangAudioProvider } from './audio/JiaochangAudioProvider'
import { JiaochangMusicPlayer } from './components/JiaochangMusicPlayer'
import { PixelStage } from './components/PixelStage'
import { PathReplayPanel } from './components/PathReplayPanel'
import { RunInspector } from './components/RunInspector'
import { StrategyPanel } from './components/StrategyPanel'
import { TimelinePanel } from './components/TimelinePanel'
import type { JiaochangRuntimeAnchor } from './data/jiaochang-types'
import { getReplayStepById } from './data/path-replay'
import { getJiaochangViewModel } from './data/selectors'
import { useJiaochangI18n } from './i18n/useJiaochangI18n'

export function JiaochangPage({ onBackToChat }: { onBackToChat: () => void }) {
  const viewModel = useMemo(() => getJiaochangViewModel(), [])
  const { t } = useJiaochangI18n()
  const firstStepId = viewModel.pathReplay.steps[0]?.id ?? null
  const [selectedReplayStepId, setSelectedReplayStepId] = useState<string | null>(firstStepId)
  const [replayPlaying, setReplayPlaying] = useState(false)
  const selectedReplayStep = useMemo(
    () => getReplayStepById(viewModel, selectedReplayStepId),
    [selectedReplayStepId, viewModel],
  )
  const selectedReplayIndex = Math.max(
    0,
    viewModel.pathReplay.steps.findIndex((step) => step.id === selectedReplayStepId),
  )

  const selectReplayIndex = useCallback((nextIndex: number) => {
    if (viewModel.pathReplay.steps.length === 0) return
    const normalizedIndex = (nextIndex + viewModel.pathReplay.steps.length) % viewModel.pathReplay.steps.length
    setSelectedReplayStepId(viewModel.pathReplay.steps[normalizedIndex]?.id ?? null)
  }, [viewModel.pathReplay.steps])

  useEffect(() => {
    if (!replayPlaying || viewModel.pathReplay.steps.length === 0) return
    const timer = window.setTimeout(() => {
      selectReplayIndex(selectedReplayIndex + 1)
    }, 1600)
    return () => window.clearTimeout(timer)
  }, [replayPlaying, selectedReplayIndex, selectReplayIndex, viewModel.pathReplay.steps.length])

  const handleOpenAnchor = useCallback((anchor: JiaochangRuntimeAnchor) => {
    if (anchor.kind === 'chat' || anchor.kind === 'tool') {
      onBackToChat()
    }

    toast.info(t('anchor.opened'), {
      description: describeAnchor(anchor),
    })
  }, [onBackToChat, t])

  return (
    <JiaochangAudioProvider>
      <main className="flex h-full min-h-0 flex-col overflow-hidden bg-[var(--jiaochang-bg,#eef7e6)] text-[var(--jiaochang-text,#2b2218)]">
      <header className="flex items-center justify-between gap-4 border-b border-[var(--jiaochang-border,rgba(43,34,24,0.1))] bg-[var(--jiaochang-header-bg,rgba(255,248,230,0.8))] px-6 py-4 backdrop-blur">
        <div className="min-w-0">
          <div className="flex items-center gap-2 text-[12px] font-semibold text-[var(--jiaochang-accent,#9b3d2d)]">
            <ScrollText className="h-4 w-4" />
            {t('app.kicker')}
          </div>
          <h1 className="font-jiaochang-pixel mt-1 text-[24px] font-semibold tracking-normal text-[var(--jiaochang-text,#2b2218)]">
            {t('app.title')}
          </h1>
        </div>
        <Button
          type="button"
          variant="outline"
          onClick={onBackToChat}
          className="font-jiaochang-pixel h-9 rounded-[6px] border-[var(--jiaochang-border,rgba(43,34,24,0.15))] bg-[var(--jiaochang-panel-bg,rgba(255,255,255,0.7))] px-3 text-[13px] hover:bg-accent hover:text-accent-foreground"
        >
          <ArrowLeft className="mr-2 h-4 w-4" />
          {t('action.backToChat')}
        </Button>
      </header>

      <div className="min-h-0 flex-1 overflow-auto p-5">
        <div className="grid min-h-full gap-4 xl:grid-cols-[minmax(0,1fr)_360px]">
          <div className="flex min-w-0 flex-col gap-4">
            <PixelStage
              agents={viewModel.agents}
              source={viewModel.source}
              replayStep={selectedReplayStep}
              t={t}
            />
            <JiaochangMusicPlayer t={t} />
            <PathReplayPanel
              replay={viewModel.pathReplay}
              selectedStepId={selectedReplayStepId}
              playing={replayPlaying}
              onSelectStep={(stepId) => setSelectedReplayStepId(stepId)}
              onTogglePlaying={() => setReplayPlaying((value) => !value)}
              onPrevious={() => selectReplayIndex(selectedReplayIndex - 1)}
              onNext={() => selectReplayIndex(selectedReplayIndex + 1)}
              onOpenAnchor={handleOpenAnchor}
              t={t}
            />
            <StrategyPanel viewModel={viewModel} t={t} />
            <TimelinePanel events={viewModel.events} onOpenAnchor={handleOpenAnchor} t={t} />
          </div>
          <RunInspector viewModel={viewModel} t={t} />
        </div>
      </div>
      </main>
    </JiaochangAudioProvider>
  )
}

function describeAnchor(anchor: JiaochangRuntimeAnchor) {
  if (anchor.filePath) return anchor.line ? `${anchor.filePath}:${anchor.line}` : anchor.filePath
  if (anchor.toolCallId) return anchor.toolCallId
  if (anchor.messageId) return anchor.messageId
  if (anchor.eventId) return anchor.eventId
  return anchor.label
}
