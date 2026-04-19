/**
 * MemoryDebugTab — Phase 8B Phase C verification UI.
 *
 * One-click smoke test for the 5 compile commands shipped in 8B.1-8B.5:
 *   - memory_compile_now (×2 to verify fingerprint cache)
 *   - memory_compiled_read
 *   - memory_compiled_clear
 *   - memory_compile_now (after clear, to verify fresh compile)
 *
 * Each step's outcome is rendered inline with timing, status badge,
 * and (for `read`) a markdown preview.  Useful for hitting Phase 8B's
 * human checkpoint #1 without dropping into devtools console.
 */

import { useEffect, useState } from 'react'
import { Activity, CheckCircle2, Loader2, Pin, Play, RefreshCw, XCircle } from 'lucide-react'
import { SettingsSurface } from '../components/SettingsSurface'
import {
  memoryCompileNow,
  memoryCompiledClear,
  memoryCompiledRead,
  pinnedGet,
  type CompileReport,
  type CompiledMemoryDto,
  type PinnedItemDto,
} from '@/lib/tauri'

type Scope = 'current' | 'all' | 'project' | 'global'

type StepStatus = 'idle' | 'running' | 'success' | 'error'

interface StepResult {
  status: StepStatus
  elapsedMs?: number
  error?: string
  /** Pretty-printed payload for inline display. */
  payload?: string
}

interface AllSteps {
  step1_first_compile: StepResult
  step2_cache_hit: StepResult
  step3_read: StepResult
  step4_clear: StepResult
  step5_recompile: StepResult
  /** Aggregated CompiledMemoryDto returned by step 3 (for preview). */
  compiledDto?: CompiledMemoryDto
}

const initialSteps: AllSteps = {
  step1_first_compile: { status: 'idle' },
  step2_cache_hit: { status: 'idle' },
  step3_read: { status: 'idle' },
  step4_clear: { status: 'idle' },
  step5_recompile: { status: 'idle' },
}

function StatusIcon({ status }: { status: StepStatus }) {
  if (status === 'running') return <Loader2 className="h-3.5 w-3.5 animate-spin text-blue-500" />
  if (status === 'success') return <CheckCircle2 className="h-3.5 w-3.5 text-emerald-500" />
  if (status === 'error') return <XCircle className="h-3.5 w-3.5 text-red-500" />
  return <div className="h-3.5 w-3.5 rounded-full border border-black/15" />
}

function compileResultBadge(kind: 'compiled' | 'skipped') {
  const tone =
    kind === 'compiled'
      ? 'bg-emerald-500/[0.12] text-emerald-700 dark:text-emerald-300'
      : 'bg-zinc-400/[0.18] text-zinc-700 dark:text-zinc-300'
  return (
    <span className={`rounded-md px-1.5 py-0.5 font-mono text-[10px] uppercase ${tone}`}>
      {kind}
    </span>
  )
}

function CompileReportRow({ report }: { report: CompileReport }) {
  return (
    <div className="grid grid-cols-2 gap-x-4 gap-y-1 font-mono text-[11px] sm:grid-cols-4">
      <div className="flex items-center gap-1.5">
        <span className="text-muted-foreground">today:</span> {compileResultBadge(report.today)}
      </div>
      <div className="flex items-center gap-1.5">
        <span className="text-muted-foreground">week:</span> {compileResultBadge(report.week)}
      </div>
      <div className="flex items-center gap-1.5">
        <span className="text-muted-foreground">longterm:</span>{' '}
        {compileResultBadge(report.longterm)}
      </div>
      <div className="flex items-center gap-1.5">
        <span className="text-muted-foreground">facts:</span> {compileResultBadge(report.facts)}
      </div>
      <div className="col-span-2 sm:col-span-4 flex items-center gap-3 text-[10.5px] text-muted-foreground">
        <span>assembled: {report.assembled ? '✅' : '❌'}</span>
        <span>elapsed: {report.elapsed_ms} ms</span>
      </div>
    </div>
  )
}

