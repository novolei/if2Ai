import stageBackground from '@/assets/jiaochang/backgrounds/shendiao-courtyard.webp'
import agentCraftsman from '@/assets/jiaochang/sprites/agent-craftsman-idle.png'
import agentCraftsmanSleepy01 from '@/assets/jiaochang/sprites/agent-craftsman-idle-sleepy-01.png'
import agentCraftsmanSleepy02 from '@/assets/jiaochang/sprites/agent-craftsman-idle-sleepy-02.png'
import agentCraftsmanSleepy03 from '@/assets/jiaochang/sprites/agent-craftsman-idle-sleepy-03.png'
import agentCraftsmanSleepy04 from '@/assets/jiaochang/sprites/agent-craftsman-idle-sleepy-04.png'
import agentScholar from '@/assets/jiaochang/sprites/agent-scholar-idle.png'
import agentScholarSleepy01 from '@/assets/jiaochang/sprites/agent-scholar-idle-sleepy-01.png'
import agentScholarSleepy02 from '@/assets/jiaochang/sprites/agent-scholar-idle-sleepy-02.png'
import agentScholarSleepy03 from '@/assets/jiaochang/sprites/agent-scholar-idle-sleepy-03.png'
import agentScholarSleepy04 from '@/assets/jiaochang/sprites/agent-scholar-idle-sleepy-04.png'
import agentSwordsman from '@/assets/jiaochang/sprites/agent-swordsman-idle.png'
import agentSwordsmanSleepy01 from '@/assets/jiaochang/sprites/agent-swordsman-idle-sleepy-01.png'
import agentSwordsmanSleepy02 from '@/assets/jiaochang/sprites/agent-swordsman-idle-sleepy-02.png'
import agentSwordsmanSleepy03 from '@/assets/jiaochang/sprites/agent-swordsman-idle-sleepy-03.png'
import agentSwordsmanSleepy04 from '@/assets/jiaochang/sprites/agent-swordsman-idle-sleepy-04.png'
import craftsmanMoveNorthWest from '@/assets/jiaochang/sprites/frames/craftsman/move-north-west.png'
import craftsmanMoveSouthEast from '@/assets/jiaochang/sprites/frames/craftsman/move-south-east.png'
import craftsmanStatusBlocked from '@/assets/jiaochang/sprites/frames/craftsman/status-blocked.png'
import craftsmanStatusDone from '@/assets/jiaochang/sprites/frames/craftsman/status-done.png'
import craftsmanStatusExecuting from '@/assets/jiaochang/sprites/frames/craftsman/status-executing.png'
import craftsmanStatusPlanning from '@/assets/jiaochang/sprites/frames/craftsman/status-planning.png'
import craftsmanStatusResearching from '@/assets/jiaochang/sprites/frames/craftsman/status-researching.png'
import craftsmanStatusSyncing from '@/assets/jiaochang/sprites/frames/craftsman/status-syncing.png'
import craftsmanStatusWriting from '@/assets/jiaochang/sprites/frames/craftsman/status-writing.png'
import scholarMoveNorthWest from '@/assets/jiaochang/sprites/frames/scholar/move-north-west.png'
import scholarMoveSouthEast from '@/assets/jiaochang/sprites/frames/scholar/move-south-east.png'
import scholarStatusBlocked from '@/assets/jiaochang/sprites/frames/scholar/status-blocked.png'
import scholarStatusDone from '@/assets/jiaochang/sprites/frames/scholar/status-done.png'
import scholarStatusExecuting from '@/assets/jiaochang/sprites/frames/scholar/status-executing.png'
import scholarStatusPlanning from '@/assets/jiaochang/sprites/frames/scholar/status-planning.png'
import scholarStatusResearching from '@/assets/jiaochang/sprites/frames/scholar/status-researching.png'
import scholarStatusSyncing from '@/assets/jiaochang/sprites/frames/scholar/status-syncing.png'
import scholarStatusWriting from '@/assets/jiaochang/sprites/frames/scholar/status-writing.png'
import swordsmanMoveNorthWest from '@/assets/jiaochang/sprites/frames/swordsman/move-north-west.png'
import swordsmanMoveSouthEast from '@/assets/jiaochang/sprites/frames/swordsman/move-south-east.png'
import swordsmanStatusBlocked from '@/assets/jiaochang/sprites/frames/swordsman/status-blocked.png'
import swordsmanStatusDone from '@/assets/jiaochang/sprites/frames/swordsman/status-done.png'
import swordsmanStatusExecuting from '@/assets/jiaochang/sprites/frames/swordsman/status-executing.png'
import swordsmanStatusPlanning from '@/assets/jiaochang/sprites/frames/swordsman/status-planning.png'
import swordsmanStatusResearching from '@/assets/jiaochang/sprites/frames/swordsman/status-researching.png'
import swordsmanStatusSyncing from '@/assets/jiaochang/sprites/frames/swordsman/status-syncing.png'
import swordsmanStatusWriting from '@/assets/jiaochang/sprites/frames/swordsman/status-writing.png'
import statusBlocked from '@/assets/jiaochang/icons/status-blocked.png'
import statusDone from '@/assets/jiaochang/icons/status-done.png'
import statusExecuting from '@/assets/jiaochang/icons/status-executing.png'
import statusIdle from '@/assets/jiaochang/icons/status-idle.png'
import statusPlanning from '@/assets/jiaochang/icons/status-planning.png'
import statusResearching from '@/assets/jiaochang/icons/status-researching.png'
import statusSyncing from '@/assets/jiaochang/icons/status-syncing.png'
import statusWriting from '@/assets/jiaochang/icons/status-writing.png'

