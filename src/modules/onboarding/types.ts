/**
 * Onboarding TypeScript type definitions.
 *
 * Mirrors the Rust backend types from modules/onboarding/state.rs
 * and modules/config/types.rs for Tauri IPC communication.
 */

// ── App State Machine ────────────────────────────────────────────────────────

/** Application global state — drives whether Onboarding or Main App renders. */
export type AppState =
  | { type: 'FirstLaunch' }
  | { type: 'Onboarding'; step: number }
  | { type: 'Ready' };

/** Backend onboarding state (persistent). */
export interface OnboardingState {
  onboarding_completed: boolean;
  current_step: number;
  completed_steps: number[];
}

// ── System Check ─────────────────────────────────────────────────────────────

export type CheckStatus =
  | { status: 'Pass' }
  | { status: 'Fail'; reason: string }
  | { status: 'Running' }
  | { status: 'Pending' };

export interface CpuInfo {
  architecture: string;
  cores: number;
  status: CheckStatus;
}

export interface GpuInfo {
  available: boolean;
  name: string | null;
  status: CheckStatus;
}

export interface MemoryInfo {
  total_mb: number;
  available_mb: number;
  status: CheckStatus;
}

export interface EmbeddedModelStatus {
  downloaded: boolean;
  progress: number | null;
  size_mb: number;
  status: CheckStatus;
}

export interface SystemReport {
  cpu: CpuInfo;
  gpu: GpuInfo;
  memory: MemoryInfo;
  embedded_model: EmbeddedModelStatus;
  overall: CheckStatus;
}

// ── Provider ─────────────────────────────────────────────────────────────────

export type ProviderCategory = 'Domestic' | 'International' | 'Local' | 'Custom';

export type ProviderStatus =
  | { status: 'Available' }
  | { status: 'ApiKeyRequired' }
  | { status: 'Unavailable'; reason: string };

export interface Provider {
  id: string;
  name: string;
  category: ProviderCategory;
  status: ProviderStatus;
  supports_models: boolean;
  is_local: boolean;
  logo_path: string | null;
}

export type ModelModality = 'Text' | 'Vision' | 'Multimodal';

export interface Model {
  id: string;
  name: string;
  context_window: number | null;
  max_tokens: number | null;
  modality: ModelModality;
  /** P-MULTI-API — model exposes reasoning / chain-of-thought stream. */
  reasoning?: boolean;
  /** P-MULTI-API — assistant tool_call history rows must carry
   * `reasoning_content` (Kimi-thinking-preview / DeepSeek-R1). */
  reasoning_required_in_tool_calls?: boolean;
  /** P-MULTI-API — model accepts top-level `reasoning_effort`
   * (low/medium/high). OpenAI o1/o3/GPT-5 family. */
  supports_reasoning_effort?: boolean;
}

/** P-MULTI-API — wire format strings, mirror openhanako-main + Rust enum. */
export type ApiType =
  | 'openai-completions'
  | 'anthropic-messages'
  | 'openai-responses'
  | 'openai-codex-responses';

export const API_TYPE_OPTIONS: { value: ApiType; label: string }[] = [
  { value: 'openai-completions', label: 'OpenAI Compatible' },
  { value: 'anthropic-messages', label: 'Anthropic Messages' },
  { value: 'openai-responses', label: 'OpenAI Responses' },
  { value: 'openai-codex-responses', label: 'ChatGPT Codex (Plus/Pro)' },
];

/** Service category for the Settings sidebar (OAuth / Coding Plan / API). */
export type ServiceCategory = 'oauth' | 'coding-plan' | 'api';

export interface ProviderConfig {
  provider_id: string;
  api_key: string | null;
  base_url: string | null;
  display_name: string;
}

export interface ModelSelection {
  provider_id: string;
  model_id: string;
}

// ── Channel ──────────────────────────────────────────────────────────────────

export type ChannelCategory = 'Social' | 'Messaging' | 'Desktop';

export interface Channel {
  id: string;
  name: string;
  category: ChannelCategory;
  icon: string;
  logo_path: string | null;
  requires_token: boolean;
  requires_secret: boolean;
  requires_webhook: boolean;
  node_version_required: string | null;
}