export function MemoryDebugTab() {
  const [scope, setScope] = useState<Scope>('global')
  const [running, setRunning] = useState(false)
  const [steps, setSteps] = useState<AllSteps>(initialSteps)
  const [pinnedSnapshot, setPinnedSnapshot] = useState<PinnedItemDto[]>([])
  const [pinnedLoading, setPinnedLoading] = useState(false)

  const refreshPinnedSnapshot = async () => {
    setPinnedLoading(true)
    try {
      // 'both' merges global + (active project's) pins per PinnedStore.list_all_for_prompt.
      const items = await pinnedGet('both')
      setPinnedSnapshot(items)
    } catch (e) {
      console.warn('refreshPinnedSnapshot failed:', e)
    } finally {
      setPinnedLoading(false)
    }
  }

  useEffect(() => {
    void refreshPinnedSnapshot()
  }, [])

  const runStep = async <T,>(
    label: keyof AllSteps,
    fn: () => Promise<T>,
    serialize: (out: T) => string,
  ): Promise<T | null> => {
    const start = performance.now()
    setSteps((prev) => ({ ...prev, [label]: { status: 'running' } }))
    try {
      const out = await fn()
      const elapsed = Math.round(performance.now() - start)
      setSteps((prev) => ({
        ...prev,
        [label]: { status: 'success', elapsedMs: elapsed, payload: serialize(out) },
      }))
      return out
    } catch (e) {
      const elapsed = Math.round(performance.now() - start)
      setSteps((prev) => ({
        ...prev,
        [label]: { status: 'error', elapsedMs: elapsed, error: String(e) },
      }))
      return null
    }
  }

  const runFullSuite = async () => {
    setRunning(true)
    setSteps(initialSteps)
    try {
      // Step 1: first compile
      await runStep('step1_first_compile', () => memoryCompileNow(scope), (r) =>
        JSON.stringify(r, null, 2),
      )

      // Step 2: cache-hit verification (immediate re-run; expect all 'skipped')
      await runStep('step2_cache_hit', () => memoryCompileNow(scope), (r) =>
        JSON.stringify(r, null, 2),
      )

      // Step 3: read compiled memory
      const dto = await runStep(
        'step3_read',
        () => memoryCompiledRead(scope),
        (m) =>
          JSON.stringify(
            {
              memory_md_chars: m.memory_md.length,
              today: { chars: m.today.chars, last_compiled_at: m.today.last_compiled_at },
              week: { chars: m.week.chars, last_compiled_at: m.week.last_compiled_at },
              longterm: {
                chars: m.longterm.chars,
                last_compiled_at: m.longterm.last_compiled_at,
              },
              facts: { chars: m.facts.chars, last_compiled_at: m.facts.last_compiled_at },
            },
            null,
            2,
          ),
      )
      if (dto) {
        setSteps((prev) => ({ ...prev, compiledDto: dto }))
      }

      // Step 4: clear
      await runStep('step4_clear', () => memoryCompiledClear(scope), () => '✅ cleared')

      // Step 5: recompile after clear (expect 'compiled' on non-empty inputs)
      await runStep('step5_recompile', () => memoryCompileNow(scope), (r) =>
        JSON.stringify(r, null, 2),
      )
    } finally {
      setRunning(false)
    }
  }

  return (
    <SettingsSurface className="px-5 py-4">
      <div className="mb-3 flex items-center gap-2.5">
        <div className="flex size-7 items-center justify-center rounded-lg bg-blue-500/[0.1]">
          <Activity className="h-3.5 w-3.5 text-blue-600" />
        </div>
        <div className="text-[10.5px] font-semibold uppercase tracking-widest text-black/30">
          Phase 8B 编译流水线 — 一键验证
        </div>
      </div>
      <p className="mb-4 text-[11.5px] text-muted-foreground">
        触发 5 步连测：
        <code className="mx-1 rounded bg-black/5 px-1 py-0.5 text-[10.5px]">compile_now</code> ×2
        (验证 fingerprint cache) →
        <code className="mx-1 rounded bg-black/5 px-1 py-0.5 text-[10.5px]">compiled_read</code> →
        <code className="mx-1 rounded bg-black/5 px-1 py-0.5 text-[10.5px]">compiled_clear</code> →
        <code className="mx-1 rounded bg-black/5 px-1 py-0.5 text-[10.5px]">compile_now</code>
        (验证 clear 后重编)
      </p>

      <div className="mb-4 flex items-center gap-3">
        <label className="text-[12px] font-medium text-foreground/70 shrink-0">Scope</label>
        <select
          value={scope}
          onChange={(e) => setScope(e.target.value as Scope)}
          disabled={running}
          className="h-8 rounded-xl border border-black/[0.09] bg-black/[0.025] px-3 text-[12px] outline-none focus:border-jade/40 focus:ring-[3px] focus:ring-jade/15 disabled:opacity-50"
        >
          <option value="global">global</option>
          <option value="project">project</option>
          <option value="current">current</option>
          <option value="all">all</option>
        </select>
        <button
          onClick={runFullSuite}
          disabled={running}
          className="ml-auto flex items-center gap-1.5 rounded-xl bg-blue-600 px-4 py-1.5 text-[12px] font-medium text-white shadow-sm transition-all hover:bg-blue-700 disabled:opacity-50"
        >
          {running ? (
            <>
              <Loader2 className="h-3.5 w-3.5 animate-spin" /> 运行中…
            </>
          ) : (
            <>
              <Play className="h-3.5 w-3.5" /> 一键全套测试
            </>
          )}
        </button>
      </div>

      <div className="flex flex-col gap-3">
        <DebugStep
          n={1}
          title="memory_compile_now (首次)"
          hint="预期: today/week/facts 多为 'compiled'（如有 session_summaries），longterm 通常 'skipped'（依赖 week.md）"
          result={steps.step1_first_compile}
          render={(payload) => {
            try {
              const r = JSON.parse(payload) as CompileReport
              return <CompileReportRow report={r} />
            } catch {
              return <pre className="text-[10.5px]">{payload}</pre>
            }
          }}
        />
        <DebugStep
          n={2}
          title="memory_compile_now (缓存命中验证)"
          hint="预期: 全部 'skipped'，elapsed_ms < 50（fingerprint 命中跳过 LLM）"
          result={steps.step2_cache_hit}
          render={(payload) => {
            try {
              const r = JSON.parse(payload) as CompileReport
              const allSkipped =
                r.today === 'skipped' &&
                r.week === 'skipped' &&
                r.longterm === 'skipped' &&
                r.facts === 'skipped'
              return (
                <div className="flex flex-col gap-1">
                  <CompileReportRow report={r} />
                  <div
                    className={`text-[10.5px] ${
                      allSkipped ? 'text-emerald-600' : 'text-amber-600'
                    }`}
                  >
                    {allSkipped
                      ? '✅ Fingerprint cache 工作正常（全部 skipped）'
                      : '⚠️ 期待全 skipped，但有 compiled 项 — 可能是 session_summaries 在 step 1 之后又变了'}
                  </div>
                </div>
              )
            } catch {
              return <pre className="text-[10.5px]">{payload}</pre>
            }
          }}
        />
        <DebugStep
          n={3}
          title="memory_compiled_read"
          hint="读 memory.md + 4 个 section 的 metadata + 文件 mtime"
          result={steps.step3_read}
          render={(payload) => <pre className="text-[10.5px] leading-relaxed">{payload}</pre>}
        />
        <DebugStep
          n={4}
          title="memory_compiled_clear"
          hint="清空 5 个 .md + 删 4 个 .fingerprint sidecar"
          result={steps.step4_clear}
          render={(payload) => <div className="text-[11px]">{payload}</div>}
        />
        <DebugStep
          n={5}
          title="memory_compile_now (clear 后重编)"
          hint="预期: 至少 today/facts 是 'compiled'（fingerprint 已删，重新算）"
          result={steps.step5_recompile}
          render={(payload) => {
            try {
              const r = JSON.parse(payload) as CompileReport
              return <CompileReportRow report={r} />
            } catch {
              return <pre className="text-[10.5px]">{payload}</pre>
            }
          }}
        />
      </div>

      {steps.compiledDto && (
        <div className="mt-4 rounded-xl border border-black/[0.06] bg-black/[0.02] p-3">
          <div className="mb-2 flex items-center gap-2 text-[10.5px] font-semibold uppercase tracking-widest text-black/40">
            <RefreshCw className="h-3 w-3" /> memory.md 预览（前 800 字）
          </div>
          <pre className="whitespace-pre-wrap text-[10.5px] leading-relaxed text-foreground/85">
            {steps.compiledDto.memory_md.slice(0, 800) ||
              '(空文件 — session_summaries 表无数据；先与 agent 聊几轮触发 RollingSummarizer)'}
          </pre>
          {steps.compiledDto.memory_md.length > 800 && (
            <div className="mt-1 text-[10px] text-muted-foreground">
              ... 共 {steps.compiledDto.memory_md.length} 字
            </div>
          )}
        </div>
      )}

      {/* ── Pinned Memory snapshot (Phase 8A.9) ── */}
      <div className="mt-3 rounded-xl border border-amber-200/60 bg-amber-50/30 p-3 dark:bg-amber-950/15">
        <div className="mb-2 flex items-center justify-between">
          <div className="flex items-center gap-2 text-[10.5px] font-semibold uppercase tracking-widest text-amber-700 dark:text-amber-400">
            <Pin className="h-3 w-3" /> pinned 记忆快照（global + 项目）
          </div>
          <button
            onClick={() => void refreshPinnedSnapshot()}
            disabled={pinnedLoading}
            className="flex items-center gap-1 rounded-md px-2 py-0.5 text-[10px] text-amber-700 hover:bg-amber-100/60 disabled:opacity-50 dark:text-amber-400"
          >
            <RefreshCw className={`h-3 w-3 ${pinnedLoading ? 'animate-spin' : ''}`} /> 刷新
          </button>
        </div>
        {pinnedSnapshot.length === 0 ? (
          <div className="text-[11px] italic text-muted-foreground">
            (无 pinned 记忆 —— 先在「设置」标签的 PinnedMemoryEditor 里 pin 几条)
          </div>
        ) : (
          <ul className="space-y-1 text-[11px]">
            {pinnedSnapshot.map((p) => (
              <li key={p.id} className="flex items-start gap-2">
                <span
                  className={`mt-0.5 rounded px-1 py-0.5 font-mono text-[9px] uppercase ${
                    p.scope === 'global'
                      ? 'bg-amber-200/60 text-amber-900 dark:bg-amber-800/40 dark:text-amber-200'
                      : 'bg-blue-200/60 text-blue-900 dark:bg-blue-800/40 dark:text-blue-200'
                  }`}
                >
                  {p.scope === 'global' ? '全局' : '项目'}
                </span>
                <span className="flex-1 break-words text-foreground/85">{p.content}</span>
              </li>
            ))}
          </ul>
        )}
        <div className="mt-2 text-[10px] text-amber-800/80 dark:text-amber-300/70">
          ⚠️ pinned.md 与 memory.md 是<strong>两个独立文件</strong>。
          <code className="mx-0.5 rounded bg-amber-100/60 px-1 py-0.5 text-[9.5px]">
            memory_compiled_read
          </code>
          <strong>只返回 memory.md</strong>，不含 pinned。两者都被
          <code className="mx-0.5 rounded bg-amber-100/60 px-1 py-0.5 text-[9.5px]">
            build_memory_injection()
          </code>
          注入 system prompt（8A.11 落地）—— 想验证 pinned 真的生效，最快办法是
          <strong>开 chat 问 agent「你知道我是谁吗？」</strong>，agent 应该能引用 pinned
          内容回答。
        </div>
      </div>

      <div className="mt-3 rounded-xl border border-blue-200/60 bg-blue-50/40 p-3 text-[10.5px] text-blue-900 dark:bg-blue-950/20 dark:text-blue-200">
        💡 <strong>Tip</strong>: memory.md 全是「（暂无）」是<strong>正常</strong>初始状态 ——
        说明 session_summaries 表为空。先开 chat 跟 agent 聊几轮，触发 RollingSummarizer
        写入 summary，然后再回来跑这个测试就能看到真实编译结果。
      </div>
    </SettingsSurface>
  )
}

