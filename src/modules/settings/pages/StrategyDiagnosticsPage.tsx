import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import {
  learningGetActiveStrategies,
  learningGetCandidate,
  learningListCandidates,
  learningResolveActiveOverlay,
  type LearningActiveStrategyOverlay,
  type LearningCandidateStrategy,
  type LearningRolloutState,
  type LearningStrategyIndexEntry,
} from '@/lib/tauri'

const STATE_LABELS: Record<LearningRolloutState, string> = {
  draft: 'Draft',
  candidate: 'Candidate',
  compared: 'Compared',
  recommended: 'Recommended',
  promotion_ready: 'Promotion Ready',
  promotion_blocked: 'Promotion Blocked',
  promoted_candidate: 'Promoted Candidate',
  active: 'Active',
  rolled_back: 'Rolled Back',
  rejected: 'Rejected',
  deprecated: 'Deprecated',
}

const STATE_COLOR: Record<LearningRolloutState, string> = {
  draft: '#6b7280',
  candidate: '#6b7280',
  compared: '#3b82f6',
  recommended: '#8b5cf6',
  promotion_ready: '#10b981',
  promotion_blocked: '#f97316',
  promoted_candidate: '#0ea5e9',
  active: '#16a34a',
  rolled_back: '#94a3b8',
  rejected: '#dc2626',
  deprecated: '#a3a3a3',
}

interface DiagnosticsState {
  loading: boolean
  index: LearningStrategyIndexEntry[]
  actives: LearningCandidateStrategy[]
  overlay: LearningActiveStrategyOverlay | null
  selected: LearningCandidateStrategy | null
  selectedId: string | null
}

const INITIAL_STATE: DiagnosticsState = {
  loading: false,
  index: [],
  actives: [],
  overlay: null,
  selected: null,
  selectedId: null,
}

/**
 * Phase M5 closeout — Strategy Diagnostics governance surface.
 *
 * Reads the candidate strategy registry + active overlay
 * directly from Tauri IPC — no local mock seed data, no
 * second truth source.  This is a **diagnostics** page, not a
 * marketing dashboard: every status badge maps to a typed
 * registry rollout state and every history entry is the
 * append-only audit chain on the backing record.
 */
