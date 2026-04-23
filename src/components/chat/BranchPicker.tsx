/**
 * BranchPicker — composer footer 的"当前 git 分支"按钮 + 下拉切换器。
 *
 * 视觉对应 image-1：搜索框 + 分支列表（current 打勾 + uncommitted 计数）+
 * 创建并检出新分支。完全独立组件，对外暴露：
 *   - `cwd`     当前项目工作目录；空字符串时整个组件渲染为禁用占位
 *   - `currentBranch`  父组件已知的当前分支（来自 `branchLabel` 上游）
 *   - `onChanged`      checkout / create 成功后回调（父组件应刷新 branchLabel）
 *
 * 数据来源：`@/modules/git/api` 的 `gitBranches` / `gitCheckoutBranch`
 * /`gitCreateBranch` / `gitStatus`。打开下拉时按需拉取，不在挂载时预热，
 * 避免在每个会话切换时无谓地跑 `git`。
 */

import * as React from 'react'
import { Check, GitBranch, Loader2, Plus, Search, Sparkles } from 'lucide-react'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { cn } from '@/lib/utils'
import {
  gitBranches,
  gitCheckoutBranch,
  gitCreateBranch,
  gitInitRepo,
  gitStatus,
  parseBranchList,
  type BranchListItem,
} from '@/modules/git/api'

type Props = {
  cwd: string | undefined
  currentBranch: string
  onChanged?: (newBranch: string) => void
  /** Tri-state Git presence:
   *   - `true`  → fully active picker
   *   - `false` → trigger shows "无 Git 仓库" and refuses to open;
   *               click prompts the parent to call `onInitRepo`
   *   - `null`  → still probing; render optimistic active state */
  isGitRepo?: boolean | null
  /** Called when the user clicks the disabled trigger and accepts the
   *  "init now" prompt.  Implementer should run `gitInitRepo(cwd)` and
   *  then re-probe `gitIsRepo` so this picker re-enables. */
  onInitRepo?: () => void
  className?: string
}

type LoadState =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'ready'; branches: BranchListItem[]; uncommittedCount: number }
  | { kind: 'error'; message: string }

function uncommittedFromStatus(raw: string | null): number {
  if (!raw) return 0
  // `git status --short --branch` first line is `## branch...origin/...`,
  // each subsequent non-empty line is one changed/untracked file.
  return raw
    .split('\n')
    .slice(1)
    .filter((line) => line.trim().length > 0).length
}

