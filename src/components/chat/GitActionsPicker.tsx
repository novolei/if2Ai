/**
 * GitActionsPicker — 顶部状态栏上的 "Git 操作" 下拉。
 *
 * 视觉与 BranchPicker 完全一致（圆角、阴影、半透明背景、动画），
 * 只是内容是动作菜单而不是分支列表。打开后呈现 4 个动作：
 *   - 提交：弹出输入框收集 commit message → `gitCommit`
 *   - 推送：调用一次性的 `gitPush`（待 backend；当前先发提示）
 *   - 创建拉取请求：收集 title + body → `gitCommitPushPr`
 *   - 创建分支：复用 `gitCreateBranch`（输入新分支名）
 *
 * 触发器是 image-3 里那个圆角胶囊按钮：commit 图标 + "提交" 文案 +
 * chevron-down，点击后弹出。`cwd` 缺失（无 active project）时整个
 * 组件渲染为禁用占位。
 */

import * as React from 'react'
import {
  AlertTriangle,
  Check,
  ChevronDown,
  Copy,
  ExternalLink,
  GitBranch,
  GitCommitHorizontal,
  GitPullRequestArrow,
  Loader2,
  PanelTopOpen,
  Sparkles,
  UploadCloud,
  X,
} from 'lucide-react'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { cn } from '@/lib/utils'
import {
  ghAvailable,
  gitCommit,
  gitCommitPushPr,
  gitCreateBranch,
  gitCreateWorktreeProject,
  gitInitRepo,
  type CreatedWorktreeProject,
} from '@/modules/git/api'

type Props = {
  cwd: string | undefined
  /** Tri-state Git presence — same semantics as `BranchPicker.isGitRepo`. */
  isGitRepo?: boolean | null
  /** Fired after this picker successfully runs `git init`; parent should
   *  re-probe `gitIsRepo` so both pickers re-enable. */
  onGitRepoChanged?: () => void
  /** Notify parent that the current branch may have changed (after
   *  create-branch flow).  Hooked to the same `setBranchLabel` used by
   *  `BranchPicker`. */
  onBranchChange?: (newBranch: string) => void
  /** Optional callback to open the Git Workbench drawer (status / diff
   *  / branches view).  When provided, the menu surfaces a "查看 Git
   *  状态" entry; when omitted that entry is hidden so the picker stays
   *  usable in environments without a workbench (settings page, etc). */
  onOpenWorkbench?: () => void
  /** Fired after `gitCreateWorktreeProject` succeeds — caller is
   *  expected to refresh the project list / sidebar and (optionally)
   *  switch the active session into the new project.  When omitted the
   *  "在新 worktree 里继续" action stays usable but the user has to
   *  refresh manually. */
  onWorktreeProjectCreated?: (project: CreatedWorktreeProject) => void
  className?: string
}

type Mode =
  | { kind: 'menu' }
  | { kind: 'commit' }
  | { kind: 'createBranch' }
  | { kind: 'createWorktree' }
  | { kind: 'pr' }
  | { kind: 'busy'; label: string }
  | { kind: 'success'; message: string }
  | { kind: 'error'; message: string }
  /**
   * "gh 不可用" 兜底视图：拿到用户填的 PR title/body 后，不发 IPC，
   * 而是把可执行的 `gh pr create` 命令 + body 一起展示，让用户复制后
   * 自己粘贴到终端。配合 `installGuidance: true` 时还会显示安装链接。
   */
  | { kind: 'prDraft'; title: string; body: string }

/** Prefix that the slash backend uses to mark PR / Issue draft fallback
 *  output (matches `pr_cmd.rs::handle` & `issue_cmd.rs::handle`).  Used
 *  upstream by `SlashResultCard` for callout styling — kept here so we
 *  can mirror the same wording in this picker's draft mode. */
const GH_INSTALL_URL = 'https://cli.github.com/'