export function StrategyDiagnosticsPage() {
  const [state, setState] = useState<DiagnosticsState>(INITIAL_STATE)

  const refresh = async (selectedId: string | null = state.selectedId) => {
    setState(prev => ({ ...prev, loading: true }))
    try {
      const [index, actives, overlay] = await Promise.all([
        learningListCandidates(),
        learningGetActiveStrategies(),
        learningResolveActiveOverlay(),
      ])
      let selected: LearningCandidateStrategy | null = null
      let resolvedSelectedId: string | null = selectedId
      if (selectedId) {
        selected = await learningGetCandidate(selectedId)
        if (!selected) {
          resolvedSelectedId = null
        }
      } else if (actives.length > 0) {
        // Default selection: first active strategy.
        selected = actives[0]
        resolvedSelectedId = selected.identity.strategyId
      } else if (index.length > 0) {
        selected = await learningGetCandidate(index[0].strategyId)
        resolvedSelectedId = selected?.identity.strategyId ?? null
      }
      setState({
        loading: false,
        index,
        actives,
        overlay,
        selected,
        selectedId: resolvedSelectedId,
      })
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      toast.error(`治理诊断面加载失败: ${message}`)
      setState(prev => ({ ...prev, loading: false }))
    }
  }

  useEffect(() => {
    void refresh(null)
    // refresh is intentionally not tracked; we re-fetch on
    // user-driven select/refresh.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const onSelect = (strategyId: string) => {
    void refresh(strategyId)
  }

  return (
    <div style={{ padding: 16, color: 'var(--text-color)' }}>
      <header style={{ marginBottom: 16 }}>
        <h2 style={{ margin: 0 }}>策略治理诊断 (Strategy Diagnostics)</h2>
        <p style={{ margin: '4px 0 0', opacity: 0.7, fontSize: 13 }}>
          M5 closeout · 这是治理 / 诊断面，不是 “AI 正在变聪明” 的营销面板。所有数据来自 candidate
          registry IPC 真实 truth source；信息含义见 Rust 模块 doc-comments。
        </p>
      </header>

      <section style={{ marginBottom: 16 }}>
        <h3 style={{ marginBottom: 8 }}>当前 Active Overlay</h3>
        <ActiveOverlayCard overlay={state.overlay} actives={state.actives} />
      </section>

      <div style={{ display: 'grid', gridTemplateColumns: '320px 1fr', gap: 16 }}>
        <section>
          <h3 style={{ marginBottom: 8 }}>Candidate Registry ({state.index.length})</h3>
          <button
            onClick={() => void refresh(state.selectedId)}
            disabled={state.loading}
            style={{ marginBottom: 8 }}
          >
            {state.loading ? '刷新中…' : '刷新'}
          </button>
          <CandidateList
            index={state.index}
            selectedId={state.selectedId}
            onSelect={onSelect}
          />
        </section>

        <section>
          <h3 style={{ marginBottom: 8 }}>Strategy Detail</h3>
          {state.selected ? (
            <CandidateDetail candidate={state.selected} />
          ) : (
            <p style={{ opacity: 0.6 }}>没有可显示的 candidate strategy。</p>
          )}
        </section>
      </div>
    </div>
  )
}

function ActiveOverlayCard(props: {
  overlay: LearningActiveStrategyOverlay | null
  actives: LearningCandidateStrategy[]
}) {
  const { overlay, actives } = props
  if (!overlay || overlay.effects.length === 0) {
    return (
      <div
        style={{
          padding: 12,
          border: '1px solid var(--border-color, #e5e7eb)',
          borderRadius: 6,
          opacity: 0.7,
        }}
      >
        当前没有 Active strategy（singleton-active 已强制；新激活会自动 supersede 之前的 active）。
      </div>
    )
  }
  return (
    <div
      style={{
        padding: 12,
        border: '1px solid var(--border-color, #e5e7eb)',
        borderRadius: 6,
      }}
    >
      <div style={{ marginBottom: 8 }}>
        <strong>overlay version:</strong> <code>{overlay.overlayVersion}</code>
      </div>
      {overlay.effects.map(effect => {
        const active = actives.find(a => a.identity.strategyId === effect.strategyId)
        return (
          <div
            key={effect.strategyId}
            style={{
              marginBottom: 8,
              padding: 8,
              borderRadius: 4,
              background: 'var(--surface-color, #f8fafc)',
            }}
          >
            <div>
              <strong>{effect.label}</strong>{' '}
              <span style={{ opacity: 0.7, fontSize: 12 }}>({effect.kind})</span>
            </div>
            <div style={{ fontSize: 12, opacity: 0.7 }}>id: <code>{effect.strategyId}</code></div>
            <pre
              style={{
                marginTop: 4,
                padding: 6,
                fontSize: 12,
                whiteSpace: 'pre-wrap',
                background: 'var(--code-bg, rgba(0,0,0,0.05))',
                borderRadius: 4,
              }}
            >
              {effect.promptOverlay || '(no prompt overlay text — definition is non-prompt)'}
            </pre>
            {active?.activationAudit && (
              <div style={{ fontSize: 12, opacity: 0.7 }}>
                activated by <code>{active.activationAudit.activatedBy}</code> at{' '}
                <code>{active.activationAudit.activatedAt}</code>
              </div>
            )}
          </div>
        )
      })}
    </div>
  )
}

function CandidateList(props: {
  index: LearningStrategyIndexEntry[]
  selectedId: string | null
  onSelect: (id: string) => void
}) {
  const { index, selectedId, onSelect } = props
  if (index.length === 0) {
    return <p style={{ opacity: 0.6 }}>registry 暂无 candidate strategy。</p>
  }
  return (
    <ul style={{ listStyle: 'none', padding: 0, margin: 0, maxHeight: 480, overflowY: 'auto' }}>
      {index.map(entry => {
        const selected = entry.strategyId === selectedId
        return (
          <li key={entry.strategyId}>
            <button
              onClick={() => onSelect(entry.strategyId)}
              style={{
                width: '100%',
                textAlign: 'left',
                padding: 8,
                border: '1px solid var(--border-color, #e5e7eb)',
                borderRadius: 4,
                marginBottom: 4,
                background: selected ? 'var(--surface-active, #eef2ff)' : 'transparent',
                cursor: 'pointer',
                color: 'inherit',
              }}
            >
              <div style={{ fontWeight: 500 }}>{entry.label}</div>
              <div style={{ display: 'flex', gap: 6, marginTop: 4, fontSize: 12 }}>
                <StatePill state={entry.rolloutState} />
                <span style={{ opacity: 0.7 }}>{entry.sourceKind}</span>
                {entry.hasCompareRef && <span style={{ opacity: 0.7 }}>• compare</span>}
                {entry.hasRecommendationRef && <span style={{ opacity: 0.7 }}>• rec</span>}
              </div>
              <div style={{ fontSize: 11, opacity: 0.6, marginTop: 2 }}>
                <code>{entry.strategyId}</code>
              </div>
            </button>
          </li>
        )
      })}
    </ul>
  )
}

function StatePill(props: { state: LearningRolloutState }) {
  const color = STATE_COLOR[props.state] ?? '#6b7280'
  return (
    <span
      style={{
        display: 'inline-block',
        padding: '1px 6px',
        borderRadius: 9999,
        fontSize: 11,
        background: color,
        color: '#fff',
      }}
    >
      {STATE_LABELS[props.state]}
    </span>
  )
}

function CandidateDetail(props: { candidate: LearningCandidateStrategy }) {
  const c = props.candidate
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
      <SectionCard title="Identity">
        <Row label="strategy_id"><code>{c.identity.strategyId}</code></Row>
        <Row label="label">{c.identity.label}</Row>
        <Row label="state"><StatePill state={c.rolloutState} /></Row>
        <Row label="definition">
          <code>{c.definition.kind}</code>
          {c.definition.kind === 'prompt_overlay' && (
            <pre style={preStyle}>{c.definition.text}</pre>
          )}
          {c.definition.kind === 'discourage_tool' && (
            <pre style={preStyle}>discourage tool: {c.definition.toolName}</pre>
          )}
        </Row>
        <Row label="created_at">{c.createdAt}</Row>
        <Row label="updated_at">{c.updatedAt}</Row>
        {c.identity.policyVersion && <Row label="policy_version">{c.identity.policyVersion}</Row>}
        {c.basedOnReflectionNote && (
          <Row label="based_on_reflection_note">
            <code>{c.basedOnReflectionNote}</code>
          </Row>
        )}
      </SectionCard>

      <SectionCard title="Latest Refs">
        <Row label="compare">
          {c.lastCompareRef ? (
            <span>
              baseline=<code>{c.lastCompareRef.baselineRunId}</code> /
              candidate=<code>{c.lastCompareRef.candidateRunId}</code>
              <span style={{ opacity: 0.6 }}> @ {c.lastCompareRef.recordedAt}</span>
            </span>
          ) : (
            <em style={{ opacity: 0.6 }}>none</em>
          )}
        </Row>
        <Row label="recommendation (compare)">
          {c.lastRecommendationRef ? (
            <span>
              <strong>{c.lastRecommendationRef.decision}</strong>
              <span style={{ opacity: 0.6 }}> [{c.lastRecommendationRef.policyId}]</span>
              <span style={{ opacity: 0.6 }}> @ {c.lastRecommendationRef.recordedAt}</span>
            </span>
          ) : (
            <em style={{ opacity: 0.6 }}>none</em>
          )}
        </Row>
        <Row label="suite eval">
          {c.lastSuiteEvaluationRef ? (
            <span>
              suite=<code>{c.lastSuiteEvaluationRef.suiteId}</code> grade=
              <strong>{c.lastSuiteEvaluationRef.suiteGrade}</strong>
              <span style={{ opacity: 0.6 }}>
                {' '}
                ({c.lastSuiteEvaluationRef.taskCount} tasks,{' '}
                {c.lastSuiteEvaluationRef.regressionCount} regressions)
              </span>
            </span>
          ) : (
            <em style={{ opacity: 0.6 }}>none</em>
          )}
        </Row>
        <Row label="recommendation (suite)">
          {c.lastSuiteRecommendationRef ? (
            <span>
              <strong>{c.lastSuiteRecommendationRef.decision}</strong>
              <span style={{ opacity: 0.6 }}>
                {' '}
                [{c.lastSuiteRecommendationRef.policyId}]
              </span>
            </span>
          ) : (
            <em style={{ opacity: 0.6 }}>none</em>
          )}
        </Row>
      </SectionCard>

      {(c.activationAudit || c.rollbackAudit || c.supersededBy) && (
        <SectionCard title="Activation / Rollback Audit">
          {c.activationAudit && (
            <Row label="activated">
              by <code>{c.activationAudit.activatedBy}</code> at{' '}
              <code>{c.activationAudit.activatedAt}</code>
              {c.activationAudit.note && (
                <div style={{ fontSize: 12, opacity: 0.7 }}>note: {c.activationAudit.note}</div>
              )}
            </Row>
          )}
          {c.rollbackAudit && (
            <Row label="rolled_back">
              by <code>{c.rollbackAudit.initiatedBy}</code> at{' '}
              <code>{c.rollbackAudit.rolledBackAt}</code>
              <div style={{ fontSize: 12, opacity: 0.85 }}>reason: {c.rollbackAudit.reason}</div>
              {c.rollbackAudit.target && (
                <div style={{ fontSize: 12, opacity: 0.7 }}>
                  target: <code>{JSON.stringify(c.rollbackAudit.target)}</code>
                </div>
              )}
            </Row>
          )}
          {c.supersededBy && (
            <Row label="superseded_by">
              <code>{c.supersededBy.supersededBy}</code> at{' '}
              <code>{c.supersededBy.supersededAt}</code>
            </Row>
          )}
        </SectionCard>
      )}

      <SectionCard title={`History (compare ${c.compareHistory?.length ?? 0} · rec ${c.recommendationHistory?.length ?? 0} · suite ${c.suiteEvaluationHistory?.length ?? 0} · activation ${c.activationHistory?.length ?? 0} · rollback ${c.rollbackHistory?.length ?? 0})`}>
        <HistoryList
          title="compare_history"
          items={(c.compareHistory ?? []).map(r => ({
            key: `${r.recordedAt}-${r.candidateRunId}`,
            text: `[${r.recordedAt}] ${r.baselineRunId} → ${r.candidateRunId}`,
          }))}
        />
        <HistoryList
          title="recommendation_history"
          items={(c.recommendationHistory ?? []).map(r => ({
            key: `${r.recordedAt}-${r.decision}`,
            text: `[${r.recordedAt}] ${r.decision} (${r.policyId})`,
          }))}
        />
        <HistoryList
          title="activation_history"
          items={(c.activationHistory ?? []).map(a => ({
            key: a.activatedAt,
            text: `[${a.activatedAt}] by ${a.activatedBy}${a.note ? ` — ${a.note}` : ''}`,
          }))}
        />
        <HistoryList
          title="rollback_history"
          items={(c.rollbackHistory ?? []).map(r => ({
            key: r.rolledBackAt,
            text: `[${r.rolledBackAt}] by ${r.initiatedBy}: ${r.reason}`,
          }))}
        />
      </SectionCard>
    </div>
  )
}

