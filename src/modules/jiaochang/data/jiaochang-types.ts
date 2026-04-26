export type JiaochangDataSource = 'projection' | 'fixture'

export type JiaochangAgentStatus =
  | 'idle'
  | 'planning'
  | 'researching'
  | 'executing'
  | 'writing'
  | 'syncing'
  | 'blocked'
  | 'done'

export type JiaochangAgentRole = 'main' | 'subagent' | 'reviewer' | 'tool'

export type JiaochangRuntimeAnchorKind = 'chat' | 'tool' | 'file' | 'event'

export interface JiaochangRuntimeAnchor {
  kind: JiaochangRuntimeAnchorKind
  label: string
  sessionId?: string
  messageId?: string
  runId?: string
  eventId?: string
  toolCallId?: string
  filePath?: string
  line?: number
}

export interface JiaochangSubagentIdentity {
  agentId: string
  displayName: string
  role: JiaochangAgentRole
  parentAgentId?: string
  model?: string
  sessionId?: string
  runId?: string
  capabilities?: string[]
}

export interface JiaochangRunProgress {
  runId: string
  phase: JiaochangAgentStatus
  percent?: number
  startedAt?: string
  updatedAt?: string
  etaSeconds?: number
  currentStep?: string
  totalSteps?: number
  completedSteps?: number
}

export interface JiaochangAgent {
  id: string
  name: string
  role: JiaochangAgentRole
  projectionIdentity?: JiaochangSubagentIdentity
  avatarKey: string
  status: JiaochangAgentStatus
  statusText: string
  currentTask?: string
  lastEventAt?: string
  progress?: number
  runProgress?: JiaochangRunProgress
  lane?: string
  motion?: {
    isMoving: boolean
    direction:
      | 'south'
      | 'south-east'
      | 'east'
      | 'north-east'
      | 'north'
      | 'north-west'
      | 'west'
      | 'south-west'
  }
  position: { x: number; y: number }
}

export interface JiaochangRunEvent {
  id: string
  ts: string
  agentId: string
  type: 'thought' | 'tool' | 'file' | 'error' | 'review' | 'done'
  title: string
  detail?: string
  severity?: 'info' | 'warning' | 'error'
  anchor?: JiaochangRuntimeAnchor
}

export interface JiaochangToolLedgerEntry {
  id: string
  runId: string
  agentId: string
  toolName: string
  status: 'queued' | 'running' | 'succeeded' | 'failed' | 'cancelled'
  startedAt?: string
  completedAt?: string
  durationMs?: number
  summary?: string
  evidenceRef?: {
    sessionId?: string
    messageId?: string
    toolCallId?: string
    filePath?: string
  }
  errorCode?: string
}

export interface JiaochangAdapterHealth {
  source: JiaochangDataSource
  status: 'ready' | 'empty' | 'degraded'
  detail: string
  lastError?: string
}

export interface JiaochangPathReplayStep {
  id: string
  ts: string
  agentId: string
  status: JiaochangAgentStatus
  title: string
  position: { x: number; y: number }
  motion?: JiaochangAgent['motion']
  anchor?: JiaochangRuntimeAnchor
}

export interface JiaochangPathReplay {
  runId: string
  steps: JiaochangPathReplayStep[]
}

export interface JiaochangViewModel {
  source: JiaochangDataSource
  title: string
  runId: string
  runProgress: JiaochangRunProgress
  agents: JiaochangAgent[]
  events: JiaochangRunEvent[]
  toolLedger: JiaochangToolLedgerEntry[]
  pathReplay: JiaochangPathReplay
  updatedAt: string
  adapterHealth: JiaochangAdapterHealth
}
