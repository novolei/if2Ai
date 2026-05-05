/**
 * Pure builder helpers used by chat-ui tool cards. Extracted from
 * `src/components/ui/chat-ui.tsx` (GF-01 PR-02) without behavioural
 * change.
 */

import type { Message } from '@/components/ui/chat-ui'
import { redactSensitiveText, truncateText } from '@/components/chat/chat-ui/utils/text'
import { buildDiagnosticCopyText } from '@/components/chat/chat-ui/utils/diagnosticCopy'

/** Union of transport tool-attempt status plus the local "error" state. */
export type ChatToolStatus = import('@/transport/contracts').ToolAttemptStatus | 'error'

/**
 * Resolve the effective `ChatToolStatus` for a message, falling back to
 * legacy heuristics (`isError`, empty content) when explicit status is
 * missing.
 */
export function normalizeToolStatus(message: Message): ChatToolStatus {
  if (message.toolStatus) return message.toolStatus
  if (message.isError) return 'error'
  if (message.content.trim()) return 'completed'
  return 'running'
}

/** True when the status represents an in-flight attempt. */
export function isToolPendingStatus(status: ChatToolStatus): boolean {
  return status === 'queued' || status === 'authorizing' || status === 'running' || status === 'retrying'
}

/** True when the status represents a terminal failure. */
export function isToolFailureStatus(status: ChatToolStatus): boolean {
  return status === 'failed' || status === 'error' || status === 'blocked' || status === 'cancelled'
}

/** Tool-card display payload built from a tool message + status. */
export type ToolDisplay = {
  title: string
  details: string[]
  copyText: string
  resultCopyText: string | null
  diagnosticCopyText: string | null
}

/**
 * Compose a fully populated `ToolDisplay` for a tool message: title +
 * up to 6 detail lines + clipboard variants. Pure; safe to memoise on
 * `(message, defaultWorkdir, status)`.
 */
export function buildToolCallDisplay(
  message: Message,
  defaultWorkdir?: string,
  status: ChatToolStatus = 'completed'
): ToolDisplay {
  const toolName = (message.toolName ?? 'unknown_tool').trim()
  const args = message.toolArgs ?? {}
  const normalizedToolName = toolName.toLowerCase()
  const command = pickToolString(args, ['command', 'cmd', 'shell'])
  const path = pickToolString(args, ['path', 'file', 'file_path', 'target', 'cwd', 'directory', 'base_path', 'workdir'])
  const pattern = pickToolString(args, ['pattern', 'query', 'prompt', 'text'])
  const location = pickToolString(args, ['location', 'city', 'place'])
  const skillName = pickToolString(args, ['skill', 'name', 'title', 'path'])
  const resultSummary = summarizeToolResult(message.content)
  const titleText = pickToolHeadline(
    normalizedToolName,
    status,
    args,
    command,
    path,
    pattern,
    location,
    skillName,
    resultSummary,
    message.content,
    defaultWorkdir
  )
  const detailLines = buildToolDetailLines(
    normalizedToolName,
    args,
    command,
    path,
    pattern,
    location,
    skillName,
    resultSummary,
    defaultWorkdir
  )
  if (message.policyDecision) {
    detailLines.push(`策略：${message.policyDecision}`)
  }
  if (message.effectiveWorkdir) {
    detailLines.push(`目录：${shortenMiddle(message.effectiveWorkdir, 56)}`)
  }
  if (message.evidenceId) {
    detailLines.push(`证据：${shortenMiddle(message.evidenceId, 32)}`)
  }
  if (message.requestId) {
    detailLines.push(`请求：${shortenMiddle(message.requestId, 32)}`)
  }
  const diagnosticCopyText = buildDiagnosticCopyText(message)
  if (diagnosticCopyText) {
    detailLines.push(`诊断：${shortenMiddle(diagnosticCopyText, 56)}`)
  }

  return {
    title: titleText,
    details: detailLines.slice(0, 6),
    copyText: redactSensitiveText(titleText),
    resultCopyText: resultSummary ? redactSensitiveText(resultSummary) : null,
    diagnosticCopyText: diagnosticCopyText ? redactSensitiveText(diagnosticCopyText) : null,
  }
}

