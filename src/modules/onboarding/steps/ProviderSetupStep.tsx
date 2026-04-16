/**
 * ProviderSetupStep — Step 4 of the 6-step Onboarding flow.
 *
 * Displays a grid of 14 provider cards. Users select a provider,
 * configure API credentials, test connection, and choose a model.
 *
 * Design reference: docs/references/onboarding-steps/ProviderSetup.png
 */

import { useState, useCallback, useEffect } from 'react';
import { Loader2, Check, X } from 'lucide-react';
import { cn } from '@/lib/utils';
import { InfoPanel, InfoCard } from '../components/InfoPanel';
import { OnboardingLayout } from '../components/OnboardingLayout';
import { StepHeader } from '../components/StepHeader';
import { StepNavigation } from '../components/StepNavigation';
import { ProviderCard } from '../components/ProviderCard';
import { ModelSelector } from '../components/ModelSelector';
import { useOnboarding } from '../hooks/useOnboarding';
import type { Provider, ProviderConfig } from '../types';

interface ProviderSetupStepProps {
  onNext: () => void;
  onPrev: () => void;
}

/** Built-in provider list with display metadata. */
const BUILTIN_PROVIDERS: Provider[] = [
  // Domestic sources
  { id: 'zai', name: 'Z.AI (GLM)', category: 'Domestic', status: { status: 'ApiKeyRequired' }, supports_models: true, is_local: false, logo_path: null },
  { id: 'moonshot', name: 'Moonshot (Ximi)', category: 'Domestic', status: { status: 'ApiKeyRequired' }, supports_models: true, is_local: false, logo_path: 'src/assets/ProviderLogos/provider_logo_moonshot.png' },
  { id: 'qwen', name: 'Qwen (阿里云)', category: 'Domestic', status: { status: 'ApiKeyRequired' }, supports_models: true, is_local: false, logo_path: 'src/assets/ProviderLogos/provider_logo_aliyun.png' },
  { id: 'minimax', name: 'MiniMax', category: 'Domestic', status: { status: 'ApiKeyRequired' }, supports_models: true, is_local: false, logo_path: 'src/assets/ProviderLogos/provider_logo_minimax.png' },
  { id: 'qianfan', name: 'Qianfan (百度)', category: 'Domestic', status: { status: 'ApiKeyRequired' }, supports_models: true, is_local: false, logo_path: 'src/assets/ProviderLogos/provider_logo_baidu.png' },
  { id: 'xiaomi', name: 'Xiaomi AI', category: 'Domestic', status: { status: 'ApiKeyRequired' }, supports_models: true, is_local: false, logo_path: 'src/assets/ProviderLogos/provider_logo_xiaomi.png' },
  { id: 'volcengine', name: 'Volcano Engine', category: 'Domestic', status: { status: 'ApiKeyRequired' }, supports_models: true, is_local: false, logo_path: 'src/assets/ProviderLogos/provider_logo_volcengine.png' },
  { id: 'byteplus', name: 'BytePlus', category: 'Domestic', status: { status: 'ApiKeyRequired' }, supports_models: true, is_local: false, logo_path: 'src/assets/ProviderLogos/provider_logo_byteplus.png' },
  { id: 'ollama', name: 'Ollama (本地)', category: 'Local', status: { status: 'Available' }, supports_models: true, is_local: true, logo_path: 'src/assets/ProviderLogos/provider_logo_ollama.png' },
  // International sources
  { id: 'openai', name: 'OpenAI', category: 'International', status: { status: 'ApiKeyRequired' }, supports_models: true, is_local: false, logo_path: 'src/assets/ProviderLogos/provider_logo_openai.png' },
  { id: 'anthropic', name: 'Anthropic (Claude)', category: 'International', status: { status: 'ApiKeyRequired' }, supports_models: true, is_local: false, logo_path: 'src/assets/ProviderLogos/provider_logo_anthropic.png' },
  { id: 'google', name: 'Google (Gemini)', category: 'International', status: { status: 'ApiKeyRequired' }, supports_models: true, is_local: false, logo_path: 'src/assets/ProviderLogos/provider_logo_google.png' },
  { id: 'openrouter', name: 'OpenRouter', category: 'International', status: { status: 'ApiKeyRequired' }, supports_models: true, is_local: false, logo_path: 'src/assets/ProviderLogos/provider_logo_openrouter.png' },
  // Custom
  { id: 'custom', name: 'Custom (自定义)', category: 'Custom', status: { status: 'Available' }, supports_models: true, is_local: false, logo_path: null },
];