import type { JiaochangAgent, JiaochangAgentStatus, JiaochangDataSource } from '../data/jiaochang-types'
import type { JiaochangPathReplayStep } from '../data/jiaochang-types'
import { applyReplayStepToAgents } from '../data/path-replay'
import type { JiaochangI18nKey } from '../i18n'

type AgentAvatarKey = 'scholar' | 'swordsman' | 'craftsman'

const STATUS_CLASS: Record<JiaochangAgentStatus, string> = {
  idle: 'border-slate-300 bg-slate-100 text-slate-700',
  planning: 'border-amber-300 bg-amber-100 text-amber-800',
  researching: 'border-sky-300 bg-sky-100 text-sky-800',
  executing: 'border-red-300 bg-red-100 text-red-800',
  writing: 'border-violet-300 bg-violet-100 text-violet-800',
  syncing: 'border-cyan-300 bg-cyan-100 text-cyan-800',
  blocked: 'border-rose-400 bg-rose-100 text-rose-900',
  done: 'border-emerald-300 bg-emerald-100 text-emerald-800',
}

const SPRITE_SRC: Record<string, string> = {
  scholar: agentScholar,
  swordsman: agentSwordsman,
  craftsman: agentCraftsman,
}

const STATUS_SPRITE_SRC: Record<AgentAvatarKey, Partial<Record<JiaochangAgentStatus, string>>> = {
  scholar: {
    planning: scholarStatusPlanning,
    researching: scholarStatusResearching,
    executing: scholarStatusExecuting,
    writing: scholarStatusWriting,
    syncing: scholarStatusSyncing,
    blocked: scholarStatusBlocked,
    done: scholarStatusDone,
  },
  swordsman: {
    planning: swordsmanStatusPlanning,
    researching: swordsmanStatusResearching,
    executing: swordsmanStatusExecuting,
    writing: swordsmanStatusWriting,
    syncing: swordsmanStatusSyncing,
    blocked: swordsmanStatusBlocked,
    done: swordsmanStatusDone,
  },
  craftsman: {
    planning: craftsmanStatusPlanning,
    researching: craftsmanStatusResearching,
    executing: craftsmanStatusExecuting,
    writing: craftsmanStatusWriting,
    syncing: craftsmanStatusSyncing,
    blocked: craftsmanStatusBlocked,
    done: craftsmanStatusDone,
  },
}

const MOVE_SPRITE_SRC: Record<AgentAvatarKey, Partial<Record<NonNullable<JiaochangAgent['motion']>['direction'], string>>> = {
  scholar: {
    'south-east': scholarMoveSouthEast,
    'north-west': scholarMoveNorthWest,
  },
  swordsman: {
    'south-east': swordsmanMoveSouthEast,
    'north-west': swordsmanMoveNorthWest,
  },
  craftsman: {
    'south-east': craftsmanMoveSouthEast,
    'north-west': craftsmanMoveNorthWest,
  },
}

const IDLE_SPRITE_SRC: Record<string, string[]> = {
  scholar: [
    agentScholar,
    agentScholarSleepy01,
    agentScholarSleepy02,
    agentScholarSleepy03,
    agentScholarSleepy04,
  ],
  swordsman: [
    agentSwordsman,
    agentSwordsmanSleepy01,
    agentSwordsmanSleepy02,
    agentSwordsmanSleepy03,
    agentSwordsmanSleepy04,
  ],
  craftsman: [
    agentCraftsman,
    agentCraftsmanSleepy01,
    agentCraftsmanSleepy02,
    agentCraftsmanSleepy03,
    agentCraftsmanSleepy04,
  ],
}