/**
 * Pick a one-line headline for the tool card based on tool name +
 * status + key arg fields. Always returns a non-empty string.
 */
export function pickToolHeadline(
  toolName: string,
  status: ChatToolStatus,
  args: Record<string, unknown>,
  command: string | null,
  path: string | null,
  pattern: string | null,
  location: string | null,
  skillName: string | null,
  resultSummary: string,
  content: string,
  defaultWorkdir?: string
): string {
  const declaredTitle = pickToolString(args, ['title'])
  if (declaredTitle) return truncateText(declaredTitle, 64)

  const declaredName = pickToolString(args, ['name'])
  if (declaredName && declaredName.trim() !== toolName) {
    return truncateText(declaredName, 64)
  }

  const contentPreview = content.trim()
  const looksStructured = /^[\[{]/.test(contentPreview) || /"exit_code"|"stdout"|"stderr"|"status"/.test(contentPreview)
  if (looksStructured && resultSummary) {
    return truncateText(resultSummary, 64)
  }

  if (contentPreview && contentPreview.length <= 48 && !contentPreview.includes('\n')) {
    if (!looksStructured) {
      return truncateText(contentPreview, 64)
    }
  }

  const targetPath = path || defaultWorkdir || ''

  if (toolName.includes('glob_search')) {
    return targetPath ? `搜索了 ${shortenMiddle(targetPath, 20)} 中的文件` : '搜索了当前目录中的文件'
  }

  if (toolName.includes('grep_search') || toolName.includes('content_search')) {
    const target = targetPath ? shortenMiddle(targetPath, 20) : '当前目录'
    return pattern ? `在 ${target} 中搜索 ${shortenMiddle(pattern, 20)}` : `在 ${target} 中搜索内容`
  }

  if (toolName.includes('read_file')) {
    return path ? `查看了 ${shortenMiddle(path, 28)}` : '查看了文件'
  }

  if (toolName.includes('file_write')) {
    if (isToolPendingStatus(status)) {
      return path ? `正在写入 ${shortenMiddle(path, 28)}` : '正在写入文件'
    }
    if (isToolFailureStatus(status)) {
      return path ? `写入失败 ${shortenMiddle(path, 28)}` : '写入失败'
    }
    return path ? `已写入 ${shortenMiddle(path, 28)}` : '已写入文件'
  }

  if (toolName.includes('web_search')) {
    return pattern ? `搜索了 ${shortenMiddle(pattern, 28)}` : '搜索了网页信息'
  }

  if (toolName.includes('weather')) {
    return location ? `查询了 ${shortenMiddle(location, 24)} 天气` : pattern ? `查询了 ${shortenMiddle(pattern, 24)} 天气` : '查询了天气'
  }

  if (toolName.includes('tool_search')) {
    return pattern ? `搜索了工具 ${shortenMiddle(pattern, 24)}` : '搜索了可用工具'
  }

  if (toolName.includes('skill_search')) {
    return pattern ? `搜索了技能 ${shortenMiddle(pattern, 24)}` : '搜索了可用技能'
  }

  if (toolName === 'skill' || toolName.includes('skill/') || toolName.includes('.skill')) {
    return skillName ? `调用了 ${shortenMiddle(skillName, 24)}` : '调用了技能'
  }

  if (command) {
    return `执行了 ${truncateText(redactSensitiveText(command), 48)}`
  }

  if (path) {
    return `执行了 ${shortenMiddle(path, 28)}`
  }

  if (pattern) {
    return `执行了 ${shortenMiddle(pattern, 28)}`
  }

  if (location) {
    return `执行了 ${shortenMiddle(location, 28)}`
  }

  return `执行了 ${toolName || '工具'}`
}

/**
 * Compose the secondary (sub-title) detail lines for the tool card,
 * capped at 3 entries.
 */
export function buildToolDetailLines(
  toolName: string,
  args: Record<string, unknown>,
  command: string | null,
  path: string | null,
  pattern: string | null,
  location: string | null,
  skillName: string | null,
  resultSummary: string,
  defaultWorkdir?: string
): string[] {
  const detailLines: string[] = []
  const targetPath = path || defaultWorkdir || ''
  const summaryLine = buildToolCommandResultLine(toolName, command, skillName, resultSummary)

  if (summaryLine) {
    detailLines.push(summaryLine)
  }

  if (toolName.includes('glob_search')) {
    if (targetPath) detailLines.push(`范围：${shortenMiddle(targetPath, 56)}`)
    if (pattern) detailLines.push(`模式：${shortenMiddle(pattern, 56)}`)
  } else if (toolName.includes('grep_search') || toolName.includes('content_search')) {
    if (targetPath) detailLines.push(`路径：${shortenMiddle(targetPath, 56)}`)
    if (pattern) detailLines.push(`模式：${shortenMiddle(pattern, 56)}`)
  } else if (toolName.includes('read_file')) {
    if (path) detailLines.push(`文件：${shortenMiddle(path, 56)}`)
  } else if (toolName.includes('file_write')) {
    if (path) detailLines.push(`写入：${shortenMiddle(path, 56)}`)
  } else if (toolName.includes('web_search')) {
    if (pattern) detailLines.push(`搜索：${shortenMiddle(pattern, 56)}`)
  } else if (toolName.includes('weather')) {
    if (location) detailLines.push(`地点：${shortenMiddle(location, 56)}`)
    else if (pattern) detailLines.push(`地点：${shortenMiddle(pattern, 56)}`)
  } else if (toolName.includes('tool_search') || toolName.includes('skill_search')) {
    if (pattern) detailLines.push(`关键词：${shortenMiddle(pattern, 56)}`)
  } else if (toolName === 'skill' || toolName.includes('skill/') || toolName.includes('.skill')) {
    if (skillName) detailLines.push(`技能：${shortenMiddle(skillName, 56)}`)
  } else if (command && !summaryLine) {
    detailLines.push(`命令：${truncateText(redactSensitiveText(command), 92)}`)
  }

  if (resultSummary && !summaryLine) {
    detailLines.push(`结果：${truncateText(resultSummary, 96)}`)
  }

  const declaredStatus = pickToolString(args, ['status'])
  if (declaredStatus && !detailLines.some((line) => line.includes(declaredStatus))) {
    detailLines.push(`状态：${shortenMiddle(declaredStatus, 32)}`)
  }

  return detailLines.slice(0, 3)
}

/** Single composite "tool · result" summary line. */
export function buildToolCommandResultLine(
  toolName: string,
  command: string | null,
  skillName: string | null,
  resultSummary: string
) {
  const toolOrCommandPart = command
    ? `命令：${truncateText(redactSensitiveText(command), resultSummary ? 68 : 108)}`
    : skillName
      ? `工具：${truncateText(skillName, resultSummary ? 34 : 72)}`
      : `工具：${truncateText(normalizeToolLabel(toolName), resultSummary ? 22 : 48)}`
  const resultPart = resultSummary
    ? `结果：${truncateText(resultSummary, command ? 24 : 52)}`
    : ''

  if (toolOrCommandPart && resultPart) {
    return `${toolOrCommandPart} · ${resultPart}`
  }

  return toolOrCommandPart || resultPart || ''
}

/**
 * Map an arbitrary tool name onto its canonical short label
 * (`grep_search`, `read_file`, …). Falls back to the trimmed input.
 */
export function normalizeToolLabel(toolName: string) {
  const normalized = toolName.trim().toLowerCase()

  if (!normalized) return 'unknown_tool'
  if (normalized.includes('glob_search')) return 'glob_search'
  if (normalized.includes('grep_search') || normalized.includes('content_search')) return 'grep_search'
  if (normalized.includes('read_file')) return 'read_file'
  if (normalized.includes('file_write')) return 'file_write'
  if (normalized.includes('web_search')) return 'web_search'
  if (normalized.includes('weather')) return 'weather'
  if (normalized.includes('tool_search')) return 'tool_search'
  if (normalized.includes('skill_search')) return 'skill_search'
  if (normalized === 'skill' || normalized.includes('skill/') || normalized.includes('.skill')) return 'skill'

  return toolName.trim()
}

/** Return the first non-empty string at any of the requested keys. */
export function pickToolString(args: Record<string, unknown>, keys: readonly string[]): string | null {
  for (const key of keys) {
    const value = args[key]
    if (typeof value === 'string' && value.trim()) {
      return value.trim()
    }
  }

  return null
}

/**
 * Summarise a tool result payload into a short single-line label.
 * Recognises memory_store / file_write structured payloads, exec
 * exit_code/stdout, and array length results.
 */
export function summarizeToolResult(content: string): string {
  const trimmed = content.trim()
  if (!trimmed) return ''

  const parsed = tryParseJson(trimmed)
  if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
    const record = parsed as Record<string, unknown>

    // memory_store structured payload — emit a human readable summary
    // instead of the raw JSON ("Stored memory: <key>" replaces what the
    // legacy plain-string handler used to produce, while pending_approval
    // / denied surface their own dedicated cards above).
    if (typeof record.status === 'string' && typeof record.key === 'string') {
      const key = record.key as string
      switch (record.status) {
        case 'stored':
          return `已写入记忆：${truncateText(key, 80)}`
        case 'pending_approval':
          return `等待审批：${truncateText(key, 80)}`
        case 'denied':
          return `已拒绝写入：${truncateText(key, 80)}`
        default:
        // Fall through to generic handling.
      }
    }

    // file_write structured payload — emit a friendly one-liner so the
    // tool-card summary doesn't dump the raw JSON (which can carry up
    // to 64 KiB of pre-write content for the diff viewer).
    if (record.kind === 'file_write' && record.ok === true) {
      const path = typeof record.path === 'string' ? record.path : ''
      return path ? `已写入 ${truncateText(path, 80)}` : '已写入文件'
    }

    const exitCode = typeof record.exit_code === 'number' ? record.exit_code : null
    const stdout = typeof record.stdout === 'string' ? record.stdout.trim() : ''
    const stderr = typeof record.stderr === 'string' ? record.stderr.trim() : ''

    if (stdout || stderr || exitCode !== null) {
      const output = stdout || stderr
      const outputSummary = output ? truncateText(firstNonEmptyLine(output), 72) : ''
      const statusText = exitCode === 0 ? '执行完成' : '执行失败'
      return outputSummary ? `${statusText}：${outputSummary}` : statusText
    }

    const count = record.count
    if (typeof count === 'number') {
      return `返回 ${count} 项结果`
    }

    for (const key of [
      'results',
      'items',
      'entries',
      'files',
      'todos',
      'new_todos',
      'newTodos',
    ] as const) {
      const value = record[key]
      if (Array.isArray(value)) {
        return `返回 ${value.length} 项结果`
      }
    }

    // For generic JSON object outputs (like TodoWrite payloads),
    // avoid falling back to the first line "{" in tool cards.
    return '返回对象结果'
  }

  if (Array.isArray(parsed)) {
    return `返回 ${parsed.length} 项结果`
  }

  const firstLine = firstNonEmptyLine(trimmed)
  if (!firstLine) return ''
  if (firstLine.length <= 120 && !firstLine.includes('\n')) {
    return truncateText(firstLine, 120)
  }

  return truncateText(firstLine, 120)
}

/** Try `JSON.parse(text)`; return `null` on failure. */
export function tryParseJson(text: string): unknown | null {
  try {
    return JSON.parse(text)
  } catch {
    return null
  }
}

/** Return the first non-empty line in `text`, trimmed. */
export function firstNonEmptyLine(text: string): string {
  return text.split(/\r?\n/).find((line) => line.trim())?.trim() ?? ''
}

/**
 * Shorten `text` to at most `maxLength` chars by removing the middle and
 * inserting an ellipsis. Keeps at least 8 chars on each side.
 */
export function shortenMiddle(text: string, maxLength: number): string {
  const value = text.trim()
  if (value.length <= maxLength) return value

  const visible = Math.max(8, Math.floor((maxLength - 1) / 2))
  const start = value.slice(0, visible)
  const end = value.slice(-visible)
  return `${start}…${end}`
}
