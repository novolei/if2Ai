/**
 * TTS 文本预处理（去除 emoji / Markdown / URL / 代码块等会让朗读出戏的元素）。
 *
 * 两个入口：
 * - `sanitizeForTts(text)` — 一次性清洗整段文本（用于手动 ▶ 按钮的整条消息）。
 * - 流式场景见 `useAgentVoiceBridge`：在 push 进合成队列前对每个 sentence 调
 *   `sanitizeForTts`；并通过 `stripFencedCodeBlocks` 对累积 buffer 提前删去尚未读完的
 *   ```代码块``` 段（避免朗读 "三反引号 t y p e s c r i p t ..."）。
 *
 * 设计取舍：
 * - 删除而非保留：emoji、URL、行内/块级代码 → 直接抹掉；保留只会让 TTS 念出"
 *   slash s l a s h"或"emoji name"。
 * - 保留语义：Markdown 加粗/斜体/标题/链接锚文本 → 留下纯文字。
 * - 控制标点：句末"~~~"→ ""；中文「。」「，」之间多个空白 → 单空格。
 *
 * 实现注意：
 * - 不依赖任何外部库（emoji-regex 600KB；这里用 Unicode property escapes，Chrome 84+
 *   原生支持，目标平台 Tauri WebView 都满足）。
 * - 顺序敏感：先去围栏代码块（多行）→ 行内代码 → 图片/链接 → emoji → 头部/列表/引用 →
 *   强调标记 → 多余空白。每一步都是幂等的。
 */

/* ──────── 围栏代码块（多行 ```...``` 或 ~~~...~~~） ─────────────────────────── */

/** 删除文本中所有完整的 ```fenced``` 代码块（含围栏开头的语言名）。 */
export function stripFencedCodeBlocks(text: string): string {
  // 处理 ``` 与 ~~~ 两种围栏；非贪婪匹配，跨行
  return text
    .replace(/```[\s\S]*?```/g, ' ')
    .replace(/~~~[\s\S]*?~~~/g, ' ')
}

/**
 * 流式场景使用：检测 buffer 中是否存在**未闭合**的围栏开始标记。
 * 如果有，返回到该开始标记之前的安全部分（已闭合的代码块也已被剥离）。
 *
 * @returns `{ safe, remainder }`
 *   - `safe`：可以喂给 sentence boundary 检测器、保证不含未闭合代码块的部分
 *   - `remainder`：剩下的（尚未闭合代码块 + 之后的内容），下次 delta 来时继续累积
 */