export function GitActionsPicker({
  cwd,
  isGitRepo = null,
  onGitRepoChanged,
  onBranchChange,
  onOpenWorkbench,
  onWorktreeProjectCreated,
  className,
}: Props) {
  const [open, setOpen] = React.useState(false)
  const [mode, setMode] = React.useState<Mode>({ kind: 'menu' })
  const [commitMessage, setCommitMessage] = React.useState('')
  const [branchName, setBranchName] = React.useState('')
  const [worktreeBranch, setWorktreeBranch] = React.useState('')
  const [worktreeTarget, setWorktreeTarget] = React.useState('')
  const [prTitle, setPrTitle] = React.useState('')
  const [prBody, setPrBody] = React.useState('')
  // `gh` 探测结果：null = 还在探测；true/false = 已知。
  // 第一次 popover 打开时触发探测；探测期间 UI 走"假定可用"的乐观
  // 路径，请求失败再回到草稿模式（由 runCreatePr 的 catch 兜底）。
  const [ghOk, setGhOk] = React.useState<boolean | null>(null)

  const noCwd = !cwd || cwd.trim() === ''
  const noRepo = isGitRepo === false
  // 不像 BranchPicker，这里 noRepo 仍然允许打开 popover —— 因为弹出
  // 的是只含"在此目录初始化 Git…" 一项的小菜单，让用户能直接 init。
  const disabled = noCwd

  React.useEffect(() => {
    if (!open) {
      // 重置所有瞬态状态，下次打开是干净的菜单
      setMode({ kind: 'menu' })
      setCommitMessage('')
      setBranchName('')
      setWorktreeBranch('')
      setWorktreeTarget('')
      setPrTitle('')
      setPrBody('')
      return
    }
    // 打开时按需探测 gh（仅探测一次；ghAvailable 反映 PATH，不需要
    // 在每次开关 popover 时反复跑）
    if (ghOk === null) {
      let cancelled = false
      void ghAvailable()
        .then((ok) => {
          if (!cancelled) setGhOk(ok)
        })
        .catch(() => {
          if (!cancelled) setGhOk(false)
        })
      return () => {
        cancelled = true
      }
    }
  }, [open, ghOk])

  const runCommit = async () => {
    if (!cwd || !commitMessage.trim()) return
    setMode({ kind: 'busy', label: '正在提交…' })
    try {
      const outcome = await gitCommit(cwd, commitMessage.trim())
      setMode({
        kind: 'success',
        message:
          outcome.status === 'created' ? '已提交' : '工作区干净，已跳过提交',
      })
    } catch (err) {
      setMode({
        kind: 'error',
        message: err instanceof Error ? err.message : String(err),
      })
    }
  }

  const runCreateBranch = async () => {
    const name = branchName.trim()
    if (!cwd || !name) return
    setMode({ kind: 'busy', label: '正在创建分支…' })
    try {
      await gitCreateBranch(cwd, name)
      onBranchChange?.(name)
      setMode({ kind: 'success', message: `已切换到 ${name}` })
    } catch (err) {
      setMode({
        kind: 'error',
        message: err instanceof Error ? err.message : String(err),
      })
    }
  }

  /** Auto-derive a worktree target dir from the branch name when the
   *  user leaves the target field blank: place it as a sibling of
   *  `cwd`, named `<repo-basename>-<branch-slug>`.  Mirrors the
   *  convention `git_worktree::create_named` (backend) used to use. */
  const deriveWorktreeTarget = React.useCallback(
    (branch: string) => {
      if (!cwd || !branch.trim()) return ''
      const segments = cwd.split('/').filter(Boolean)
      const repo = segments[segments.length - 1] ?? 'project'
      const parent = '/' + segments.slice(0, -1).join('/')
      const slug = branch.trim().replace(/[^A-Za-z0-9_-]+/g, '-')
      return `${parent}/${repo}-${slug}`
    },
    [cwd],
  )

  const runInitRepo = async () => {
    if (!cwd) return
    setMode({ kind: 'busy', label: '正在初始化 Git…' })
    try {
      await gitInitRepo(cwd)
      onGitRepoChanged?.()
      setMode({
        kind: 'success',
        message: '已在当前项目目录初始化 Git 仓库',
      })
    } catch (err) {
      setMode({
        kind: 'error',
        message: err instanceof Error ? err.message : String(err),
      })
    }
  }

  const runCreateWorktree = async () => {
    const branch = worktreeBranch.trim()
    if (!cwd || !branch) return
    const target = worktreeTarget.trim() || deriveWorktreeTarget(branch)
    if (!target) return
    setMode({ kind: 'busy', label: '正在创建 worktree…' })
    try {
      const project = await gitCreateWorktreeProject({
        cwd,
        target,
        branch,
        // project_name 不传 → backend 用 target basename，跟 ProjectRail
        // 现有命名习惯一致
      })
      onWorktreeProjectCreated?.(project)
      setMode({
        kind: 'success',
        message: `已创建 ${branch} worktree → 切换到新项目 "${project.name}" 继续`,
      })
    } catch (err) {
      setMode({
        kind: 'error',
        message: err instanceof Error ? err.message : String(err),
      })
    }
  }

  const runCreatePr = async () => {
    if (!cwd || !prTitle.trim()) return
    // gh 已探明缺失 → 直接进入草稿模式，不发 IPC（backend 会
    // 立刻返回 MissingBinary，等价于多一次 round-trip）
    if (ghOk === false) {
      setMode({ kind: 'prDraft', title: prTitle.trim(), body: prBody })
      return
    }
    setMode({ kind: 'busy', label: '正在提交并创建 PR…' })
    try {
      const result = await gitCommitPushPr({
        cwd,
        title: prTitle.trim(),
        body: prBody,
      })
      setMode({ kind: 'success', message: result })
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      // 兜底：探测期间用户已经按了提交，IPC 失败提示是 gh 相关 →
      // 转到草稿模式，避免给用户一段没头没尾的英文 stderr
      if (/gh\b|MissingBinary/i.test(message)) {
        setGhOk(false)
        setMode({ kind: 'prDraft', title: prTitle.trim(), body: prBody })
        return
      }
      setMode({ kind: 'error', message })
    }
  }

  const renderBody = () => {
    switch (mode.kind) {
      case 'menu':
        // 没有 .git 时把整张菜单缩成一项 init 引导，避免用户看见
        // 一堆灰着的"提交/推送/PR"操作不知从何下手。init 成功后父
        // 组件会刷新 isGitRepo，下次打开就回到正常菜单。
        if (noRepo) {
          return (
            <>
              <div className="px-3.5 pb-1 pt-2.5 text-[11.5px] text-black/40">
                Git 操作
              </div>
              <div className="px-3.5 pb-2 text-[11.5px] leading-5 text-black/55">
                当前项目目录还不是 Git 仓库。初始化后即可使用提交、分支、PR 等功能。
              </div>
              <ActionItem
                icon={<Sparkles className="h-[14px] w-[14px]" strokeWidth={1.75} />}
                label="在此目录初始化 Git 仓库"
                onClick={runInitRepo}
              />
              <div className="h-1.5" />
            </>
          )
        }
        return (
          <>
            <div className="px-3.5 pb-1 pt-2.5 text-[11.5px] text-black/40">
              Git 操作
            </div>
            {onOpenWorkbench && (
              <ActionItem
                icon={<PanelTopOpen className="h-[14px] w-[14px]" strokeWidth={1.75} />}
                label="查看 Git 状态…"
                onClick={() => {
                  setOpen(false)
                  onOpenWorkbench()
                }}
              />
            )}
            <ActionItem
              icon={<GitCommitHorizontal className="h-[14px] w-[14px]" strokeWidth={1.75} />}
              label="提交"
              onClick={() => setMode({ kind: 'commit' })}
            />
            <ActionItem
              icon={<UploadCloud className="h-[14px] w-[14px]" strokeWidth={1.75} />}
              label="推送"
              onClick={() => {
                // 推送需要 backend `git push` IPC（待补）。当前以 PR
                // 流程兜底——大多数场景"推送 → 让人审核"已经被 PR 流
                // 程覆盖；纯 push 后续再加。
                setMode({
                  kind: 'error',
                  message: '推送暂未单独支持，请使用「创建拉取请求」一键提交并推送。',
                })
              }}
            />
            <ActionItem
              icon={<GitPullRequestArrow className="h-[14px] w-[14px]" strokeWidth={1.75} />}
              label="创建拉取请求"
              onClick={() => setMode({ kind: 'pr' })}
            />
            <ActionItem
              icon={<GitBranch className="h-[14px] w-[14px]" strokeWidth={1.75} />}
              label="创建分支"
              onClick={() => setMode({ kind: 'createBranch' })}
            />
            {onWorktreeProjectCreated && (
              <ActionItem
                icon={<PanelTopOpen className="h-[14px] w-[14px]" strokeWidth={1.75} />}
                label="在新 worktree 里继续…"
                onClick={() => setMode({ kind: 'createWorktree' })}
              />
            )}
            <div className="h-1.5" />
          </>
        )

      case 'commit':
        return (
          <FormShell title="提交" onCancel={() => setMode({ kind: 'menu' })}>
            <textarea
              autoFocus
              value={commitMessage}
              onChange={(e) => setCommitMessage(e.target.value)}
              placeholder="Commit message"
              rows={3}
              className="w-full resize-none rounded-lg border border-black/10 bg-white/70 px-3 py-2 text-[13px] text-black/85 outline-none placeholder:text-black/30 focus:border-black/30"
            />
            <PrimaryButton disabled={!commitMessage.trim()} onClick={runCommit}>
              提交
            </PrimaryButton>
          </FormShell>
        )

      case 'createBranch':
        return (
          <FormShell title="创建分支" onCancel={() => setMode({ kind: 'menu' })}>
            <input
              autoFocus
              value={branchName}
              onChange={(e) => setBranchName(e.target.value)}
              placeholder="新分支名"
              className="w-full rounded-lg border border-black/10 bg-white/70 px-3 py-2 text-[13px] outline-none focus:border-black/30"
            />
            <PrimaryButton disabled={!branchName.trim()} onClick={runCreateBranch}>
              创建并检出
            </PrimaryButton>
          </FormShell>
        )

      case 'createWorktree': {
        const trimmedBranch = worktreeBranch.trim()
        const targetSuggestion = trimmedBranch ? deriveWorktreeTarget(trimmedBranch) : ''
        return (
          <FormShell title="新 worktree" onCancel={() => setMode({ kind: 'menu' })}>
            <input
              autoFocus
              value={worktreeBranch}
              onChange={(e) => setWorktreeBranch(e.target.value)}
              placeholder="分支名（不存在则自动创建）"
              className="w-full rounded-lg border border-black/10 bg-white/70 px-3 py-2 text-[13px] outline-none focus:border-black/30"
            />
            <input
              value={worktreeTarget}
              onChange={(e) => setWorktreeTarget(e.target.value)}
              placeholder={targetSuggestion || '目标目录（可选，默认与项目同级）'}
              className="w-full rounded-lg border border-black/10 bg-white/70 px-3 py-2 font-mono text-[12px] outline-none focus:border-black/30"
            />
            <p className="text-[11px] leading-4 text-black/40">
              成功后会自动注册为新项目；侧边栏会刷新出现，你可以直接切过去继续聊。
            </p>
            <PrimaryButton disabled={!trimmedBranch} onClick={runCreateWorktree}>
              创建 worktree 并打开
            </PrimaryButton>
          </FormShell>
        )
      }

      case 'pr':
        return (
          <FormShell title="创建拉取请求" onCancel={() => setMode({ kind: 'menu' })}>
            {ghOk === false && <GhMissingBanner />}
            <input
              autoFocus
              value={prTitle}
              onChange={(e) => setPrTitle(e.target.value)}
              placeholder="PR 标题"
              className="w-full rounded-lg border border-black/10 bg-white/70 px-3 py-2 text-[13px] outline-none focus:border-black/30"
            />
            <textarea
              value={prBody}
              onChange={(e) => setPrBody(e.target.value)}
              placeholder="PR 描述（可选）"
              rows={3}
              className="w-full resize-none rounded-lg border border-black/10 bg-white/70 px-3 py-2 text-[13px] outline-none placeholder:text-black/30 focus:border-black/30"
            />
            <PrimaryButton disabled={!prTitle.trim()} onClick={runCreatePr}>
              {ghOk === false ? '生成草稿' : '提交并创建'}
            </PrimaryButton>
          </FormShell>
        )

      case 'prDraft':
        return <PrDraftView title={mode.title} body={mode.body} onBack={() => setMode({ kind: 'pr' })} />


      case 'busy':
        return (
          <div className="flex items-center justify-center gap-2 py-7 text-[13px] leading-6 text-black/55">
            <Loader2 className="h-4 w-4 animate-spin" />
            {mode.label}
          </div>
        )

      case 'success':
        return (
          <div className="px-3.5 py-3.5">
            <div className="text-[13px] leading-6 text-emerald-700">{mode.message}</div>
            <button
              type="button"
              onClick={() => setOpen(false)}
              className="mt-2.5 w-full rounded-lg bg-black/80 px-3 py-1.5 text-[12px] font-medium text-white hover:opacity-90"
            >
              完成
            </button>
          </div>
        )

      case 'error':
        return (
          <div className="px-3.5 py-3.5">
            <div className="text-[11.5px] leading-5 text-rose-600/85">
              {mode.message}
            </div>
            <button
              type="button"
              onClick={() => setMode({ kind: 'menu' })}
              className="mt-2.5 w-full rounded-lg border border-black/10 bg-white px-3 py-1.5 text-[12px] font-medium text-black/70 hover:bg-black/[0.03]"
            >
              返回
            </button>
          </div>
        )
    }
  }

  return (
    <Popover open={open} onOpenChange={(next) => !disabled && setOpen(next)}>
      <PopoverTrigger asChild>
        <button
          type="button"
          disabled={disabled}
          className={cn(
            // 圆角胶囊 → 圆角方形：跟 Home 项目 picker 的触发按钮保持
            // 一致（rounded-lg + 浅灰底 + 黑细边）。
            'window-no-drag inline-flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-[12px] font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-60',
            // 没有 .git → 触发按钮换成琥珀色调，提示用户这里需要先 init；
            // 仍然 clickable，因为下拉里有"初始化 Git 仓库"。
            noRepo
              ? 'border-amber-200 bg-amber-50 text-amber-800 hover:border-amber-300 hover:bg-amber-100'
              : 'border-black/[0.08] text-black/55 hover:border-black/14 hover:bg-black/[0.05] hover:text-black/82',
            className,
          )}
          data-window-no-drag="true"
          aria-label="Git 操作"
          title={noRepo ? '当前目录尚未初始化 Git — 点击查看初始化选项' : undefined}
        >
          {noRepo ? (
            <Sparkles className="h-3.5 w-3.5" strokeWidth={1.75} />
          ) : (
            <GitCommitHorizontal className="h-3.5 w-3.5 text-black/55" strokeWidth={1.75} />
          )}
          <span>{noRepo ? '初始化 Git' : '提交'}</span>
          <ChevronDown className={cn('h-3 w-3', noRepo ? 'text-amber-600' : 'text-black/45')} />
        </button>
      </PopoverTrigger>
      <PopoverContent
        align="center"
        sideOffset={12}
        collisionPadding={16}
        className={cn(
          'w-[240px] overflow-hidden rounded-2xl border border-black/[0.06] bg-white/95 p-0 text-[13px] backdrop-blur-2xl backdrop-saturate-150',
          'shadow-[0_2px_4px_rgba(0,0,0,0.04),0_8px_20px_rgba(0,0,0,0.08),0_24px_56px_rgba(0,0,0,0.16),0_0_0_0.5px_rgba(0,0,0,0.04)]',
          'origin-[var(--radix-popover-content-transform-origin)] transition-all duration-200 ease-out',
          'data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 data-[state=open]:slide-in-from-top-1',
          'data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95',
        )}
      >
        {renderBody()}
      </PopoverContent>
    </Popover>
  )
}