const STATUS_ICON_SRC: Record<JiaochangAgentStatus, string> = {
  idle: statusIdle,
  planning: statusPlanning,
  researching: statusResearching,
  executing: statusExecuting,
  writing: statusWriting,
  syncing: statusSyncing,
  blocked: statusBlocked,
  done: statusDone,
}

function stableSpriteIndex(seed: string, modulo: number) {
  let hash = 0
  for (let index = 0; index < seed.length; index += 1) {
    hash = (hash * 31 + seed.charCodeAt(index)) >>> 0
  }
  return hash % modulo
}

function resolveAgentSprite(agent: JiaochangAgent) {
  const avatarKey = isAgentAvatarKey(agent.avatarKey) ? agent.avatarKey : 'scholar'

  if (agent.motion?.isMoving) {
    return MOVE_SPRITE_SRC[avatarKey][agent.motion.direction]
      ?? STATUS_SPRITE_SRC[avatarKey][agent.status]
      ?? SPRITE_SRC[avatarKey]
  }

  if (agent.status !== 'idle') {
    return STATUS_SPRITE_SRC[avatarKey][agent.status] ?? SPRITE_SRC[avatarKey]
  }

  const idleSprites = IDLE_SPRITE_SRC[avatarKey] ?? IDLE_SPRITE_SRC.scholar
  return idleSprites[stableSpriteIndex(agent.id, idleSprites.length)] ?? agentScholar
}

function isAgentAvatarKey(value: string): value is AgentAvatarKey {
  return value === 'scholar' || value === 'swordsman' || value === 'craftsman'
}

export function PixelStage({
  agents,
  source,
  replayStep,
  t,
}: {
  agents: JiaochangAgent[]
  source: JiaochangDataSource
  replayStep?: JiaochangPathReplayStep
  t: (key: JiaochangI18nKey) => string
}) {
  const projectedAgents = applyReplayStepToAgents(agents, replayStep)

  return (
    <section className="relative min-h-[420px] overflow-hidden rounded-[8px] border border-[var(--jiaochang-border,rgba(43,34,24,0.1))] bg-[var(--jiaochang-stage-bg,#bfe9cf)] shadow-[var(--shadow-lg)]">
      <img
        src={stageBackground}
        alt=""
        className="absolute inset-0 h-full w-full object-cover [image-rendering:pixelated]"
        draggable={false}
      />
      <div className="absolute inset-0 bg-[linear-gradient(180deg,rgba(255,255,255,0.06),rgba(255,255,255,0)_42%,rgba(43,34,24,0.08))] opacity-80" />

      <div
        className="font-jiaochang-pixel absolute left-[5%] top-[6%] rounded-full border border-[var(--jiaochang-border,rgba(255,255,255,0.7))] bg-[var(--jiaochang-panel-bg,rgba(255,255,255,0.7))] px-3 py-1 text-[12px] font-semibold text-[var(--jiaochang-text,#2f4c3a)] shadow-sm"
        title={t(source === 'fixture' ? 'stage.source.fixture' : 'stage.source.projection')}
      >
        {t(source === 'fixture' ? 'stage.theme.fixture' : 'stage.theme.projection')}
      </div>

      {replayStep ? (
        <div
          className="absolute z-[9] h-4 w-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white bg-[#b9462f] shadow-[0_0_0_4px_rgba(185,70,47,0.22)]"
          style={{ left: `${replayStep.position.x}%`, top: `${replayStep.position.y}%` }}
          title={replayStep.title}
        />
      ) : null}

      {projectedAgents.map((agent) => (
        <div
          key={agent.id}
          className="absolute z-10 -translate-x-1/2 -translate-y-1/2"
          style={{ left: `${agent.position.x}%`, top: `${agent.position.y}%` }}
        >
          <div className="flex flex-col items-center gap-1">
            <img
              src={resolveAgentSprite(agent)}
              alt=""
              className="h-16 w-12 drop-shadow-[4px_5px_0_rgba(0,0,0,0.18)] [image-rendering:pixelated]"
              draggable={false}
            />
            <div className={`font-jiaochang-pixel flex items-center gap-1 whitespace-nowrap rounded-full border px-2 py-0.5 text-[11px] font-semibold shadow-sm ${STATUS_CLASS[agent.status]}`}>
              <img
                src={STATUS_ICON_SRC[agent.status]}
                alt=""
                className="h-4 w-4 [image-rendering:pixelated]"
                draggable={false}
              />
              {t(`role.${agent.role}`)} · {t(`status.${agent.status}`)}
            </div>
          </div>
        </div>
      ))}
    </section>
  )
}
