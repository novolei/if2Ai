import { enUS } from './en-US.ts'
import { jaJP } from './ja-JP.ts'
import { koKR } from './ko-KR.ts'
import type { JiaochangI18nCatalog, JiaochangI18nKey, JiaochangLocale } from './locales.ts'
import { JIACHANG_LOCALES, normalizeJiaochangLocale } from './locales.ts'
import { zhCN } from './zh-CN.ts'

export { JIACHANG_LOCALES, normalizeJiaochangLocale }
export type { JiaochangI18nCatalog, JiaochangI18nKey, JiaochangLocale }

export const JIACHANG_CATALOGS: Record<JiaochangLocale, JiaochangI18nCatalog> = {
  'zh-CN': zhCN,
  'en-US': enUS,
  'ja-JP': jaJP,
  'ko-KR': koKR,
}

export function getJiaochangCatalog(locale?: string | null): JiaochangI18nCatalog {
  return JIACHANG_CATALOGS[normalizeJiaochangLocale(locale)]
}

export function createJiaochangTranslator(locale?: string | null) {
  const catalog = getJiaochangCatalog(locale)
  const fallback = JIACHANG_CATALOGS['en-US']
  return (key: JiaochangI18nKey) => catalog[key] ?? fallback[key] ?? key
}
