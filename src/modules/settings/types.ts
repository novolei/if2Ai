import type { LucideIcon } from "lucide-react";

export type SettingsSectionId =
  | "general"
  | "agent-identity"
  | "usage"
  | "skills"
  | "tools"
  | "web-search"
  | "memory"
  | "model"
  | "connections"
  | "remote"
  | "tts-settings"
  | "tts-profiles"
  | "tts-test"
  | "stt-config"
  | "strategy-diagnostics"
  | "prompt-diagnostics"
  | "about";

export type ThemeMode = "system" | "light" | "dark";
export type FontMode = "serif" | "sans";

export interface SettingsSectionMeta {
  id: SettingsSectionId;
  label: string;
  description: string;
  icon: LucideIcon;
}

export interface SettingsState {
  theme: ThemeMode;
  fontMode: FontMode;
  language: string;
  startupMode: string;
  density: string;
  autoScroll: boolean;
  notifications: boolean;
  username: string;
  email: string;
  apiKey: string;
  baseUrl: string;
}

export interface SettingsActions {
  setTheme: (value: ThemeMode) => void;
  setFontMode: (value: FontMode) => void;
  setLanguage: (value: string) => void;
  setStartupMode: (value: string) => void;
  setDensity: (value: string) => void;
  setAutoScroll: (value: boolean) => void;
  setNotifications: (value: boolean) => void;
  setUsername: (value: string) => void;
  setEmail: (value: string) => void;
  setApiKey: (value: string) => void;
  setBaseUrl: (value: string) => void;
}

export interface SettingsPageProps {
  state: SettingsState;
  actions: SettingsActions;
}