interface DebugStepProps {
  n: number
  title: string
  hint: string
  result: StepResult
  render: (payload: string) => React.ReactNode
}

function DebugStep({ n, title, hint, result, render }: DebugStepProps) {
  return (
    <div className="rounded-xl border border-black/[0.06] bg-white/40 p-3 dark:bg-black/20">
      <div className="mb-1.5 flex items-center gap-2">
        <StatusIcon status={result.status} />
        <span className="font-mono text-[10.5px] font-semibold text-black/40">{n}</span>
        <span className="text-[12px] font-medium">{title}</span>
        {result.elapsedMs != null && (
          <span className="ml-auto font-mono text-[10.5px] text-muted-foreground">
            {result.elapsedMs} ms
          </span>
        )}
      </div>
      <div className="mb-2 ml-5 text-[10.5px] text-muted-foreground">{hint}</div>
      {result.status === 'success' && result.payload && (
        <div className="ml-5 rounded-lg bg-black/[0.025] p-2 dark:bg-white/[0.04]">
          {render(result.payload)}
        </div>
      )}
      {result.status === 'error' && result.error && (
        <div className="ml-5 rounded-lg bg-red-50 p-2 text-[10.5px] text-red-700 dark:bg-red-950/30 dark:text-red-300">
          ❌ {result.error}
        </div>
      )}
    </div>
  )
}
