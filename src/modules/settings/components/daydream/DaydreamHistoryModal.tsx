import type { DaydreamReportPayload } from '@/transport/contracts'

interface Props {
  history: DaydreamReportPayload[]
  onClose: () => void
}

/// Modal overlay that renders the cap-20 daydream cycle history list.
export function DaydreamHistoryModal({ history, onClose }: Props) {
  return (
    <div
      onClick={onClose}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="bg-background rounded-lg shadow-xl w-[90%] max-w-2xl max-h-[80vh] overflow-auto p-4 border border-border"
      >
        <div className="flex justify-between items-center mb-3">
          <h3 className="text-base font-semibold">Daydream history</h3>
          <button
            type="button"
            onClick={onClose}
            className="text-muted-foreground hover:text-foreground"
            aria-label="Close"
          >
            ×
          </button>
        </div>
        {history.length === 0 ? (
          <p className="text-sm text-muted-foreground">No cycles yet.</p>
        ) : (
          <ul className="space-y-2">
            {history.map((r) => {
              const failed = r.steps.some((s) => s.error)
              return (
                <li
                  key={r.cycleId}
                  className={`p-2 rounded border ${failed ? 'border-destructive/40 bg-destructive/5' : 'border-border'}`}
                >
                  <div className="text-xs text-muted-foreground mb-1">
                    {new Date(r.finishedAt).toLocaleString()} · {r.trigger} · {r.strategy}
                  </div>
                  <ol className="text-sm pl-4 list-decimal space-y-0.5">
                    {r.steps.map((s) => (
                      <li key={s.step}>
                        <span className="font-medium">{s.step}</span>
                        {s.step === 'reflect' && s.extractedCount !== undefined ? (
                          <>
                            : extracted {s.extractedCount} · rejected {s.rejectedCount ?? 0} ·
                            promoted {s.mutated} ({s.durationMs}ms)
                          </>
                        ) : (
                          <>
                            : examined {s.examined} / mutated {s.mutated} ({s.durationMs}ms)
                          </>
                        )}
                        {s.error ? (
                          <span className="text-destructive">
                            {' '}
                            — {s.error.kind}: {s.error.message}
                          </span>
                        ) : null}
                      </li>
                    ))}
                  </ol>
                </li>
              )
            })}
          </ul>
        )}
      </div>
    </div>
  )
}