export interface ChannelConfig {
  channel_id: string;
  bot_token: string | null;
  app_secret: string | null;
  webhook_url: string | null;
  display_name: string;
}

/** Redacted channel config — safe to return to frontend. */
export interface ChannelConfigRedacted {
  channel_id: string;
  display_name: string;
  has_bot_token: boolean;
  has_app_secret: boolean;
  has_access_token: boolean;
  webhook_url: string | null;
  bot_username: string | null;
  app_id: string | null;
  phone_number_id: string | null;
  last_test_result: TestResult | null;
}

// ── Test Result ──────────────────────────────────────────────────────────────

export interface TestResult {
  success: boolean;
  message: string;
  latency_ms: number | null;
  details: string | null;
}

// ── Activation ───────────────────────────────────────────────────────────────

export interface ActivationChecklist {
  system_check: boolean;
  security_confirmed: boolean;
  provider_configured: boolean;
  channels_configured: boolean;
}

export interface ActivationResult {
  success: boolean;
  session_id: string | null;
  message: string;
  /** First response from the configured LLM. */
  ai_response?: string | null;
}

// ── AppConfig ────────────────────────────────────────────────────────────────

export interface AppConfig {
  active_provider: ProviderConfig | null;
  active_model: ModelSelection | null;
  channels: ChannelConfig[];
  onboarding: OnboardingState;
  security_confirmed: boolean;
}

// ── useOnboarding Hook Return ────────────────────────────────────────────────

export interface UseOnboardingReturn {
  // State
  appState: AppState;
  currentStep: number;
  completedSteps: number[];

  // System check
  systemReport: SystemReport | null;
  isChecking: boolean;
  downloadProgress: number;
  downloadedBytes: number;
  totalBytes: number;
  downloadError: string | null;
  isDownloading: boolean;

  // Provider
  providers: Provider[];
  selectedProvider: Provider | null;
  availableModels: Model[];
  selectedModel: Model | null;
  providerTestResult: TestResult | null;
  modelTestResult: TestResult | null;

  // Channel
  channels: Channel[];
  configuredChannels: ChannelConfigRedacted[];
  channelTestResult: Record<string, TestResult>;

  // Activation
  activationChecklist: ActivationChecklist | null;
  activationResult: ActivationResult | null;
  loadActivationChecklist: () => Promise<void>;
  loadAppState: () => Promise<void>;

  // Operations
  nextStep: () => Promise<void>;
  prevStep: () => Promise<void>;
  runSystemCheck: () => Promise<void>;
  downloadEmbeddedModel: () => Promise<void>;
  confirmSecurity: () => Promise<void>;
  configureProvider: (config: ProviderConfig) => Promise<void>;
  configureProviderWithModels: (config: ProviderConfig, modelIds: string[]) => Promise<void>;
  getConfiguredModels: (providerId: string) => Promise<string[]>;
  getConfiguredProviders: () => Promise<string[]>;
  getProviderConfig: (providerId: string) => Promise<ProviderConfig | null>;
  getAllConfiguredModels: () => Promise<Map<string, string[]>>;
  testProvider: (config: ProviderConfig) => Promise<void>;
  selectModel: (modelId: string) => Promise<void>;
  testModel: (modelId: string) => Promise<void>;
  loadModels: (providerId: string, baseUrl: string, apiKey: string | null) => Promise<Model[]>;
  configureChannel: (config: ChannelConfig) => Promise<void>;
  testChannel: (config: ChannelConfig) => Promise<void>;
  wakeAgent: () => Promise<void>;
}

// ── Tauri Event Payloads (ADR-014 Section 18.6) ──────────────────────────────

/** Event payloads pushed from backend to frontend. */
export interface OnboardingEvents {
  'onboarding://step_changed': {
    from_step: number;
    to_step: number;
  };
  'onboarding://download_progress': {
    model_name: string;
    downloaded_bytes: number;
    total_bytes: number;
    percent: number;
  };
  'onboarding://test_completed': {
    test_type: 'provider' | 'channel' | 'model';
    target_id: string;
    success: boolean;
    latency_ms?: number;
    error?: string;
  };
  'onboarding://error': {
    step: number;
    code: string;
    message: string;
    retriable: boolean;
  };
}
