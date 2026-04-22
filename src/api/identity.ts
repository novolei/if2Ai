// FEAT-ID-003 — identity domain facade.
//
// Wraps the existing prompt-control defaults/catalog commands plus
// the new session-level identity override command so UI layers do
// not need to reach into `@/lib/tauri` directly.

import {
  getIdentityCustomizationPack,
  getPromptControlCatalog,
  getPromptControlSettings,
  setIdentityCustomizationPack,
  setPromptControlSettings,
  type IdentityCustomizationPack,
  type PersonaCustomization,
  type PromptControlCatalog,
  type PromptControlPersonaOption,
  type PromptControlSettings,
  type PromptControlSettingsInput,
  type PromptControlSoulOption,
  type SessionIdentityInput,
  type SoulCustomization,
} from "@/lib/tauri";

import { setSessionIdentity, type SessionMeta } from "./sessions";

export type {
  PromptControlCatalog,
  PromptControlSettings,
  PromptControlSettingsInput,
  SessionIdentityInput,
  IdentityCustomizationPack,
  SoulCustomization,
  PersonaCustomization,
  PromptControlSoulOption,
  PromptControlPersonaOption,
};

/** Load the built-in identity catalog (Souls + Personas). */
export async function getIdentityCatalog(): Promise<PromptControlCatalog> {
  return getPromptControlCatalog();
}

/** Load the saved global default identity settings. */
export async function getIdentityDefaults(): Promise<PromptControlSettings> {
  return getPromptControlSettings();
}

/** Load the editable custom identity pack. */
export async function getIdentityPack(): Promise<IdentityCustomizationPack> {
  return getIdentityCustomizationPack();
}

/** Persist the editable custom identity pack. */
export async function setIdentityPack(
  pack: IdentityCustomizationPack,
): Promise<IdentityCustomizationPack> {
  return setIdentityCustomizationPack(pack);
}

/** Persist global default identity settings. */
export async function setIdentityDefaults(
  request: PromptControlSettingsInput,
): Promise<PromptControlSettings> {
  return setPromptControlSettings(request);
}

/** Persist the current session's identity override. */
export async function updateSessionIdentity(
  id: string,
  identity: SessionIdentityInput,
): Promise<SessionMeta> {
  return setSessionIdentity(id, identity);
}
