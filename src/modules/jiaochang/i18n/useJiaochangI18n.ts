import { useMemo } from 'react'

import { createJiaochangTranslator, normalizeJiaochangLocale } from './index.ts'

export function useJiaochangI18n() {
  const locale =
    typeof navigator === 'undefined'
      ? 'zh-CN'
      : normalizeJiaochangLocale(navigator.language)

  const t = useMemo(() => createJiaochangTranslator(locale), [locale])

  return { locale, t }
}
