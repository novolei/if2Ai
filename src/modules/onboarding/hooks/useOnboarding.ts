/**
 * Onboarding state management hook.
 *
 * Wraps Tauri invoke calls and event listeners to provide a unified
 * interface for all Onboarding step pages.
 *
 * State is driven by the backend — this hook does NOT make step
 * transition decisions locally.
 */

import { useState, useCallback, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import type {
  AppState,
  Provider,
  Model,
  TestResult,
  SystemReport,
  Channel,
  ChannelConfigRedacted,
  ChannelConfig,
  ProviderConfig,
  ActivationChecklist,
  ActivationResult,
  UseOnboardingReturn,
} from '../types';

/** Map backend AppState enum to frontend state. */
function parseAppState(raw: unknown): AppState {
  const obj = raw as Record<string, unknown>;
  if ('FirstLaunch' in obj) return { type: 'FirstLaunch' };
  if ('Ready' in obj) return { type: 'Ready' };
  if ('Onboarding' in obj) {
    const stepObj = obj['Onboarding'] as Record<string, unknown>;
    return { type: 'Onboarding', step: (stepObj.step as number) ?? 1 };
  }
  return { type: 'FirstLaunch' };
}

/** Convert ProviderConfig to the shape expected by Tauri commands. */
function providerConfigToTauri(config: ProviderConfig): Record<string, unknown> {
  return {
    provider_id: config.provider_id,
    api_key: config.api_key,
    base_url: config.base_url,
    display_name: config.display_name,
  };
}

/** Convert ChannelConfig to the shape expected by Tauri commands. */
function channelConfigToTauri(config: ChannelConfig): Record<string, unknown> {
  return {
    channel_id: config.channel_id,
    bot_token: config.bot_token,
    app_secret: config.app_secret,
    webhook_url: config.webhook_url,
    display_name: config.display_name,
  };
}

export function useOnboarding(): UseOnboardingReturn {
  const [appState, setAppState] = useState<AppState>({ type: 'FirstLaunch' });
  const [systemReport, setSystemReport] = useState<SystemReport | null>(null);
  const [isChecking, setIsChecking] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [isDownloading, setIsDownloading] = useState(false);
  const [providers, setProviders] = useState<Provider[]>([]);
  const [selectedProvider, _setSelectedProvider] = useState<Provider | null>(null);
  const [availableModels, _setAvailableModels] = useState<Model[]>([]);
  const [selectedModel, setSelectedModel] = useState<Model | null>(null);
  const [providerTestResult, setProviderTestResult] = useState<TestResult | null>(null);
  const [modelTestResult, setModelTestResult] = useState<TestResult | null>(null);
  const [channels, setChannels] = useState<Channel[]>([]);
  const [configuredChannels, setConfiguredChannels] = useState<ChannelConfigRedacted[]>([]);
  const [channelTestResult, setChannelTestResult] = useState<Record<string, TestResult>>({});
  const [activationChecklist, _setActivationChecklist] = useState<ActivationChecklist | null>(null);
  const [activationResult, setActivationResult] = useState<ActivationResult | null>(null);

  // Prevent duplicate event listener registration
  const listenersSetRef = useRef(false);

  // Load initial app state
  const loadAppState = useCallback(async () => {
    try {
      const raw = await invoke<unknown>('onboarding_get_state');
      setAppState(parseAppState(raw));
    } catch (err) {
      console.error('[onboarding] Failed to load app state:', err);
    }
  }, []);

  // Load provider list
  const loadProviders = useCallback(async () => {
    try {
      const list = await invoke<Provider[]>('provider_list');
      setProviders(list);
    } catch (err) {
      console.error('[onboarding] Failed to load providers:', err);
    }
  }, []);

  // Load channel list
  const loadChannels = useCallback(async () => {
    try {
      const list = await invoke<Channel[]>('channel_list');
      setChannels(list);
    } catch (err) {
      console.error('[onboarding] Failed to load channels:', err);
    }
  }, []);

  // Load configured channels
  const loadConfiguredChannels = useCallback(async () => {
    try {
      const list = await invoke<ChannelConfigRedacted[]>('channel_list_configured');
      setConfiguredChannels(list);
    } catch {
      // No configured channels yet — OK
    }
  }, []);

  // ── Operations ──────────────────────────────────────────────────────────────

  const nextStep = useCallback(async () => {
    await invoke<unknown>('onboarding_next_step');
    await loadAppState();
  }, [loadAppState]);

  const prevStep = useCallback(async () => {
    await invoke<unknown>('onboarding_prev_step');
    await loadAppState();
  }, [loadAppState]);

  const runSystemCheck = useCallback(async () => {
    setIsChecking(true);
    try {
      const report = await invoke<SystemReport>('system_check_run');
      setSystemReport(report);
    } catch (err) {
      console.error('[onboarding] System check failed:', err);
    } finally {
      setIsChecking(false);
    }
  }, []);

  const downloadEmbeddedModel = useCallback(async () => {
    setIsDownloading(true);
    setDownloadProgress(0);
    try {
      await invoke('embedded_model_download');
    } catch (err) {
      console.error('[onboarding] Model download failed:', err);
    } finally {
      setIsDownloading(false);
    }
  }, []);

  const confirmSecurity = useCallback(async () => {
    await invoke('security_confirm');
    await nextStep();
  }, [nextStep]);

  const configureProvider = useCallback(async (config: ProviderConfig) => {
    await invoke('provider_configure', { config: providerConfigToTauri(config) });
  }, []);

  const testProvider = useCallback(async (config: ProviderConfig) => {
    const result = await invoke<TestResult>('provider_test', {
      config: providerConfigToTauri(config),
    });
    setProviderTestResult(result);
  }, []);

  const selectModel = useCallback(async (modelId: string) => {
    if (!selectedProvider) return;
    await invoke('model_select', {
      provider_id: selectedProvider.id,
      model_id: modelId,
    });
    setSelectedModel({
      id: modelId,
      name: modelId,
      context_window: null,
      max_tokens: null,
      modality: 'Text',
    });
  }, [selectedProvider]);

  const testModel = useCallback(async (modelId: string) => {
    if (!selectedProvider) return;
    const result = await invoke<TestResult>('model_test', {
      provider_id: selectedProvider.id,
      model_id: modelId,
    });
    setModelTestResult(result);
  }, [selectedProvider]);

  const configureChannel = useCallback(async (config: ChannelConfig) => {
    await invoke('channel_configure', { config: channelConfigToTauri(config) });
    await loadConfiguredChannels();
  }, [loadConfiguredChannels]);

  const testChannel = useCallback(async (config: ChannelConfig) => {
    const result = await invoke<TestResult>('channel_test', {
      config: channelConfigToTauri(config),
    });
    setChannelTestResult((prev) => ({ ...prev, [config.channel_id]: result }));
  }, []);

  const wakeAgent = useCallback(async () => {
    const result = await invoke<ActivationResult>('activation_start');
    setActivationResult(result);
    if (result.success) {
      await invoke('activation_complete');
      await loadAppState();
    }
  }, [loadAppState]);

  // ── Initial Load ────────────────────────────────────────────────────────────

  useEffect(() => {
    loadAppState();
    loadProviders();
    loadChannels();
    loadConfiguredChannels();
  }, [loadAppState, loadProviders, loadChannels, loadConfiguredChannels]);

  // ── Event Listeners ─────────────────────────────────────────────────────────

  useEffect(() => {
    if (listenersSetRef.current) return;
    listenersSetRef.current = true;

    const cleanup: UnlistenFn[] = [];

    const setup = async () => {
      try {
        const unlistenStep = await listen('onboarding://step_changed', () => {
          loadAppState();
        });
        cleanup.push(unlistenStep);
      } catch {
        // Events may not be available — OK
      }

      try {
        const unlistenProgress = await listen(
          'onboarding://download_progress',
          (event) => {
            const payload = event.payload as Record<string, unknown>;
            if (typeof payload.percent === 'number') {
              setDownloadProgress(payload.percent);
            }
          },
        );
        cleanup.push(unlistenProgress);
      } catch {
        // Events may not be available — OK
      }
    };

    setup();

    return () => {
      for (const unlisten of cleanup) {
        unlisten();
      }
    };
  }, [loadAppState]);

  // ── Derived State ───────────────────────────────────────────────────────────

  const currentStep = appState.type === 'Onboarding' ? appState.step : 0;
  const completedSteps =
    appState.type === 'Onboarding'
      ? [] // Backend tracks this; frontend derives from appState
      : [];

  return {
    appState,
    currentStep,
    completedSteps,
    systemReport,
    isChecking,
    downloadProgress,
    isDownloading,
    providers,
    selectedProvider,
    availableModels,
    selectedModel,
    providerTestResult,
    modelTestResult,
    channels,
    configuredChannels,
    channelTestResult,
    activationChecklist,
    activationResult,
    nextStep,
    prevStep,
    runSystemCheck,
    downloadEmbeddedModel,
    confirmSecurity,
    configureProvider,
    testProvider,
    selectModel,
    testModel,
    configureChannel,
    testChannel,
    wakeAgent,
  };
}