function ActionItem({
  icon,
  label,
  onClick,
}: {
  icon: React.ReactNode
  label: string
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      // `outline-none` 抑制 WebKit `:focus-visible` 默认蓝色描边；
      // 用 `focus-visible:bg-...` 给键盘用户保留可见的焦点反馈，
      // 同时不会被 PopoverContent 的 overflow-hidden 切成横线。
      className="flex w-full items-center gap-2.5 px-3.5 py-1.5 text-left text-[11.5px] leading-6 text-black/82 outline-none transition-colors hover:bg-black/[0.035] focus-visible:bg-black/[0.05]"
    >
      <span className="text-black/55">{icon}</span>
      {label}
    </button>
  )
}

function FormShell({
  title,
  onCancel,
  children,
}: {
  title: string
  onCancel: () => void
  children: React.ReactNode
}) {
  return (
    <div className="px-3.5 py-2.5">
      <div className="mb-2 flex items-center justify-between">
        <span className="text-[11.5px] font-medium text-black/55">{title}</span>
        <button
          type="button"
          onClick={onCancel}
          className="flex size-5 items-center justify-center rounded-full text-black/35 hover:bg-black/[0.05] hover:text-black/70"
          aria-label="取消"
        >
          <X className="h-3 w-3" />
        </button>
      </div>
      <div className="flex flex-col gap-2">{children}</div>
    </div>
  )
}

