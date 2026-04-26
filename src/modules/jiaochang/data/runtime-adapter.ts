import type {
  JiaochangAgent,
  JiaochangRunEvent,
  JiaochangRunProgress,
  JiaochangSubagentIdentity,
  JiaochangToolLedgerEntry,
  JiaochangViewModel,
  JiaochangPathReplay,
} from './jiaochang-types.ts'
import { createPathReplayFromEvents } from './path-replay.ts'

export interface JiaochangProjectionSnapshot {
  title?: string
  runId: string
  updatedAt?: string
  subagentIdentities: JiaochangSubagentIdentity[]
  toolLedger: JiaochangToolLedgerEntry[]
  runProgress: JiaochangRunProgress
  events: JiaochangRunEvent[]
  pathReplay?: JiaochangPathReplay
}

const POSITIONS = [
  { x: 48, y: 45 },
  { x: 28, y: 34 },
  { x: 66, y: 62 },
  { x: 72, y: 35 },
  { x: 36, y: 66 },
]

const ROLE_AVATAR: Record<JiaochangSubagentIdentity['role'], string> = {
  main: 'scholar',
  subagent: 'scholar',
  reviewer: 'swordsman',
  tool: 'craftsman',
}

const ROLE_LANE: Record<JiaochangSubagentIdentity['role'], string> = {
  main: '演武台',
  subagent: '偏殿',
  reviewer: '藏经阁',
  tool: '工坊',
}

export function createJiaochangProjectionViewModel(
  snapshot: JiaochangProjectionSnapshot,
): JiaochangViewModel {
  const agents = snapshot.subagentIdentities.map<JiaochangAgent>((identity, index) => ({
    id: identity.agentId,
    name: identity.displayName,
    role: identity.role,
    projectionIdentity: identity,
    avatarKey: ROLE_AVATAR[identity.role],
    status: snapshot.runProgress.phase,
    statusText: statusText(snapshot.runProgress.phase),
    currentTask: snapshot.runProgress.currentStep,
    lastEventAt: snapshot.updatedAt,
    progress: snapshot.runProgress.percent,
    runProgress: snapshot.runProgress,
    lane: ROLE_LANE[identity.role],
    motion: {
      isMoving: snapshot.runProgress.phase === 'executing' || snapshot.runProgress.phase === 'syncing',
      direction: identity.role === 'tool' ? 'north-west' : 'south-east',
    },
    position: POSITIONS[index % POSITIONS.length],
  }))

  return {
    source: 'projection',
    title: snapshot.title ?? '校场运行',
    runId: snapshot.runId,
    runProgress: snapshot.runProgress,
    agents,
    events: snapshot.events,
    toolLedger: snapshot.toolLedger,
    pathReplay: snapshot.pathReplay ?? createPathReplayFromEvents(snapshot.runId, snapshot.events, agents),
    updatedAt: snapshot.updatedAt ?? snapshot.runProgress.updatedAt ?? new Date(0).toISOString(),
    adapterHealth: {
      source: 'projection',
      status: agents.length > 0 ? 'ready' : 'empty',
      detail:
        agents.length > 0
          ? '已从 runtime projection 构建校场视图。'
          : 'runtime projection 暂无 subagent identity。',
    },
  }
}

function statusText(status: JiaochangRunProgress['phase']) {
  if (status === 'planning') return '谋定'
  if (status === 'researching') return '查招'
  if (status === 'executing') return '出招中'
  if (status === 'writing') return '落卷'
  if (status === 'syncing') return '归档'
  if (status === 'blocked') return '受阻'
  if (status === 'done') return '收招'
  return '待命'
}