export function splitOnUnclosedFence(buffer: string): { safe: string; remainder: string } {
  // 先去掉所有已闭合的代码块
  const closed = stripFencedCodeBlocks(buffer)
  // 再看剩余文本里是否还有 ``` 或 ~~~（一定是 unmatched）
  const m = /```|~~~/.exec(closed)
  if (!m) return { safe: closed, remainder: '' }
  return {
    safe: closed.slice(0, m.index),
    remainder: closed.slice(m.index),
  }
}

/* ──────── Emoji + 杂项符号 ────────────────────────────────────────────────── */

/**
 * 去除 emoji + 各类符号 pictographs。
 * 用 Unicode property escapes 覆盖：
 * - `\p{Extended_Pictographic}`：所有 emoji（含 ❤️ ✅ 等）
 * - `\u{FE0F}` / `\u{200D}`：变体选择符 + ZWJ（emoji 序列连接符）
 * - `\p{Emoji_Component}`：肤色等修饰符
 *
 * 同时保留正常文字（CJK / 拉丁 / 数字）。
 */
function stripEmoji(text: string): string {
  // 注意：分多次扫描，避免一次性大正则过慢
  return text
    .replace(/\p{Extended_Pictographic}/gu, '')
    .replace(/[\u{FE00}-\u{FE0F}]/gu, '') // 变体选择符
    .replace(/\u200D/g, '') // ZWJ
    .replace(/[\u{1F1E6}-\u{1F1FF}]/gu, '') // 区域指示符（国旗 emoji 的两个组成字符）
}

/* ──────── URL / HTML / 杂项 markdown ─────────────────────────────────────── */

const URL_REGEX = /\bhttps?:\/\/[^\s<>"')]+/gi
const BARE_DOMAIN_REGEX = /\b(?:www\.)[^\s<>"')]+/gi

function stripUrls(text: string): string {
  return text.replace(URL_REGEX, ' ').replace(BARE_DOMAIN_REGEX, ' ')
}

function stripHtmlTags(text: string): string {
  // 简单去标签；不解析实体（&nbsp; &amp; 等罕见到值得依赖 dompurify）
  return text.replace(/<\/?[a-zA-Z][^>]*>/g, ' ')
}

/* ──────── Markdown 标记 ──────────────────────────────────────────────────── */

function stripMarkdownTokens(text: string): string {
  return (
    text
      // 行内代码 `xxx`
      .replace(/`[^`\n]*`/g, ' ')
      // 图片 ![alt](url) → alt（图片描述往往有用）
      .replace(/!\[([^\]]*)\]\([^)]*\)/g, '$1')
      // 链接 [文字](url) → 文字
      .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
      // 引用链接 [文字][ref] → 文字
      .replace(/\[([^\]]+)\]\[[^\]]*\]/g, '$1')
      // 引用定义 [ref]: url → 删除整行
      .replace(/^\s*\[[^\]]+\]:\s*\S+.*$/gm, '')
      // 标题 # ## ### → 删 # 保留文字（句末加句号让 TTS 自然停顿）
      .replace(/^\s{0,3}#{1,6}\s+(.+?)\s*#*\s*$/gm, '$1。')
      // 列表项 - / * / + / 1. → 删除标记
      .replace(/^\s*[-*+]\s+/gm, '')
      .replace(/^\s*\d+[.)]\s+/gm, '')
      // 引用 > xxx → 删 >
      .replace(/^\s*>+\s?/gm, '')
      // 表格分隔行 |---|---|
      .replace(/^\s*\|?\s*[:|\- ]+\|[:|\- ]+\|?\s*$/gm, '')
      // 表格内容行的 | 分隔符 → 逗号停顿
      .replace(/\s*\|\s*/g, '，')
      // 加粗 **xxx** / __xxx__
      .replace(/\*\*([^*\n]+)\*\*/g, '$1')
      .replace(/__([^_\n]+)__/g, '$1')
      // 斜体 *xxx* / _xxx_（避免误伤 file_name；要求两侧非字符）
      .replace(/(^|[\s(])[*_]([^\s*_][^*_\n]*?)[*_](?=[\s).,!?;:，。！？；：]|$)/g, '$1$2')
      // 删除线 ~~xxx~~
      .replace(/~~([^~\n]+)~~/g, '$1')
      // 水平线 --- *** ___
      .replace(/^\s*([-*_])\1{2,}\s*$/gm, '')
  )
}

/* ──────── 标点 / 空白归一化 ───────────────────────────────────────────────── */

function normalizePunctuation(text: string): string {
  return (
    text
      // 全角省略号 → 中文省略号
      .replace(/\.{3,}/g, '…')
      // em-dash / en-dash → 中文逗号停顿
      .replace(/[—–]+/g, '，')
      // 多个连续标点 → 仅保留第一个
      .replace(/([。！？，；：])\1+/g, '$1')
      // 多空格/制表符 → 单空格
      .replace(/[ \t]+/g, ' ')
      // 多换行 → 段落符（TTS 会做停顿）
      .replace(/\n{2,}/g, '\n')
      .trim()
  )
}

/* ──────── 入口 ────────────────────────────────────────────────────────────── */

/**
 * 把任意 Markdown / 富文本内容清洗为 TTS 友好的纯文本。
 *
 * 处理顺序固定：围栏代码 → URL → HTML → Markdown 标记 → emoji → 标点归一化。
 * 返回空字符串意味着这段内容**完全由 emoji/代码块/链接组成**，不应朗读。
 */
export function sanitizeForTts(text: string): string {
  if (!text) return ''
  let s = text
  s = stripFencedCodeBlocks(s)
  s = stripUrls(s)
  s = stripHtmlTags(s)
  s = stripMarkdownTokens(s)
  s = stripEmoji(s)
  s = normalizePunctuation(s)
  // 纯标点 / 纯空白 → 视为空
  if (!/[\p{L}\p{N}]/u.test(s)) return ''
  return s
}
