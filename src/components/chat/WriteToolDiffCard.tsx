/**
 * WriteToolDiffCard — file_write 工具调用展开后的 diff 卡片。
 *
 * 像 Cursor / Codex 一样以 **hunk 视图** 呈现差异：只渲染变更行 +
 * 上下若干行 context；中间的大段未改动代码用 "── N 行未变 ──"
 * 折叠条折叠，点击可展开。这样：
 *   - 大文件改三行 → 卡片只占 ~10 行高度，省 token / 省眼睛
 *   - 真正想看完整文件 → 顶部 "显示全文" toggle，回退到旧行为
 *
 * 输入：
 *   - `path`        目标文件绝对/相对路径
 *   - `newContent`  新写入内容（来自 toolArgs.content）
 *   - `oldContent`  旧内容（可选；缺失时退化为"全文新增"渲染）
 *   - `contextLines`  hunk 上下文宽度，默认 3（git diff 默认）
 *
 * 行内 diff 算法用 jsdiff 的 `structuredPatch`（Myers），跟
 * `git diff` / Cursor 是同一个家族，正确处理插入/删除导致的行
 * 偏移，不再有"插一行就全文飘红"的旧 bug。
 */

import * as React from 'react'
import { structuredPatch, type StructuredPatchHunk } from 'diff'
import { Check, ChevronsDownUp, ChevronsUpDown, Copy } from 'lucide-react'
import { cn } from '@/lib/utils'

type DiffLine =
  | { kind: 'add'; text: string; newNo: number }
  | { kind: 'del'; text: string; oldNo: number }
  | { kind: 'ctx'; text: string; oldNo: number; newNo: number }

/** 一段连续的 hunk 渲染数据 + 它跟下一个 hunk 之间被折叠的行数。 */
interface RenderHunk {
  /** 原始 hunk 的元信息，用于展开/折叠语义。 */
  hunk: StructuredPatchHunk
  /** 已经按 +/-/' ' 拆好的展示行；每行带原始行号（从 hunk 起算）。 */
  lines: DiffLine[]
}

/** 把 jsdiff 的 hunks 拍成本组件的 RenderHunk 列表。 */
function buildRenderHunks(oldText: string, newText: string, context: number): RenderHunk[] {
  // structuredPatch 接受任意名字，这里用占位 "a" / "b"，反正不展示
  const patch = structuredPatch('a', 'b', oldText, newText, '', '', { context })
  return patch.hunks.map((hunk) => {
    const lines: DiffLine[] = []
    let oldNo = hunk.oldStart
    let newNo = hunk.newStart
    for (const raw of hunk.lines) {
      const sign = raw.charAt(0)
      const text = raw.slice(1)
      if (sign === '+') {
        lines.push({ kind: 'add', text, newNo })
        newNo += 1
      } else if (sign === '-') {
        lines.push({ kind: 'del', text, oldNo })
        oldNo += 1
      } else {
        // 既包含 ' '（context）也包含 '\\'（"\ No newline at end of file"），
        // 后者直接当 context 处理即可——视觉上无差别。
        lines.push({ kind: 'ctx', text, oldNo, newNo })
        oldNo += 1
        newNo += 1
      }
    }
    return { hunk, lines }
  })
}

/** 计算两个相邻 hunk 之间被折叠的行数。 */
function gapLineCount(prev: StructuredPatchHunk, next: StructuredPatchHunk): number {
  const prevEnd = prev.oldStart + prev.oldLines
  return Math.max(0, next.oldStart - prevEnd)
}

/** 简易"全文新增"渲染：oldContent 为空（首次创建文件）时跳过 LCS，
 *  把所有行直接当 add 渲染，省一遍 diff 的 CPU。 */
function buildAllAddedLines(newText: string): DiffLine[] {
  if (newText === '') return []
  return newText.split('\n').map((text, i) => ({
    kind: 'add' as const,
    text,
    newNo: i + 1,
  }))
}