function PrimaryButton({
  disabled,
  onClick,
  children,
}: {
  disabled?: boolean
  onClick: () => void
  children: React.ReactNode
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className="rounded-lg bg-black/80 px-3 py-1.5 text-[12px] font-medium text-white transition-opacity hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-50"
    >
      {children}
    </button>
  )
}

/** Inline 警告条：gh CLI 不可用时挂在 PR/Issue 表单顶部，给安装链接。 */
function GhMissingBanner() {
  return (
    <div className="flex items-start gap-2 rounded-lg border border-amber-200 bg-amber-50 px-2.5 py-2 text-[11.5px] leading-5 text-amber-800">
      <AlertTriangle className="mt-[2px] h-3.5 w-3.5 shrink-0" strokeWidth={2} />
      <div className="min-w-0 flex-1">
        <div className="font-medium">未检测到 gh CLI</div>
        <div className="text-amber-800/80">
          将以草稿形式生成命令，复制到终端执行；
          <a
            href={GH_INSTALL_URL}
            target="_blank"
            rel="noreferrer"
            className="ml-1 inline-flex items-center gap-0.5 underline underline-offset-2 hover:text-amber-900"
          >
            安装 gh
            <ExternalLink className="h-2.5 w-2.5" />
          </a>
        </div>
      </div>
    </div>
  )
}

