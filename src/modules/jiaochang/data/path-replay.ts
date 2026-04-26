import type {
  JiaochangAgent,
  JiaochangPathReplay,
  JiaochangPathReplayStep,
  JiaochangRunEvent,
  JiaochangViewModel,
} from './jiaochang-types.ts'

const DEFAULT_EVENT_POSITIONS = [
  { x: 48, y: 45 },
  { x: 32, y: 38 },
  { x: 60, y: 58 },
  { x: 42, y: 64 },
  { x: 70, y: 42 },
]

export function createPathReplayFromEvents(
  runId: string,
  events: JiaochangRunEvent[],
  agents: JiaochangAgent[],
): JiaochangPathReplay {
  const agentById = new Map(agents.map((agent) => [agent.id, agent]))
  const steps = events.map<JiaochangPathReplayStep>((event, index) => {
    const agent = agentById.get(event.agentId) ?? agents[index % Math.max(agents.length, 1)]
    const position = agent?.position ?? DEFAULT_EVENT_POSITIONS[index % DEFAULT_EVENT_POSITIONS.length]
    const status = eventStatus(event)

    return {
      id: `replay-${event.id}`,
      ts: event.ts,
      agentId: event.agentId,
      status,
      title: event.title,
      position: offsetPosition(position, index),
      motion: {
        isMoving: event.type === 'tool' || event.type === 'file',
        direction: index % 2 === 0 ? 'south-east' : 'north-west',
      },
      anchor: event.anchor,
    }
  })

  return { runId, steps }
}

export function applyReplayStepToAgents(
  agents: JiaochangAgent[],
  step?: JiaochangPathReplayStep,
): JiaochangAgent[] {
  if (!step) return agents

  return agents.map((agent) => {
    if (agent.id !== step.agentId) return agent

    return {
      ...agent,
      status: step.status,
      statusText: step.title,
      position: step.position,
      motion: step.motion ?? agent.motion,
    }
  })
}

export function getReplayStepById(
  viewModel: JiaochangViewModel,
  stepId: string | null,
): JiaochangPathReplayStep | undefined {
  if (!stepId) return undefined
  return viewModel.pathReplay.steps.find((step) => step.id === stepId)
}

function eventStatus(event: JiaochangRunEvent): JiaochangPathReplayStep['status'] {
  if (event.type === 'thought') return 'planning'
  if (event.type === 'review') return 'researching'
  if (event.type === 'tool') return event.severity === 'error' ? 'blocked' : 'executing'
  if (event.type === 'file') return 'writing'
  if (event.type === 'error') return 'blocked'
  if (event.type === 'done') return 'done'
  return 'idle'
}

function offsetPosition(position: { x: number; y: number }, index: number) {
  const xOffset = (index % 3 - 1) * 4
  const yOffset = (index % 2) * 5
  return {
    x: clamp(position.x + xOffset, 12, 88),
    y: clamp(position.y + yOffset, 14, 86),
  }
}

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max)
}