const preStyle: React.CSSProperties = {
  marginTop: 4,
  padding: 6,
  fontSize: 12,
  whiteSpace: 'pre-wrap',
  background: 'var(--code-bg, rgba(0,0,0,0.05))',
  borderRadius: 4,
}

function SectionCard(props: { title: string; children: React.ReactNode }) {
  return (
    <div
      style={{
        padding: 12,
        border: '1px solid var(--border-color, #e5e7eb)',
        borderRadius: 6,
      }}
    >
      <h4 style={{ margin: 0, marginBottom: 8 }}>{props.title}</h4>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>{props.children}</div>
    </div>
  )
}

function Row(props: { label: string; children: React.ReactNode }) {
  return (
    <div style={{ display: 'flex', gap: 8, alignItems: 'flex-start' }}>
      <span
        style={{
          minWidth: 180,
          fontSize: 12,
          opacity: 0.7,
          textTransform: 'uppercase',
        }}
      >
        {props.label}
      </span>
      <span style={{ flex: 1, fontSize: 13 }}>{props.children}</span>
    </div>
  )
}

function HistoryList(props: { title: string; items: { key: string; text: string }[] }) {
  return (
    <div>
      <div style={{ fontSize: 12, fontWeight: 600, opacity: 0.8, marginTop: 8 }}>{props.title}</div>
      {props.items.length === 0 ? (
        <em style={{ fontSize: 12, opacity: 0.6 }}>empty</em>
      ) : (
        <ul style={{ margin: 0, paddingLeft: 16 }}>
          {props.items.map(item => (
            <li key={item.key} style={{ fontSize: 12, opacity: 0.85 }}>
              {item.text}
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}

export default StrategyDiagnosticsPage