/**
 * gh 不可用时的草稿视图。展示用户填写的 title + body，并把可执行的
 * `gh pr create` 命令拼出来；点 Copy 复制整段命令到剪贴板，用户即可
 * 在装好 gh 后直接粘贴执行。
 *
 * Body 经过 shell-escape 嵌入 `--body $'...'`：避免单引号 / 反斜杠
 * 截断命令；用 `$''` ANSI-C 引用形式，转义换行 / 单引号。
 */
function PrDraftView({
  title,
  body,
  onBack,
}: {
  title: string
  body: string
  onBack: () => void
}) {
  const [copied, setCopied] = React.useState(false)

  const command = React.useMemo(() => {
    const escapedTitle = shellAnsiCQuote(title)
    const escapedBody = shellAnsiCQuote(body || '(no body provided)')
    return `gh pr create --title ${escapedTitle} --body ${escapedBody}`
  }, [title, body])

  const onCopy = async () => {
    try {
      await navigator.clipboard.writeText(command)
      setCopied(true)
      window.setTimeout(() => setCopied(false), 1600)
    } catch {
      /* clipboard unavailable; surface no error — Copy button is best-effort */
    }
  }

  return (
    <div className="px-3.5 py-2.5">
      <div className="mb-2 flex items-center justify-between">
        <span className="text-[11.5px] font-medium text-black/55">PR 草稿</span>
        <button
          type="button"
          onClick={onBack}
          className="flex size-5 items-center justify-center rounded-full text-black/35 hover:bg-black/[0.05] hover:text-black/70"
          aria-label="返回"
        >
          <X className="h-3 w-3" />
        </button>
      </div>
      <GhMissingBanner />
      <div className="mt-2 space-y-1.5">
        <div className="text-[11px] uppercase tracking-wider text-black/40">命令</div>
        <pre className="m-0 max-h-[160px] overflow-auto whitespace-pre-wrap break-all rounded-lg border border-black/[0.06] bg-black/[0.03] px-2.5 py-2 font-mono text-[11.5px] leading-5 text-black/75">
          {command}
        </pre>
        <button
          type="button"
          onClick={onCopy}
          className="inline-flex items-center gap-1.5 rounded-lg bg-black/80 px-3 py-1.5 text-[12px] font-medium text-white hover:opacity-90"
        >
          {copied ? (
            <>
              <Check className="h-3.5 w-3.5" />
              已复制
            </>
          ) : (
            <>
              <Copy className="h-3.5 w-3.5" />
              复制命令
            </>
          )}
        </button>
      </div>
    </div>
  )
}

/**
 * 把任意字符串编码成 bash ANSI-C ($'...') 引用形式：
 * `\` `'` 都转义；换行变成 `\n`，回车 `\r`，制表 `\t`。
 * 这样 `gh pr create --body $'...'` 在用户终端里粘贴后能保留多行
 * body 不被截断。
 */
function shellAnsiCQuote(value: string): string {
  const escaped = value
    .replace(/\\/g, '\\\\')
    .replace(/'/g, "\\'")
    .replace(/\n/g, '\\n')
    .replace(/\r/g, '\\r')
    .replace(/\t/g, '\\t')
  return `$'${escaped}'`
}
