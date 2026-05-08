import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const source = readFileSync(new URL('../ErrorCard.tsx', import.meta.url), 'utf8')

test('ErrorCard — exports the component and the classifier helper privately', () => {
  assert.match(source, /export function ErrorCard\(/)
  assert.match(source, /function classifyError\(/)
  assert.match(source, /function classifyErrorWithOutcome\(/)
})

test('ErrorCard — preserves the legacy classifier branches', () => {
  assert.match(source, /任务部分完成/)
  assert.match(source, /模型流超时/)
  assert.match(source, /权限受限/)
  assert.match(source, /请求格式不兼容/)
  assert.match(source, /网络传输中断/)
  assert.match(source, /模型流异常/)
  assert.match(source, /认证失败/)
  assert.match(source, /请求频率受限/)
  assert.match(source, /服务端异常/)
  assert.match(source, /缺少 API 配置/)
  assert.match(source, /Agent 执行异常/)
})

test('ErrorCard — wires retry + resume button copy', () => {
  assert.match(source, />\s*重试\s*</)
  assert.match(source, />\s*继续未完成任务\s*</)
  assert.match(source, /import \{ truncateText \} from '\.\.\/utils\/text'/)
})
