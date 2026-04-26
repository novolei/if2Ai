import { strict as assert } from 'node:assert'
import { describe, it } from 'node:test'

import { JIACHANG_CATALOGS, JIACHANG_LOCALES, createJiaochangTranslator } from './index.ts'

describe('jiaochang i18n catalog', () => {
  it('keeps zh-CN/en-US/ja-JP/ko-KR key sets aligned', () => {
    const baseKeys = Object.keys(JIACHANG_CATALOGS['zh-CN']).sort()
    for (const locale of JIACHANG_LOCALES) {
      assert.deepEqual(Object.keys(JIACHANG_CATALOGS[locale]).sort(), baseKeys)
    }
  })

  it('normalizes locale and returns translated runtime labels', () => {
    assert.equal(createJiaochangTranslator('en')('status.blocked'), 'Blocked')
    assert.equal(createJiaochangTranslator('ja-JP')('strategy.title'), '戦略提案')
    assert.equal(createJiaochangTranslator('ko-KR')('panel.toolLedger'), '도구 장부')
  })
})