function densityCells(
  totals: { add: number; del: number },
  cellCount = 12,
): Array<'add' | 'del' | 'mix' | 'none'> {
  // 没法算精确密度时给一个粗糙汇总：纯加 → 全 add，纯删 → 全 del，
  // 混合 → 全 mix。详细密度的视觉信息其实由"几个 hunk + 各自体量"承
  // 担，密度条退化为一个总览徽章。
  const total = totals.add + totals.del
  if (total === 0) return Array(cellCount).fill('none')
  const addRatio = totals.add / total
  return Array.from({ length: cellCount }, (_, i) => {
    if (totals.add > 0 && totals.del > 0) {
      return i / cellCount < addRatio ? 'add' : 'del'
    }
    return totals.add > 0 ? 'add' : 'del'
  })
}

export function WriteToolDiffCard({
  path,
  newContent,
  oldContent = '',
  contextLines = 3,
  className,
}: {
  path: string
  newContent: string
  oldContent?: string
  /** Lines of unchanged context above/below each change.  Defaults to
   *  3 (matches `git diff` default).  Pass a larger value if you'd
   *  rather see more surrounding code. */
  contextLines?: number
  className?: string
}) {
  // showFull = true → 跳过 hunking，直接展开整文件（兼容旧行为，
  // 用户偶尔需要做大局对比时启用）
  const [showFull, setShowFull] = React.useState(false)
  // 用户主动展开过的折叠段，用 hunk-index pair 当 key
  const [expandedGaps, setExpandedGaps] = React.useState<Set<string>>(() => new Set())

  // hunked 视图所需数据
  const isFreshFile = oldContent === ''
  const renderHunks = React.useMemo(
    () => (isFreshFile ? [] : buildRenderHunks(oldContent, newContent, contextLines)),
    [isFreshFile, oldContent, newContent, contextLines],
  )
  const fullLines = React.useMemo(
    () => (isFreshFile ? buildAllAddedLines(newContent) : null),
    [isFreshFile, newContent],
  )

  // 行数统计：fresh 文件 → 全是 add；否则从 hunks 累加
  const totals = React.useMemo(() => {
    if (isFreshFile) {
      return { add: fullLines?.length ?? 0, del: 0 }
    }
    return renderHunks.reduce(
      (acc, h) => {
        for (const l of h.lines) {
          if (l.kind === 'add') acc.add += 1
          else if (l.kind === 'del') acc.del += 1
        }
        return acc
      },
      { add: 0, del: 0 },
    )
  }, [isFreshFile, fullLines, renderHunks])

  const cells = React.useMemo(() => densityCells(totals), [totals])

  const [copied, setCopied] = React.useState(false)
  const onCopy = React.useCallback(async () => {
    try {
      await navigator.clipboard.writeText(path)
      setCopied(true)
      window.setTimeout(() => setCopied(false), 1400)
    } catch {
      /* clipboard 不可用时静默 */
    }
  }, [path])

  // 行号宽度按最大行号决定，保证对齐
  const maxNo = React.useMemo(() => {
    if (isFreshFile) return Math.max(1, fullLines?.length ?? 1)
    let m = 1
    for (const h of renderHunks) {
      m = Math.max(m, h.hunk.oldStart + h.hunk.oldLines, h.hunk.newStart + h.hunk.newLines)
    }
    return m
  }, [isFreshFile, fullLines, renderHunks])
  const noWidthCh = String(maxNo).length

  // 渲染：hunk 视图 vs 全文视图 vs 全新文件
  const body = (() => {
    if (isFreshFile) {
      return <LinesPre lines={fullLines ?? []} noWidthCh={noWidthCh} />
    }
    if (showFull) {
      // 全文模式：直接对老/新文本走一次 0-context patch，把所有 hunk
      // 串起来。等价于"无折叠"展开。
      const fullHunks = buildRenderHunks(oldContent, newContent, Number.MAX_SAFE_INTEGER)
      const allLines = fullHunks.flatMap((h) => h.lines)
      return <LinesPre lines={allLines} noWidthCh={noWidthCh} />
    }
    if (renderHunks.length === 0) {
      return (
        <div className="px-3 py-4 text-center text-[12px] text-muted-foreground/70">
          无文本差异（写入内容与旧版本完全一致）
        </div>
      )
    }
    return (
      <div className="font-mono text-[11.5px] leading-[1.55]">
        {renderHunks.map((h, i) => {
          const next = renderHunks[i + 1]
          const gap = next ? gapLineCount(h.hunk, next.hunk) : 0
          const gapKey = `gap-${i}`
          const gapExpanded = expandedGaps.has(gapKey)
          return (
            <React.Fragment key={`hunk-${i}`}>
              {/* hunk 头：@@ -oldStart,oldLines +newStart,newLines @@ */}
              <div className="border-y border-border/30 bg-muted/30 px-3 py-1 font-mono text-[10.5px] text-muted-foreground/70">
                @@ -{h.hunk.oldStart},{h.hunk.oldLines} +{h.hunk.newStart},{h.hunk.newLines} @@
              </div>
              <LinesPre lines={h.lines} noWidthCh={noWidthCh} />
              {next && gap > 0 && (
                <button
                  type="button"
                  onClick={() =>
                    setExpandedGaps((prev) => {
                      const nextSet = new Set(prev)
                      if (gapExpanded) nextSet.delete(gapKey)
                      else nextSet.add(gapKey)
                      return nextSet
                    })
                  }
                  className="flex w-full items-center justify-center gap-1.5 border-y border-border/20 bg-muted/15 px-3 py-1 text-[11px] text-muted-foreground/65 outline-none transition-colors hover:bg-muted/40 hover:text-foreground/80 focus-visible:bg-muted/40"
                >
                  {gapExpanded ? (
                    <ChevronsDownUp className="h-3 w-3" />
                  ) : (
                    <ChevronsUpDown className="h-3 w-3" />
                  )}
                  {gapExpanded ? '收起未变内容' : `展开 ${gap} 行未变内容`}
                </button>
              )}
              {next && gapExpanded && gap > 0 && (
                <GapExpansion
                  oldStart={h.hunk.oldStart + h.hunk.oldLines}
                  newStart={h.hunk.newStart + h.hunk.newLines}
                  count={gap}
                  source={oldContent}
                  noWidthCh={noWidthCh}
                />
              )}
            </React.Fragment>
          )
        })}
      </div>
    )
  })()

  // 完全干净的 0/0 也给一个非破坏性提示
  const isNoOp = !isFreshFile && totals.add === 0 && totals.del === 0

  return (
    <div
      className={cn(
        'overflow-hidden rounded-lg border border-border/60 bg-muted/20',
        className,
      )}
    >
      {/* Header：文件路径 + +N/-N 徽章 + Copy path */}
      <div className="flex items-start justify-between gap-3 border-b border-border/50 bg-muted/30 px-3 py-2">
        <div className="min-w-0 flex-1">
          <div className="truncate font-mono text-[11.5px] leading-[1.4] text-muted-foreground">
            {path}
          </div>
          <div className="mt-1.5 flex items-center gap-1.5 text-[10.5px] font-medium leading-[1.4]">
            <span className="rounded-full bg-emerald-100 px-1.5 py-0.5 text-emerald-700">
              +{totals.add}
            </span>
            <span className="rounded-full bg-rose-100 px-1.5 py-0.5 text-rose-700">
              -{totals.del}
            </span>
            {!isFreshFile && !isNoOp && (
              <span className="rounded-full bg-muted px-1.5 py-0.5 text-muted-foreground/75">
                {renderHunks.length} 个 hunk
              </span>
            )}
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          {!isFreshFile && (
            <button
              type="button"
              onClick={() => setShowFull((v) => !v)}
              className="rounded-md border border-border/60 bg-background px-2 py-1 text-[11px] text-muted-foreground outline-none transition-colors hover:bg-muted hover:text-foreground focus-visible:bg-muted"
              aria-pressed={showFull}
              title={showFull ? '回到 hunk 视图（仅显示变更附近）' : '展开整个文件'}
            >
              {showFull ? '隐藏未变内容' : '显示全文'}
            </button>
          )}
          <button
            type="button"
            onClick={onCopy}
            className="inline-flex items-center gap-1 rounded-md border border-border/60 bg-background px-2 py-1 text-[11px] text-muted-foreground outline-none transition-colors hover:bg-muted hover:text-foreground focus-visible:bg-muted"
            aria-label="复制文件路径"
          >
            {copied ? (
              <>
                <Check className="h-3 w-3" />
                Copied
              </>
            ) : (
              <>
                <Copy className="h-3 w-3" />
                Copy path
              </>
            )}
          </button>
        </div>
      </div>

      {/* 密度条 */}
      <div className="flex items-center gap-1.5 border-b border-border/40 bg-muted/15 px-3 py-1.5">
        <span className="font-mono text-[10.5px] text-muted-foreground/70">
          {totals.add + totals.del}
        </span>
        <div className="flex items-center gap-[3px]">
          {cells.map((c, i) => (
            <span
              key={i}
              className={cn(
                'h-2 w-3 rounded-[2px]',
                c === 'add' && 'bg-emerald-300/70',
                c === 'del' && 'bg-rose-300/70',
                c === 'mix' && 'bg-amber-300/70',
                c === 'none' && 'bg-muted-foreground/15',
              )}
            />
          ))}
        </div>
      </div>

      {/* Diff body：hunked / 全文 / 全新文件 */}
      <div className="max-h-[420px] overflow-auto bg-background/40">{body}</div>
    </div>
  )
}