/** Default base URLs per provider. */
const DEFAULT_BASE_URLS: Record<string, string> = {
  ollama: 'http://localhost:11434',
  openai: 'https://api.openai.com/v1',
  anthropic: 'https://api.anthropic.com',
  google: 'https://generativelanguage.googleapis.com',
  openrouter: 'https://openrouter.ai/api/v1',
  zai: 'https://api.z.ai/v1',
  qwen: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
};

export function ProviderSetupStep({ onNext, onPrev }: ProviderSetupStepProps) {
  const {
    providers: backendProviders,
    availableModels,
    selectedModel,
    testProvider,
    selectModel,
  } = useOnboarding();

  // Use backend providers if available, fall back to built-in list
  const providers = backendProviders.length > 0 ? backendProviders : BUILTIN_PROVIDERS;

  // Local UI state
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [apiKey, setApiKey] = useState('');
  const [baseUrl, setBaseUrl] = useState('');
  const [isTesting, setIsTesting] = useState(false);
  const [testPassed, setTestPassed] = useState(false);
  const [testError, setTestError] = useState<string | null>(null);
  const [modelsLoaded, setModelsLoaded] = useState(false);

  const selectedProv = providers.find((p) => p.id === selectedId);
  const isLocalProvider = selectedProv?.is_local ?? false;
  const needsApiKey = selectedProv?.status.status === 'ApiKeyRequired';

  // Reset form when selection changes
  useEffect(() => {
    setShowForm(false);
    setApiKey('');
    setBaseUrl(DEFAULT_BASE_URLS[selectedId ?? ''] ?? '');
    setIsTesting(false);
    setTestPassed(false);
    setTestError(null);
    setModelsLoaded(false);
  }, [selectedId]);

  const handleProviderClick = (providerId: string) => {
    setSelectedId(providerId);
    setShowForm(true);
  };

  const handleTestConnection = useCallback(async () => {
    if (needsApiKey && !apiKey.trim()) {
      setTestError('请输入 API Key');
      return;
    }

    setIsTesting(true);
    setTestError(null);
    setTestPassed(false);

    try {
      const config: ProviderConfig = {
        provider_id: selectedId ?? '',
        api_key: apiKey || null,
        base_url: baseUrl || null,
        display_name: selectedProv?.name ?? '',
      };
      await testProvider(config);
      setTestPassed(true);
    } catch (err) {
      setTestError(err instanceof Error ? err.message : '连接测试失败');
    } finally {
      setIsTesting(false);
    }
  }, [selectedId, selectedProv, apiKey, baseUrl, needsApiKey, testProvider]);

  const handleSelectModel = useCallback(
    async (modelId: string) => {
      await selectModel(modelId);
      setModelsLoaded(true);
    },
    [selectModel],
  );

  const canProceed = testPassed && modelsLoaded;

  return (
    <OnboardingLayout
      rightPanel={
        <InfoPanel
          stepLabel="STEP 4: 模型服务商"
          title="选定默认模型来源。"
          bullets={[
            '选择后可随时在设置中更改',
            '支持 14 个主流服务商',
            '测试通过后再选模型',
          ]}
        >
          {/* Selected provider details */}
          {selectedProv && (
            <div className="mt-4">
              <InfoCard className="flex flex-col gap-2">
                <div className="flex items-center gap-2">
                  <div className="flex h-8 w-8 items-center justify-center rounded-md border border-border bg-white">
                    <span className="text-token-sm font-bold text-brand-orange">
                      {selectedProv.name[0]}
                    </span>
                  </div>
                  <div className="min-w-0">
                    <span className="text-token-sm font-semibold text-white">
                      {selectedProv.name}
                    </span>
                    {testPassed && (
                      <div className="flex items-center gap-1">
                        <Check className="h-3 w-3 text-status-success" />
                        <span className="text-token-xs text-status-success">
                          已连接
                        </span>
                      </div>
                    )}
                  </div>
                </div>

                {/* Model selector in right panel */}
                {testPassed && availableModels.length > 0 && (
                  <div className="mt-2">
                    <span className="text-token-xs text-muted-foreground mb-1 block">
                      可用模型
                    </span>
                    <ModelSelector
                      models={availableModels}
                      selectedModelId={selectedModel?.id ?? null}
                      onSelect={handleSelectModel}
                    />
                  </div>
                )}
              </InfoCard>
            </div>
          )}
        </InfoPanel>
      }
    >
      <StepHeader currentStep={4} title="选定默认模型来源" />

      {/* Left content area */}
      <div className="flex-1 overflow-y-auto px-8 pb-4">
        <p className="text-token-sm text-muted-foreground mb-4 leading-relaxed">
          选择后可随时在"设置 &gt; 模型"中更改或添加新服务商。
        </p>

        {/* Provider grid */}
        <div className="grid grid-cols-4 gap-2.5 mb-5">
          {providers.map((provider) => (
            <ProviderCard
              key={provider.id}
              provider={provider}
              isSelected={selectedId === provider.id}
              isConfigured={testPassed && selectedId === provider.id}
              onClick={() => handleProviderClick(provider.id)}
            />
          ))}
        </div>

        {/* Config form for selected provider */}
        {showForm && selectedProv && (
          <div className="rounded-lg border border-border bg-muted/20 px-4 py-4">
            <h3 className="text-token-sm font-semibold text-foreground mb-3">
              配置 {selectedProv.name}
            </h3>

            <div className="flex flex-col gap-3">
              {/* Base URL */}
              <div>
                <label className="text-token-xs text-muted-foreground mb-1 block">
                  Base URL
                </label>
                <input
                  type="text"
                  value={baseUrl}
                  onChange={(e) => setBaseUrl(e.target.value)}
                  placeholder={
                    isLocalProvider
                      ? 'http://localhost:11434'
                      : 'https://api.example.com/v1'
                  }
                  className="w-full rounded-md border border-border bg-input px-3 py-2 text-token-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-brand-orange"
                />
              </div>

              {/* API Key (skip for local providers) */}
              {needsApiKey && (
                <div>
                  <label className="text-token-xs text-muted-foreground mb-1 block">
                    API Key
                  </label>
                  <input
                    type="password"
                    value={apiKey}
                    onChange={(e) => setApiKey(e.target.value)}
                    placeholder="sk-..."
                    className="w-full rounded-md border border-border bg-input px-3 py-2 text-token-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-brand-orange"
                  />
                </div>
              )}

              {/* Test button */}
              <button
                type="button"
                onClick={handleTestConnection}
                disabled={isTesting || (needsApiKey && !apiKey.trim())}
                className={cn(
                  'flex items-center justify-center gap-2 rounded-md px-4 py-2 text-token-sm font-medium transition-colors',
                  testPassed
                    ? 'bg-status-success text-white'
                    : 'bg-brand-orange text-white hover:bg-brand-orange-dark',
                  'disabled:bg-brand-orange/50 disabled:text-white/60 disabled:cursor-not-allowed',
                )}
              >
                {isTesting ? (
                  <>
                    <Loader2 className="h-4 w-4 animate-spin" />
                    测试连接中...
                  </>
                ) : testPassed ? (
                  <>
                    <Check className="h-4 w-4" />
                    连接成功 — 请在右侧选择模型
                  </>
                ) : (
                  '测试连接'
                )}
              </button>

              {/* Error message */}
              {testError && (
                <div className="flex items-center gap-2 text-token-xs text-status-error">
                  <X className="h-3.5 w-3.5 shrink-0" />
                  <span>{testError}</span>
                  <button
                    type="button"
                    onClick={handleTestConnection}
                    className="text-brand-orange hover:underline ml-auto"
                  >
                    重试
                  </button>
                </div>
              )}
            </div>
          </div>
        )}
      </div>

      {/* Bottom navigation */}
      <StepNavigation
        currentStep={4}
        onNext={onNext}
        onPrev={onPrev}
        canGoNext={canProceed}
        canGoPrev
        nextLabel="再配置 →"
      />
    </OnboardingLayout>
  );
}