export function BranchPicker({
  cwd,
  currentBranch,
  onChanged,
  isGitRepo = null,
  onInitRepo,
  className,
}: Props) {
  const [open, setOpen] = React.useState(false)
  const [state, setState] = React.useState<LoadState>({ kind: 'idle' })
  const [query, setQuery] = React.useState('')
  const [creating, setCreating] = React.useState(false)
  const [busyBranch, setBusyBranch] = React.useState<string | null>(null)
  const [createName, setCreateName] = React.useState('')
  // 二次确认弹层：脏工作树切分支会让 git 自己 abort 或者更糟（被
  // 静默吞掉未提交改动），所以前端在 dirty 时拦一次让用户确认。
  // `null` = 不在确认态；`{ name }` = 等待用户对该分支的 confirm/cancel。
  const [pendingCheckout, setPendingCheckout] = React.useState<string | null>(null)

  const noCwd = !cwd || cwd.trim() === ''
  // `isGitRepo === false` 是已知"非 git 目录"；`null` 仍在探测期间，
  // 走乐观可用，避免每次切换项目时短暂闪现 disabled。
  const noRepo = isGitRepo === false
  // popover 在 noRepo 时不打开（用户的单击改为触发 init），但 trigger
  // 仍然 clickable —— 没有 cwd 时才真正 disabled。
  const popoverDisabled = noCwd || noRepo
  const triggerDisabled = noCwd
  const [initing, setIniting] = React.useState(false)

  const handleInit = React.useCallback(async () => {
    if (!cwd || initing) return
    setIniting(true)
    try {
      await gitInitRepo(cwd)
      onInitRepo?.()
    } catch (err) {
      // 失败时打开 popover 把错误显示到列表区域
      setOpen(true)
      setState({
        kind: 'error',
        message: err instanceof Error ? err.message : String(err),
      })
    } finally {
      setIniting(false)
    }
  }, [cwd, initing, onInitRepo])

  const refresh = React.useCallback(async () => {
    if (!cwd) return
    setState({ kind: 'loading' })
    try {
      const [branchesRaw, statusRaw] = await Promise.all([
        gitBranches(cwd),
        gitStatus(cwd).catch(() => null),
      ])
      setState({
        kind: 'ready',
        branches: parseBranchList(branchesRaw),
        uncommittedCount: uncommittedFromStatus(statusRaw),
      })
    } catch (err) {
      setState({
        kind: 'error',
        message: err instanceof Error ? err.message : String(err),
      })
    }
  }, [cwd])

  // Load on open; reset transient inputs on close so the next open feels fresh.
  React.useEffect(() => {
    if (open) {
      void refresh()
    } else {
      setQuery('')
      setCreating(false)
      setCreateName('')
      setBusyBranch(null)
      setPendingCheckout(null)
    }
  }, [open, refresh])

  const filtered = React.useMemo(() => {
    if (state.kind !== 'ready') return [] as BranchListItem[]
    const q = query.trim().toLowerCase()
    if (!q) return state.branches
    return state.branches.filter((b) => b.name.toLowerCase().includes(q))
  }, [state, query])

  const handleCheckout = async (name: string) => {
    if (!cwd || name === currentBranch) {
      setOpen(false)
      return
    }
    // 工作树有未提交改动 → 弹二次确认，由 confirmCheckout 真正执行。
    // 注意：当前分支自身的 row 仍然不会触发（上面的 early return），
    // dirty 检查只针对"切到别的分支"。
    if (state.kind === 'ready' && state.uncommittedCount > 0) {
      setPendingCheckout(name)
      return
    }
    await runCheckout(name)
  }

  const runCheckout = async (name: string) => {
    if (!cwd) return
    setPendingCheckout(null)
    setBusyBranch(name)
    try {
      await gitCheckoutBranch(cwd, name)
      onChanged?.(name)
      setOpen(false)
    } catch (err) {
      setState({
        kind: 'error',
        message: err instanceof Error ? err.message : String(err),
      })
    } finally {
      setBusyBranch(null)
    }
  }

  const handleCreate = async () => {
    const name = createName.trim()
    if (!cwd || !name) return
    setBusyBranch(name)
    try {
      await gitCreateBranch(cwd, name)
      onChanged?.(name)
      setOpen(false)
    } catch (err) {
      setState({
        kind: 'error',
        message: err instanceof Error ? err.message : String(err),
      })
    } finally {
      setBusyBranch(null)
    }
  }

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        // noRepo 状态下不打开 popover：单击转去 init；下面 trigger
        // 的 onClick 已经处理了。这里防止 Radix 自己打开（例如键盘
        // Enter）误触。
        if (popoverDisabled) return
        setOpen(next)
      }}
    >
      <PopoverTrigger asChild>
        <button
          type="button"
          disabled={triggerDisabled}
          onClick={
            noRepo
              ? (e) => {
                  // noRepo 时：拦截 popover 打开 → 直接走 init 流程
                  e.preventDefault()
                  e.stopPropagation()
                  void handleInit()
                }
              : undefined
          }
          title={noRepo ? '当前目录不是 Git 仓库 — 点击执行 git init' : undefined}
          className={cn(
            'flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[11px] transition-colors disabled:cursor-not-allowed disabled:opacity-60',
            noRepo
              ? 'text-black/35 hover:bg-amber-50 hover:text-amber-700'
              : 'text-black/40 hover:bg-black/[0.04] hover:text-black/70',
            className,
          )}
          aria-label={noRepo ? '初始化 Git 仓库' : '切换 git 分支'}
        >
          <GitBranch className="h-[11px] w-[11px]" />
          {noRepo ? (
            <span className="inline-flex items-center gap-1">
              {initing ? (
                <Loader2 className="h-[10px] w-[10px] animate-spin" />
              ) : (
                <Sparkles className="h-[10px] w-[10px]" />
              )}
              <span>无 Git 仓库 · 点击初始化</span>
            </span>
          ) : (
            <span className="max-w-[160px] truncate">
              {currentBranch || '—'}
            </span>
          )}
        </button>
      </PopoverTrigger>
      <PopoverContent
        align="center"
        sideOffset={12}
        collisionPadding={16}
        className={cn(
          'w-[260px] overflow-hidden rounded-2xl border border-black/[0.06] bg-white/95 p-0 text-[13px] backdrop-blur-2xl backdrop-saturate-150',
          // 多层阴影模拟"浮起"：近距离柔光 + 中距离环境光 + 远距离落影 + 0.5px hairline
          'shadow-[0_2px_4px_rgba(0,0,0,0.04),0_8px_20px_rgba(0,0,0,0.08),0_24px_56px_rgba(0,0,0,0.16),0_0_0_0.5px_rgba(0,0,0,0.04)]',
          // Radix open/close 动画：略带弹性的 scale + 从上方滑入
          'origin-[var(--radix-popover-content-transform-origin)] transition-all duration-200 ease-out',
          'data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 data-[state=open]:slide-in-from-top-1',
          'data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95',
        )}
      >
        {/* Dirty checkout 二次确认：覆盖在列表上方的小型确认条，
            CTA 用琥珀色突出，给"返回"作 escape hatch。Cancel 不
            关闭 popover，让用户可以先去看 /diff 再决定。 */}
        {pendingCheckout && state.kind === 'ready' && (
          <div className="border-b border-amber-200/70 bg-amber-50/70 px-3.5 py-3">
            <div className="text-[12px] leading-5 text-amber-900">
              工作区有 <span className="font-semibold">{state.uncommittedCount}</span> 个未提交的文件。切到{' '}
              <span className="font-mono text-[12px] text-amber-900">{pendingCheckout}</span>{' '}
              可能会覆盖或保留改动，git 视情况决定 —— 建议先提交或暂存。
            </div>
            <div className="mt-2 flex items-center justify-end gap-1.5">
              <button
                type="button"
                onClick={() => setPendingCheckout(null)}
                className="rounded-md px-2.5 py-1 text-[11.5px] text-black/55 outline-none transition-colors hover:bg-black/[0.05] focus-visible:bg-black/[0.05]"
              >
                返回
              </button>
              <button
                type="button"
                onClick={() => void runCheckout(pendingCheckout)}
                className="rounded-md bg-amber-600 px-2.5 py-1 text-[11.5px] font-medium text-white outline-none transition-opacity hover:opacity-90 focus-visible:opacity-90"
              >
                仍要切换
              </button>
            </div>
          </div>
        )}

        {/* Search */}
        <div className="flex items-center gap-2 px-3.5 pt-3 pb-2.5">
          <Search className="h-3.5 w-3.5 shrink-0 text-black/30" />
          <input
            type="text"
            autoFocus
            placeholder="搜索分支"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="flex-1 bg-transparent text-[11.5px] leading-6 text-black/85 outline-none placeholder:text-black/35"
          />
        </div>

        {/* List */}
        <div className="max-h-[280px] overflow-y-auto pb-1.5">
          {state.kind === 'loading' && (
            <div className="flex items-center justify-center gap-2 py-7 text-[13px] text-black/40">
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
              <span>加载中…</span>
            </div>
          )}
          {state.kind === 'error' && (
            <div className="px-3.5 py-3 text-[11.5px] leading-5 text-rose-600/80">
              {state.message}
            </div>
          )}
          {state.kind === 'ready' && (
            <>
              <div className="px-3.5 pb-1 pt-1 text-[11.5px] text-black/40">
                分支
              </div>
              {filtered.length === 0 && (
                <div className="px-3.5 py-5 text-center text-[13px] text-black/30">
                  无匹配分支
                </div>
              )}
              {filtered.map((b) => {
                const isCurrent =
                  b.isCurrent || b.name === currentBranch
                const isBusy = busyBranch === b.name
                return (
                  <button
                    key={b.name}
                    type="button"
                    disabled={isBusy}
                    onClick={() => handleCheckout(b.name)}
                    className={cn(
                      // outline-none + focus-visible bg：见 GitActionsPicker
                      // 同位置注释 —— 抑制 WebKit `:focus-visible` 默认蓝
                      // 描边被 PopoverContent overflow-hidden 切成两条横
                      // 线的渲染异常。
                      'flex w-full items-start gap-2.5 px-3.5 py-1.5 text-left outline-none transition-colors hover:bg-black/[0.035] focus-visible:bg-black/[0.05]',
                      isBusy && 'opacity-60',
                    )}
                  >
                    <GitBranch
                      className={cn(
                        'mt-[3px] h-[14px] w-[14px] shrink-0',
                        isCurrent ? 'text-black/70' : 'text-black/45',
                      )}
                      strokeWidth={1.75}
                    />
                    <div className="min-w-0 flex-1">
                      <span className="block truncate text-[13px] leading-6 text-black/82">
                        {b.name}
                      </span>
                      {isCurrent && state.uncommittedCount > 0 && (
                        <div className="text-[11.5px] leading-5 text-black/40">
                          未提交的更改：{state.uncommittedCount} 个文件
                        </div>
                      )}
                    </div>
                    {isCurrent && !isBusy && (
                      <Check
                        className="mt-[5px] h-[13px] w-[13px] shrink-0 text-black/65"
                        strokeWidth={2}
                      />
                    )}
                    {isBusy && (
                      <Loader2 className="mt-[5px] h-3.5 w-3.5 shrink-0 animate-spin text-black/40" />
                    )}
                  </button>
                )
              })}
            </>
          )}
        </div>

        {/* Create new branch */}
        <div className="border-t border-black/[0.06]">
          {!creating ? (
            <button
              type="button"
              onClick={() => setCreating(true)}
              disabled={state.kind !== 'ready'}
              className="flex w-full items-center gap-2.5 px-3.5 py-2.5 text-left text-[11.5px] leading-6 text-black/68 outline-none transition-colors hover:bg-black/[0.035] focus-visible:bg-black/[0.05] disabled:cursor-not-allowed disabled:opacity-60"
            >
              <Plus className="h-3.5 w-3.5 text-black/45" strokeWidth={2} />
              创建并检出新分支…
            </button>
          ) : (
            <div className="flex items-center gap-2 px-3.5 py-2.5">
              <input
                autoFocus
                type="text"
                placeholder="新分支名"
                value={createName}
                onChange={(e) => setCreateName(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    e.preventDefault()
                    void handleCreate()
                  } else if (e.key === 'Escape') {
                    setCreating(false)
                    setCreateName('')
                  }
                }}
                className="flex-1 rounded-lg border border-black/10 bg-white/70 px-2.5 py-1.5 text-[13px] outline-none focus:border-black/30"
              />
              <button
                type="button"
                onClick={() => void handleCreate()}
                disabled={!createName.trim() || busyBranch !== null}
                className="rounded-lg bg-black/80 px-3 py-1.5 text-[12px] font-medium text-white transition-opacity hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-50"
              >
                创建
              </button>
            </div>
          )}
        </div>
      </PopoverContent>
    </Popover>
  )
}