/** 渲染一段 DiffLine 为左行号 + +/- + 内容的 `<pre>`。 */
function LinesPre({ lines, noWidthCh }: { lines: DiffLine[]; noWidthCh: number }) {
  return (
    <pre className="m-0 font-mono text-[11.5px] leading-[1.55]">
      {lines.map((line, idx) => {
        const bg =
          line.kind === 'add'
            ? 'bg-emerald-100/60'
            : line.kind === 'del'
              ? 'bg-rose-100/60'
              : ''
        const sign = line.kind === 'add' ? '+' : line.kind === 'del' ? '-' : ' '
        const signColor =
          line.kind === 'add'
            ? 'text-emerald-700'
            : line.kind === 'del'
              ? 'text-rose-700'
              : 'text-muted-foreground/50'
        const no = line.kind === 'del' ? line.oldNo : line.newNo
        return (
          <div key={idx} className={cn('flex items-baseline gap-2 px-3 py-[1px]', bg)}>
            <span
              className="shrink-0 select-none text-right font-mono text-[10.5px] text-muted-foreground/60"
              style={{ width: `${noWidthCh}ch` }}
            >
              {no}
            </span>
            <span className={cn('shrink-0 select-none font-semibold', signColor)}>{sign}</span>
            <span className="whitespace-pre-wrap break-words text-foreground/90">
              {line.text || '\u00A0'}
            </span>
          </div>
        )
      })}
    </pre>
  )
}

/** "展开 N 行未变内容" 后渲染的那段 context —— 直接从原文切出来，
 *  按"未变"行渲染。行号同步前向 hunk 末尾。 */
function GapExpansion({
  oldStart,
  newStart,
  count,
  source,
  noWidthCh,
}: {
  oldStart: number
  newStart: number
  count: number
  source: string
  noWidthCh: number
}) {
  const slice = React.useMemo(() => {
    const all = source.split('\n')
    return all.slice(oldStart - 1, oldStart - 1 + count)
  }, [source, oldStart, count])
  const lines: DiffLine[] = slice.map((text, i) => ({
    kind: 'ctx' as const,
    text,
    oldNo: oldStart + i,
    newNo: newStart + i,
  }))
  return <LinesPre lines={lines} noWidthCh={noWidthCh} />
}
