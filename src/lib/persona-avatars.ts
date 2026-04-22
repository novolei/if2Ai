/**
 * Shared persona avatar registry.
 *
 * Built-in PNG avatars are imported here so any UI surface (settings,
 * sidebar, telemetry, future @-mention picker, etc.) can render the
 * exact same artwork. The map is keyed by the canonical persona id
 * registered in `src-tauri/src/modules/identity/registry.rs`.
 *
 * Future personas only need to:
 *   1. Drop a 1024×512 (or square) PNG into `src/assets/personas/`
 *   2. Add one line to `PERSONA_AVATAR_BY_ID` below
 *
 * Personas without a mapped avatar fall back to the procedural SVG
 * `PersonaPortrait` so the UI never shows a broken image.
 */

import staffArchitectAvatar from "@/assets/personas/staff-architect.png";
import executionPartnerAvatar from "@/assets/personas/execution-partner.png";
import quietStrategistAvatar from "@/assets/personas/quiet-strategist.png";
import warmCompanionAvatar from "@/assets/personas/warm-companion.png";
import sharpAnalystAvatar from "@/assets/personas/sharp-analyst.png";
import creativeMuseAvatar from "@/assets/personas/creative-muse.png";

export const PERSONA_AVATAR_BY_ID: Record<string, string> = {
  "staff-architect": staffArchitectAvatar,
  "execution-partner": executionPartnerAvatar,
  // Reserved for future personas; ids are placeholders the user can pick when
  // adding new personas via the identity pack import flow.
  "quiet-strategist": quietStrategistAvatar,
  "warm-companion": warmCompanionAvatar,
  "sharp-analyst": sharpAnalystAvatar,
  "creative-muse": creativeMuseAvatar,
};

/**
 * Ordered library of all bundled avatars, useful for "pick an avatar"
 * UIs (e.g. when the user creates a custom persona).
 */
export const PERSONA_AVATAR_LIBRARY: Array<{
  id: string;
  src: string;
  label: string;
  vibe: string;
}> = [
  {
    id: "staff-architect",
    src: staffArchitectAvatar,
    label: "Staff Architect",
    vibe: "克制 / 精准 / 沉稳",
  },
  {
    id: "execution-partner",
    src: executionPartnerAvatar,
    label: "Execution Partner",
    vibe: "直接 / 务实 / 推力强",
  },
  {
    id: "quiet-strategist",
    src: quietStrategistAvatar,
    label: "Quiet Strategist",
    vibe: "沉静 / 长线 / 思辨",
  },
  {
    id: "warm-companion",
    src: warmCompanionAvatar,
    label: "Warm Companion",
    vibe: "温柔 / 共情 / 陪伴",
  },
  {
    id: "sharp-analyst",
    src: sharpAnalystAvatar,
    label: "Sharp Analyst",
    vibe: "锐利 / 数据 / 拆解",
  },
  {
    id: "creative-muse",
    src: creativeMuseAvatar,
    label: "Creative Muse",
    vibe: "灵感 / 跳跃 / 想象",
  },
];

/** Get the avatar src for a persona id, or null if no bundled avatar exists. */
export function getPersonaAvatarSrc(personaId: string | null | undefined): string | null {
  if (!personaId) return null;
  return PERSONA_AVATAR_BY_ID[personaId] ?? null;
}
