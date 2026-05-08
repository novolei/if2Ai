import { useState } from 'react'
import { toast } from 'sonner'
import { runDaydreamCycle } from '@/api/memory'
import { useDaydreamHistory } from './use-daydream-history'
import { DaydreamHistoryModal } from './DaydreamHistoryModal'

/// Status row showing the last daydream cycle result with a "Run now"
/// trigger and a history modal on click.
export function DaydreamStatusRow() {
  const history = useDaydreamHistory()
  const [open, setOpen] = useState(false)
  const [running, setRunning] = useState(false)
  const last = history[0]

  let summary: string
  if (!last) {
    summary = 'Never run'
  } else {
    const failed = last.steps.find((s) => s.error)
    if (failed) {
      summary = `failed at ${failed.step}: ${failed.error!.message}`
    } else {
      const counts = last.steps
        .filter((s) => s.examined > 0 || s.mutated > 0)
        .map((s) => `${s.step} ${s.mutated}/${s.examined}`)
        .join(', ')
      summary = counts || 'no-op'
    }
  }

  const onRunNow = async () => {
    setRunning(true)
    try {
      await runDaydreamCycle()
      toast.success('Daydream cycle complete')
    } catch (err) {
      toast.error(`Cycle failed: ${err}`)
    } finally {
      setRunning(false)
    }
  }

  return (
    <div className="flex items-center gap-2 mt-3">
      <button
        type="button"
        onClick={() => setOpen(true)}
        className="flex-1 text-left px-3 py-2 rounded-md border border-border bg-muted/40 hover:bg-muted text-sm"
        title="Click to view cycle history"
      >
        <span className="text-muted-foreground">Last cycle:&nbsp;</span>
        <span>{summary}</span>
      </button>
      <button
        type="button"
        onClick={onRunNow}
        disabled={running}
        className="px-3 py-2 rounded-md border border-border bg-background hover:bg-muted text-sm disabled:opacity-50"
      >
        {running ? 'Running…' : 'Run now'}
      </button>
      {open && <DaydreamHistoryModal history={history} onClose={() => setOpen(false)} />}
    </div>
  )
}
