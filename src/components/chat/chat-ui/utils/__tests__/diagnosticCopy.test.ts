import assert from 'node:assert/strict'
import { test } from 'node:test'

import { buildDiagnosticCopyText } from '@/components/chat/chat-ui/utils/diagnosticCopy'

const baseMessage = {
  id: 'm1',
  role: 'tool' as const,
  content: 'ok',
  timestamp: new Date(0),
}

test('diagnosticCopy — buildDiagnosticCopyText emits stream/trace/request triple', () => {
  const out = buildDiagnosticCopyText({
    ...baseMessage,
    streamId: 'sid-1',
    evidenceId: 'eid-2',
    requestId: 'rid-3',
  } as any)
  assert.equal(out, 'stream_id=sid-1;trace_id=eid-2;request_id=rid-3')
})

test('diagnosticCopy — returns null when any identifier missing', () => {
  assert.equal(buildDiagnosticCopyText(baseMessage as any), null)
  assert.equal(
    buildDiagnosticCopyText({
      ...baseMessage,
      streamId: 'sid',
      evidenceId: 'eid',
    } as any),
    null,
  )
})
