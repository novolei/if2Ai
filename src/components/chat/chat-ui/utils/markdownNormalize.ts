/**
 * Markdown / ASCII diagram normalization helpers shared by chat-ui
 * markdown / code blocks.
 *
 * Extracted verbatim from `src/components/ui/chat-ui.tsx` (GF-01 PR-02).
 * Function bodies are unchanged — only the surrounding declaration is
 * relocated (refactor invariant I6).
 */

/**
 * Walk a markdown source and rewrite ASCII relationship-diagram code
 * fences into the cleaned blockquote form, leaving other content alone.
 */
export function normalizeAsciiDiagramBlocks(content: string): string {
  return content.replace(/```(?:[\w-]+)?\n([\s\S]*?)```/g, (fullMatch, rawBlock) => {
    const transformed = transformAsciiRelationshipBlock(rawBlock)
    return transformed ?? fullMatch
  })
}

/**
 * Convert a single ASCII relationship-box block into a blockquote
 * heading-list narrative. Returns `null` if the input is not a
 * relationship box.
 */
export function transformAsciiRelationshipBlock(rawBlock: string): string | null {
  const normalized = rawBlock.replace(/\r\n/g, '\n').trim()
  if (!looksLikeRelationshipBox(normalized)) return null

  const cleanedLines = normalized
    .split('\n')
    .map((line) => stripBoxDrawingLine(line))
    .filter((line) => line.length > 0 && !looksLikeDecorativeNoiseLine(line))

  if (cleanedLines.length < 3) return null

  const firstLine = cleanedLines[0]
  const bodyLines = cleanedLines.slice(1)
  const quoteLines: string[] = [`> **${firstLine}**`, '>']

  for (const line of bodyLines) {
    const bulletText = line.replace(/^([✦•◆▪●○\-*]+)\s*/, '').trim()
    if (/^([✦•◆▪●○\-*]+)/.test(line)) {
      quoteLines.push(`> - ${bulletText}`)
      continue
    }
    if (looksLikeRelationshipHeading(line)) {
      if (quoteLines[quoteLines.length - 1] !== '>') {
        quoteLines.push('>')
      }
      quoteLines.push(`> **${line}**`)
      continue
    }
    quoteLines.push(`> ${line}`)
  }

  return quoteLines.join('\n')
}

/**
 * Heuristic: `text` looks like a relationship-box (enough box-drawing
 * chars + bullets / numbered headings / heading lines, low code-signal
 * density).
 */
export function looksLikeRelationshipBox(text: string): boolean {
  const boxCharCount = text.match(/[│┌┐└┘─/\\_|-]/g)?.length ?? 0
  const bulletCount = text.match(/^[ \t]*[✦•◆▪●○\-*]/gm)?.length ?? 0
  const numberedCount = text.match(/^[ \t]*(?:\d+[.)]|\d+️⃣|[①②③④⑤⑥⑦⑧⑨⑩]|🔟)/gm)?.length ?? 0
  const headingCount = text
    .replace(/\r\n/g, '\n')
    .split('\n')
    .map((line) => stripBoxDrawingLine(line))
    .filter((line) => looksLikeRelationshipHeading(line)).length
  const codeSignalCount = text.match(/[{}();=<>]/g)?.length ?? 0
  return boxCharCount >= 8 && codeSignalCount < 6 && (bulletCount >= 2 || numberedCount >= 2 || headingCount >= 3)
}

/**
 * Heuristic: a `line` looks like a numbered relationship heading
 * (decimal, full-width circled, or keycap).
 */
export function looksLikeRelationshipHeading(line: string): boolean {
  const normalized = line.trim()
  if (!normalized) return false
  return /^(?:\d+[.)]|\d+️⃣|[①②③④⑤⑥⑦⑧⑨⑩]|🔟)\s*/.test(normalized)
}

/** Strip leading/trailing box-drawing characters and decorative noise. */
export function stripBoxDrawingLine(line: string): string {
  const trimmed = line.trim()
  if (!trimmed) return ''
  if (/^[┌┐└┘─│/\\_|+\-\s]+$/.test(trimmed)) return ''

  const cleaned = trimmed
    .replace(/^[│\s]+/, '')
    .replace(/[│\s]+$/, '')
    .replace(/^[┌┐└┘─/\\_|+\-\s]+/, '')
    .replace(/[┌┐└┘─/\\_|+\-\s]+$/, '')
    .trim()
    .replace(/\s+[\/\\|]+\s*$/g, '')
    .replace(/^\s*[\/\\|]+\s+/g, '')
    .trim()

  if (looksLikeDecorativeNoiseLine(cleaned)) return ''
  return cleaned
}

/** True if `line` is empty or pure box-edge / underscore noise. */
export function looksLikeDecorativeNoiseLine(line: string): boolean {
  const trimmed = line.trim()
  if (!trimmed) return true
  return /^[\/\\|_\-+]+$/.test(trimmed)
}

/**
 * Stable, fast 32-bit FNV-1a hash for memo cache keys. Matches the
 * legacy `markdownNormalizeCache` keying byte-for-byte.
 */
export function hashString(input: string): number {
  let hash = 0x811c9dc5
  for (let i = 0; i < input.length; i += 1) {
    hash ^= input.charCodeAt(i)
    hash = Math.imul(hash, 0x01000193)
  }
  return hash >>> 0
}

/**
 * Light cleanup for code blocks that look like a directory tree —
 * collapses excess blank lines while preserving tree-art alignment.
 */
export function normalizeCodeForDisplay(text: string): string {
  if (!looksLikeTreeText(text)) return text
  return text
    .replace(/\r\n/g, '\n')
    .replace(/\n[ \t]*\n(?=[ \t]*[│├└┌┐┬┼─|])/g, '\n')
    .replace(/\n{3,}/g, '\n\n')
}

/**
 * Heuristic: text contains enough tree-drawing characters and file-like
 * tokens to be considered a directory listing.
 */
export function looksLikeTreeText(text: string): boolean {
  const treeCharCount = text.match(/[│├└┌┐┬┼─]/g)?.length ?? 0
  const fileLikeCount = text.match(/([A-Za-z0-9._-]+\/|[A-Za-z0-9._-]+\.[A-Za-z0-9]+)/g)?.length ?? 0
  return treeCharCount >= 3 && fileLikeCount >= 4
}

/**
 * Heuristic: a code-fenced block looks like prose / dialogue rather
 * than code, based on punctuation density and CJK content.
 */
export function looksLikeNarrativeTextBlock(text: string): boolean {
  const normalized = text.replace(/\r\n/g, '\n').trim()
  if (!normalized || normalized.length < 16) return false

  const contentLines = normalized
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
  const lineCount = contentLines.length
  const punctuationSignals = normalized.match(/[「」『』。！？：…]/g)?.length ?? 0
  const dialogueSignals = normalized.match(/(^|\n)\s*[\p{Script=Han}A-Za-z0-9_-]{1,12}[:：]/gu)?.length ?? 0
  const quoteLineCount = contentLines.filter((line) => /^[[「『“"']/u.test(line)).length
  const codeSignals = normalized.match(/[{}();=<>`]/g)?.length ?? 0
  const cjkCharCount = normalized.match(/[\p{Script=Han}]/gu)?.length ?? 0
  const shortLineCount = contentLines.filter((line) => line.length <= 28).length
  const plainLineCount = contentLines.filter((line) => !/^[\-*•◆▪●○]/.test(line)).length

  if (codeSignals >= 6) return false

  if (lineCount >= 3 && punctuationSignals >= 3 && dialogueSignals >= 1) {
    return true
  }

  if (lineCount >= 2 && quoteLineCount === lineCount && punctuationSignals >= 2) {
    return true
  }

  if (lineCount >= 3 && cjkCharCount >= 12 && punctuationSignals >= 1 && shortLineCount >= 2) {
    return true
  }

  if (lineCount >= 3 && plainLineCount === lineCount && shortLineCount >= 2 && cjkCharCount >= 10) {
    return true
  }

  return lineCount >= 4 && cjkCharCount >= 16 && codeSignals === 0
}

/** Whitespace cleanup for narrative-text blocks. */
export function normalizeNarrativeTextForDisplay(text: string): string {
  return text
    .replace(/\r\n/g, '\n')
    .replace(/\n{3,}/g, '\n\n')
    .replace(/[ \t]+\n/g, '\n')
}

/**
 * Heuristic: a single-line block that looks like a `|`-separated
 * keyword list rather than code.
 */
export function looksLikeKeywordLineBlock(text: string): boolean {
  const normalized = text.replace(/\r\n/g, '\n').trim()
  if (!normalized || normalized.includes('\n')) return false
  const separatorCount = normalized.match(/\|/g)?.length ?? 0
  const codeSignals = normalized.match(/[{}();=<>`[\]]/g)?.length ?? 0
  const cjkCharCount = normalized.match(/[\p{Script=Han}A-Za-z]/gu)?.length ?? 0
  return separatorCount >= 4 && codeSignals === 0 && cjkCharCount >= 8
}

/** Reformat a `|`-separated keyword line for display. */
export function normalizeKeywordLineForDisplay(text: string): string {
  return text
    .replace(/\s*\|\s*/g, '  |  ')
    .replace(/\s{3,}/g, '  ')
}
