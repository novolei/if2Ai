/**
 * ProviderSetupStep — Step 4 of the 6-step Onboarding flow.
 *
 * Displays provider cards in two sections (国内推荐 / 国际主流).
 * Clicking a provider card opens its configuration form in the RIGHT panel.
 * For Ollama local, auto-connects and shows multi-select model list.
 *
 * Design: Unified white card with large rounded corners, semi-transparent
 * model rows with right-side checkboxes, confirm button at bottom.
 */

import { useState, useCallback, useEffect, useRef } from 'react';
import { Check, Loader2, X, Circle, Eye, EyeOff, Shield } from 'lucide-react';
import { cn } from '@/lib/utils';
import { OnboardingLayout } from '../components/OnboardingLayout';
import { StepNavigation } from '../components/StepNavigation';
import { StepProgressBar } from '../components/StepProgressBar';
import { RightPanelHeader } from '../components/RightPanelHeader';
import { useOnboarding } from '../hooks/useOnboarding';
import type { ProviderConfig, Model } from '../types';

// Inject closing animation keyframe once
if (typeof document !== 'undefined' && !document.getElementById('onboarding-slide-out')) {
  const style = document.createElement('style');
  style.id = 'onboarding-slide-out';
  style.textContent = `
    @keyframes slideFadeOut {
      0% { opacity: 1; transform: translateX(0); }
      100% { opacity: 0; transform: translateX(24px); }
    }
  `;
  document.head.appendChild(style);
}

interface ProviderSetupStepProps {
  onNext: () => void;
  onPrev: () => void;
  onWindowDrag?: (event: React.MouseEvent<HTMLElement>) => void;
}

/** Badge type for a provider card. */
type CardBadge = 'recommended' | 'redirect' | 'local' | null;

/** Card data for display rendering. */
interface ProviderCardData {
  id: string;
  displayName: string;
  subtitle: string;
  logoLetter: string;
  logoColor: string;
  badge: CardBadge;
  isLocal: boolean;
  needsApiKey: boolean;
  logoAsset: string | null;
  supportsModels: boolean;
  subChoices?: Array<{
    id: string;
    label: string;
    description?: string;
    credentialLabel: string;
    credentialPlaceholder: string;
    baseUrl: string;
  }>;
}

const DOMESTIC_PROVIDERS: ProviderCardData[] = [
  {
    id: 'zai',
    displayName: 'Z.AI (GLM)',
    subtitle: '国内主流',
    logoLetter: 'Z',
    logoColor: '#4F46E5',
    badge: null,
    isLocal: false,
    needsApiKey: true,
    logoAsset: 'provider_logo_zai.png',
    supportsModels: true,
    subChoices: [
      {
        id: 'zai-coding-global',
        label: 'Coding-Plan-Global',
        description: 'GLM Coding Plan (api.z.ai)',
        credentialLabel: 'Z.AI API Key',
        credentialPlaceholder: 'sk-…',
        baseUrl: 'https://api.z.ai/api/paas/v4',
      },
      {
        id: 'zai-coding-cn',
        label: 'Coding-Plan-CN',
        description: 'GLM Coding Plan（国内）',
        credentialLabel: 'Z.AI API Key',
        credentialPlaceholder: 'sk-…',
        baseUrl: 'https://open.bigmodel.cn/api/paas/v4',
      },
      {
        id: 'zai-global',
        label: 'Global',
        description: 'GLM Global (api.z.ai)',
        credentialLabel: 'Z.AI API Key',
        credentialPlaceholder: 'sk-…',
        baseUrl: 'https://api.z.ai/api/paas/v4',
      },
      {
        id: 'zai-cn',
        label: 'CN',
        description: 'GLM 国内版',
        credentialLabel: 'Z.AI API Key',
        credentialPlaceholder: 'sk-…',
        baseUrl: 'https://open.bigmodel.cn/api/paas/v4',
      },
    ],
  },
  {
    id: 'moonshot',
    displayName: 'Moonshot (Kimi)',
    subtitle: '国内主流',
    logoLetter: 'M',
    logoColor: '#6366F1',
    badge: 'recommended',
    isLocal: false,
    needsApiKey: true,
    logoAsset: 'provider_logo_moonshot.png',
    supportsModels: true,
    subChoices: [
      {
        id: 'moonshot-cn',
        label: 'Kimi API key (.cn)',
        description: '国内版 · api.moonshot.cn',
        credentialLabel: 'Moonshot API Key (.cn)',
        credentialPlaceholder: 'sk-…',
        baseUrl: 'https://api.moonshot.cn/v1',
      },
      {
        id: 'moonshot-global',
        label: 'Kimi API key (.ai)',
        description: '国际版 · api.moonshot.ai',
        credentialLabel: 'Moonshot API Key (.ai)',
        credentialPlaceholder: 'sk-…',
        baseUrl: 'https://api.moonshot.ai/v1',
      },
      {
        id: 'moonshot-code',
        label: 'Kimi Code API key',
        description: 'Code 订阅用户',
        credentialLabel: 'Kimi Code API Key',
        credentialPlaceholder: 'sk-…',
        baseUrl: 'https://api.kimi.com/coding/v1',
      },
    ],
  },
  {
    id: 'qwen',
    displayName: 'Qwen (阿里云)',
    subtitle: '国内主流',
    logoLetter: 'Q',
    logoColor: '#F59E0B',
    badge: null,
    isLocal: false,
    needsApiKey: true,
    logoAsset: 'provider_logo_aliyun.png',
    supportsModels: true,
    subChoices: [
      {
        id: 'qwen-cn',
        label: '国内版（DashScope）',
        description: 'dashscope.aliyuncs.com',
        credentialLabel: 'DashScope API Key',
        credentialPlaceholder: 'sk-…',
        baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
      },
      {
        id: 'qwen-global',
        label: '国际版（DashScope Intl）',
        description: 'dashscope-intl.aliyuncs.com',
        credentialLabel: 'DashScope API Key',
        credentialPlaceholder: 'sk-…',
        baseUrl: 'https://dashscope-intl.aliyuncs.com/compatible-mode/v1',
      },
    ],
  },
  {
    id: 'minimax',
    displayName: 'MiniMax',
    subtitle: '国内/国际',
    logoLetter: 'M',
    logoColor: '#10B981',
    badge: null,
    isLocal: false,
    needsApiKey: true,
    logoAsset: 'provider_logo_minimax.png',
    supportsModels: true,
    subChoices: [
      {
        id: 'minimax-global',
        label: '国际版 API Key',
        description: 'api.minimaxi.com',
        credentialLabel: 'MiniMax API Key',
        credentialPlaceholder: '…',
        baseUrl: 'https://api.minimaxi.com/v1',
      },
      {
        id: 'minimax-cn',
        label: '国内版 API Key',
        description: 'api.minimax.chat',
        credentialLabel: 'MiniMax API Key',
        credentialPlaceholder: '…',
        baseUrl: 'https://api.minimax.chat/v1',
      },
    ],
  },
  {
    id: 'qianfan',
    displayName: 'Qianfan (百度)',
    subtitle: '国内',
    logoLetter: 'Q',
    logoColor: '#2563EB',
    badge: null,
    isLocal: false,
    needsApiKey: true,
    logoAsset: 'provider_logo_baidu.png',
    supportsModels: true,
  },
  {
    id: 'xiaomi',
    displayName: 'Xiaomi AI',
    subtitle: '国内',
    logoLetter: 'X',
    logoColor: '#EF4444',
    badge: null,
    isLocal: false,
    needsApiKey: true,
    logoAsset: 'provider_logo_xiaomi.png',
    supportsModels: true,
  },
  {
    id: 'volcengine',
    displayName: 'Volcano Engine',
    subtitle: '国内',
    logoLetter: 'V',
    logoColor: '#F97316',
    badge: null,
    isLocal: false,
    needsApiKey: true,
    logoAsset: 'provider_logo_volcengine.png',
    supportsModels: true,
  },
  {
    id: 'byteplus',
    displayName: 'BytePlus',
    subtitle: '国内/国际',
    logoLetter: 'B',
    logoColor: '#3B82F6',
    badge: null,
    isLocal: false,
    needsApiKey: true,
    logoAsset: 'provider_logo_byteplus.png',
    supportsModels: true,
  },
  {
    id: 'deepseek',
    displayName: 'DeepSeek',
    subtitle: '国内主流',
    logoLetter: 'D',
    logoColor: '#2563EB',
    badge: null,
    isLocal: false,
    needsApiKey: true,
    logoAsset: 'provider_logo_deepseek.png',
    supportsModels: true,
    subChoices: [
      {
        id: 'deepseek-official',
        label: 'DeepSeek 官方 API',
        description: '直连 DeepSeek 官方接口，无需中转',
        credentialLabel: 'API Key',
        credentialPlaceholder: 'sk-…',
        baseUrl: 'https://api.deepseek.com/v1',
      },
      {
        id: 'deepseek-siliconflow',
        label: 'SiliconFlow（国内加速）',
        description: '通过 SiliconFlow 调用 DeepSeek，国内网络友好',
        credentialLabel: 'SiliconFlow API Key',
        credentialPlaceholder: 'sk-…',
        baseUrl: 'https://api.siliconflow.cn/v1',
      },
    ],
  },
  {
    id: 'ollama',
    displayName: 'Ollama',
    subtitle: '本地运行',
    logoLetter: 'O',
    logoColor: '#10B981',
    badge: 'local',
    isLocal: true,
    needsApiKey: false,
    logoAsset: 'provider_logo_ollama.png',
    supportsModels: true,
    subChoices: [
      {
        id: 'ollama-default',
        label: 'Ollama 本地（推荐）',
        description: '无需 API Key，自动连接本地服务',
        credentialLabel: '无需 API Key',
        credentialPlaceholder: '（留空即可）',
        baseUrl: 'http://localhost:11434/v1',
      },
    ],
  },
];

const INTERNATIONAL_PROVIDERS: ProviderCardData[] = [
  {
    id: 'openai',
    displayName: 'OpenAI',
    subtitle: '国际主流',
    logoLetter: 'O',
    logoColor: '#10B981',
    badge: null,
    isLocal: false,
    needsApiKey: true,
    logoAsset: 'provider_logo_openai.png',
    supportsModels: true,
  },
  {
    id: 'anthropic',
    displayName: 'Anthropic (Claude)',
    subtitle: '国际主流',
    logoLetter: 'A',
    logoColor: '#F97316',
    badge: null,
    isLocal: false,
    needsApiKey: true,
    logoAsset: 'provider_logo_anthropic.png',
    supportsModels: true,
  },
  {
    id: 'google',
    displayName: 'Google (Gemini)',
    subtitle: '国际主流',
    logoLetter: 'G',
    logoColor: '#4285F4',
    badge: null,
    isLocal: false,
    needsApiKey: true,
    logoAsset: 'provider_logo_google.png',
    supportsModels: true,
  },
  {
    id: 'openrouter',
    displayName: 'OpenRouter',
    subtitle: '国际主流',
    logoLetter: 'O',
    logoColor: '#000000',
    badge: null,
    isLocal: false,
    needsApiKey: true,
    logoAsset: 'provider_logo_openrouter.png',
    supportsModels: true,
  },
  {
    id: 'mistral',
    displayName: 'Mistral',
    subtitle: '国际主流',
    logoLetter: 'M',
    logoColor: '#5B5BD6',
    badge: null,
    isLocal: false,
    needsApiKey: true,
    logoAsset: 'provider_logo_mistral.png',
    supportsModels: true,
  },
  {
    id: 'xai',
    displayName: 'xAI (Grok)',
    subtitle: '国际主流',
    logoLetter: 'X',
    logoColor: '#000000',
    badge: null,
    isLocal: false,
    needsApiKey: true,
    logoAsset: 'provider_logo_xai.png',
    supportsModels: true,
  },
];

const DEFAULT_BASE_URLS: Record<string, string> = {
  ollama: 'http://localhost:11434',
  openai: 'https://api.openai.com/v1',
  anthropic: 'https://api.anthropic.com',
  google: 'https://generativelanguage.googleapis.com',
  openrouter: 'https://openrouter.ai/api/v1',
  mistral: 'https://api.mistral.ai/v1',
  xai: 'https://api.x.ai/v1',
  zai: 'https://api.z.ai/api/paas/v4',
  moonshot: 'https://api.moonshot.cn/v1',
  qwen: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
  minimax: 'https://api.minimaxi.com/v1',
  deepseek: 'https://api.deepseek.com/v1',
  volcengine: 'https://ark.cn-beijing.volces.com/api/v3',
};

/** Render badge for a provider card. */
function CardBadge({ badge }: { badge: CardBadge }) {
  if (!badge) return null;

  switch (badge) {
    case 'recommended':
      return (
        <span className="rounded bg-[#E8733A]/10 px-1.5 py-0.5 text-[10px] font-semibold text-[#E8733A]">
          推荐
        </span>
      );
    case 'redirect':
      return (
        <span className="rounded bg-[#FF4D4F]/10 px-1.5 py-0.5 text-[10px] font-semibold text-[#FF4D4F]">
          Re-Dir
        </span>
      );
    case 'local':
      return (
        <span className="rounded bg-status-success/10 px-1.5 py-0.5 text-[10px] font-semibold text-status-success">
          本地
        </span>
      );
  }
}

/** Single provider card — compact: logo + name on same line, subtitle left-aligned below.
 * Fixed height to prevent text folding from affecting layout. */
function ProviderGridCard({
  data,
  isSelected,
  isConfigured,
  onClick,
}: {
  data: ProviderCardData;
  isSelected: boolean;
  isConfigured: boolean;
  onClick: () => void;
}) {
  const logoUrl = data.logoAsset
    ? new URL(
        `../../../assets/ProviderLogos/${data.logoAsset}`,
        import.meta.url,
      ).href
    : null;

  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        'group relative flex items-center gap-2.5 rounded-xl border px-3 py-2.5 text-left transition-all duration-200 font-sans',
        isConfigured && !isSelected
          ? 'border-[#10B981]/30 bg-[#10B981]/3 hover:shadow-md hover:border-[#10B981]/40'
          : isSelected
            ? 'border-[#CC4400] bg-[#FF6B4D]/5 shadow-sm'
            : 'border-[#D0D5D8] bg-white hover:shadow-md hover:border-[#CC4400]/30',
      )}
      style={{ minHeight: '3.25rem' }}
    >
      {/* Logo */}
      <div
        className={cn(
          'flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border text-token-xs font-bold transition-all',
          !logoUrl && data.logoColor
            ? 'text-white border-transparent'
            : 'bg-white border-black/5 group-hover:border-black/10',
        )}
        style={!logoUrl ? { backgroundColor: data.logoColor } : undefined}
      >
        {logoUrl ? (
          <img
            src={logoUrl}
            alt={data.displayName}
            className="h-5 w-5 object-contain"
            onError={(e) => {
              (e.target as HTMLImageElement).style.display = 'none';
            }}
          />
        ) : (
          <span style={{ color: 'white' }}>{data.logoLetter}</span>
        )}
      </div>

      {/* Name + subtitle — truncated, single line each */}
      <div className="min-w-0 flex-1 py-0.5">
        <div className="flex items-center gap-1.5">
          <span className="text-token-sm font-semibold text-foreground truncate">
            {data.displayName}
          </span>
          <CardBadge badge={data.badge} />
        </div>
        <span className="text-token-xs text-muted-foreground block mt-0.5 truncate">
          {data.subtitle}
        </span>
      </div>

      {/* Configured badge — top-right corner (green medal) */}
      {isConfigured && !isSelected && (
        <div className="absolute top-1.5 right-1.5 flex h-4 w-4 items-center justify-center rounded-full bg-[#10B981] shadow-sm">
          <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="12" cy="8" r="6" />
            <path d="M15.477 12.89L17 22l-5-3-5 3 1.523-9.11" />
          </svg>
        </div>
      )}

      {/* Selection indicator — top-right corner */}
      {isSelected && (
        <div className="absolute top-1.5 right-1.5 flex h-4 w-4 items-center justify-center rounded-full bg-[#FF6B4D]">
          <Check className="h-2.5 w-2.5 text-white" />
        </div>
      )}
    </button>
  );
}

/** Single model row for multi-select — right-side checkbox. */
function ModelRow({
  model,
  isSelected,
  onToggle,
}: {
  model: Model;
  isSelected: boolean;
  onToggle: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onToggle}
      className={cn(
        'flex items-center justify-between rounded-lg px-3 py-2.5 text-left transition-all w-full group font-sans',
        'hover:bg-white/10 border border-transparent',
        isSelected && 'bg-white/15 border-white/20',
      )}
    >
      {/* Model name */}
      <span className="text-token-sm font-medium text-white truncate">
        {model.name}
      </span>

      {/* Checkbox on the right */}
      <div
        className={cn(
          'flex h-5 w-5 shrink-0 items-center justify-center rounded-full border transition-all',
          isSelected
            ? 'border-white bg-white'
            : 'border-white/30 bg-transparent group-hover:border-white/50',
        )}
      >
        {isSelected && <Check className="h-3 w-3" style={{ color: '#00D178' }} />}
      </div>
    </button>
  );
}

/** Empty state — shown when no provider is selected, positioned at top of right panel. */
function ProviderEmptyState({
  modelPool,
  allProviders,
}: {
  modelPool: Map<string, string[]>;
  allProviders: ProviderCardData[];
}) {
  // If no models configured, show the default empty state
  if (modelPool.size === 0) {
    return (
      <div className="flex flex-col items-center px-8 py-6 text-center">
        {/* Decorative icon */}
        <div className="mb-4 flex h-14 w-14 items-center justify-center rounded-2xl" style={{ background: 'rgba(255,255,255,0.1)' }}>
          <svg
            className="h-7 w-7"
            style={{ color: 'rgba(255,255,255,0.4)' }}
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={1.5}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M5.25 14.25h13.5m-13.5 0a3 3 0 01-3-3m3 3a3 3 0 100 6h13.5a3 3 0 100-6m-16.5-3a3 3 0 013-3h13.5a3 3 0 013 3m-19.5 0a4.5 4.5 0 01.9-2.7L5.737 5.1a3.375 3.375 0 012.7-1.35h7.126c1.062 0 2.062.5 2.7 1.35l2.587 3.45a4.5 4.5 0 01.9 2.7m0 0a3 3 0 01-3 3m0 3h.008v.008h-.008v-.008zm0-6h.008v.008h-.008v-.008zm-3 6h.008v.008h-.008v-.008zm0-6h.008v.008h-.008v-.008z"
            />
          </svg>
        </div>

        <h3 className="font-sans text-[15px] font-semibold text-white mb-1.5">
          选择模型服务商
        </h3>
        <p className="font-sans text-[13px]" style={{ color: 'rgba(255,255,255,0.5)' }}>
          点击左侧卡片开始配置
        </p>
      </div>
    );
  }

  // Show configured providers summary
  const totalModels = Array.from(modelPool.values()).reduce(
    (sum, ids) => sum + ids.length,
    0,
  );

  return (
    <div className="flex flex-col h-full px-3 py-4 overflow-y-auto">
      {/* ═══ Summary Card ═══ */}
      <div className="mx-3 mt-3 rounded-2xl p-4" style={{ background: 'rgba(255, 255, 255, 0.12)', backdropFilter: 'blur(8px)', border: '1px solid rgba(255, 255, 255, 0.15)' }}>
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl" style={{ background: 'rgba(16, 185, 129, 0.15)' }}>
            <Check className="h-5 w-5 text-[#10B981]" />
          </div>
          <div>
            <h3 className="text-token-sm font-semibold text-white">
              已配置模型
            </h3>
            <p className="text-token-xs" style={{ color: 'rgba(255,255,255,0.6)' }}>
              共 {totalModels} 个模型，来自 {modelPool.size} 个服务商
            </p>
          </div>
        </div>
      </div>

      {/* ═══ Provider Summary List ═══ */}
      <div className="flex-1 px-3 py-4">
        {Array.from(modelPool.entries()).map(([provId, modelIds]) => {
          const provData = allProviders.find((p) => p.id === provId);
          const provName = provData?.displayName ?? provId;
          const logoUrl = provData?.logoAsset
            ? new URL(`../../../assets/ProviderLogos/${provData.logoAsset}`, import.meta.url).href
            : null;

          return (
            <div
              key={provId}
              className="flex items-start gap-3 rounded-xl px-4 py-3 mb-2"
              style={{ background: 'rgba(255, 255, 255, 0.06)', border: '1px solid rgba(255, 255, 255, 0.1)' }}
            >
              {/* Provider icon */}
              <div
                className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-token-xs font-bold"
                style={!logoUrl ? { backgroundColor: provData?.logoColor ?? '#6B7280', color: 'white' } : undefined}
              >
                {logoUrl ? (
                  <img src={logoUrl} alt={provName} className="h-4 w-4 object-contain" onError={(e) => { (e.target as HTMLImageElement).style.display = 'none'; }} />
                ) : (
                  (provData?.displayName ?? provId)[0]
                )}
              </div>

              {/* Provider name + model count */}
              <div className="flex-1 min-w-0">
                <span className="text-token-sm font-medium text-white">
                  {provName}
                </span>
                <span className="text-token-xs ml-2" style={{ color: 'rgba(255,255,255,0.5)' }}>
                  ({modelIds.length} 个模型)
                </span>

                {/* Model tags */}
                <div className="flex flex-wrap gap-1.5 mt-2">
                  {modelIds.slice(0, 4).map((mid) => (
                    <span
                      key={mid}
                      className="rounded-md px-2 py-0.5 text-token-xs font-medium"
                      style={{ background: 'rgba(255,255,255,0.08)', color: 'rgba(255,255,255,0.7)' }}
                    >
                      {mid}
                    </span>
                  ))}
                  {modelIds.length > 4 && (
                    <span className="text-token-xs" style={{ color: 'rgba(255,255,255,0.4)' }}>
                      +{modelIds.length - 4}
                    </span>
                  )}
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

/** Right panel: Provider configuration with model multi-select. */
function ProviderConfigPanel({
  provider,
  onClose,
  onProviderConfigured,
}: {
  provider: ProviderCardData;
  onClose: () => void;
  onProviderConfigured: (providerId: string) => void;
}) {
  const { testProvider, loadModels, configureProviderWithModels, getConfiguredModels, getProviderConfig } =
    useOnboarding();

  const [selectedSubChoice, setSelectedSubChoice] = useState<string | null>(
    provider.subChoices?.[0]?.id ?? null,
  );
  const [apiKey, setApiKey] = useState('');
  const [baseUrl, setBaseUrl] = useState(
    provider.subChoices?.[0]?.baseUrl ??
      DEFAULT_BASE_URLS[provider.id] ??
      '',
  );
  const [isTesting, setIsTesting] = useState(false);
  const [testPassed, setTestPassed] = useState(false);
  const [testError, setTestError] = useState<string | null>(null);
  const [loadingModels, setLoadingModels] = useState(false);
  const [modelError, setModelError] = useState<string | null>(null);
  const [models, setModels] = useState<Model[]>([]);
  const [selectedModelIds, setSelectedModelIds] = useState<Set<string>>(new Set());
  const [isSaving, setIsSaving] = useState(false);
  const [justSaved, setJustSaved] = useState(false);
  const [showApiKey, setShowApiKey] = useState(false);
  const [isShaking, setIsShaking] = useState(false);
  const [showShieldHint, setShowShieldHint] = useState(false);

  const currentSubChoice = provider.subChoices?.[0] ?? null;
  const isOllama = provider.id === 'ollama';
  const needsApiKey = provider.needsApiKey;

  // Track which provider we've already auto-connected for (avoids stale ref issues)
  const autoConnectedProvider = useRef<string | null>(null);
  // Store previously configured model IDs to pre-select after models load
  const prevConfiguredModelIds = useRef<string[]>([]);

  // Reset when provider changes — handles Ollama auto-connect inline
  useEffect(() => {
    // Clear auto-connect tracking when switching away from Ollama
    if (!isOllama) {
      autoConnectedProvider.current = null;
    }

    let cancelled = false;

    const loadPrevAndReset = async () => {
      // Load previously configured models first
      const prevIds = await getConfiguredModels(provider.id);
      if (cancelled) return;
      prevConfiguredModelIds.current = prevIds;

      // Reset all state
      setSelectedSubChoice(provider.subChoices?.[0]?.id ?? null);
      setApiKey('');
      const url = provider.subChoices?.[0]?.baseUrl ??
        DEFAULT_BASE_URLS[provider.id] ??
        '';
      setBaseUrl(url);
      setIsTesting(false);
      setTestPassed(false);
      setTestError(null);
      setLoadingModels(false);
      setModelError(null);
      setModels([]);
      setSelectedModelIds(new Set());
      setIsSaving(false);
      setJustSaved(false);
      setShowApiKey(false);
      setIsShaking(false);
      setShowShieldHint(false);

      // Try to load saved config for already-configured providers
      const savedConfig = await getProviderConfig(provider.id);
      if (cancelled) return;
      if (savedConfig?.base_url && savedConfig?.api_key) {
        // Restore saved credentials
        setBaseUrl(savedConfig.base_url);
        setApiKey(savedConfig.api_key);

        // Auto-test and fetch models without requiring user input
        setTestPassed(true);
        setLoadingModels(true);
        setModelError(null);
        try {
          const loadedModels = await loadModels(provider.id, savedConfig.base_url, savedConfig.api_key);
          if (cancelled) return;
          setModels(loadedModels);
          const prevIds2 = new Set(prevConfiguredModelIds.current);
          const availableIds = new Set(loadedModels.map((m: Model) => m.id));
          const matchingIds = [...prevIds2].filter((id) => availableIds.has(id));
          if (matchingIds.length > 0) {
            setSelectedModelIds(new Set(matchingIds));
          }
        } catch (err) {
          if (cancelled) return;
          setModelError(
            err instanceof Error ? err.message : '获取模型列表失败',
          );
        } finally {
          if (!cancelled) setLoadingModels(false);
        }
      } else if (isOllama && autoConnectedProvider.current !== provider.id) {
        console.log('[ollama] Starting auto-connect with url:', url);
        autoConnectedProvider.current = provider.id;
        try {
          await testProvider({
            provider_id: provider.id,
            api_key: null,
            base_url: url,
            display_name: provider.displayName,
          });
          if (cancelled) return;
          console.log('[ollama] Connection successful, loading models...');
          setTestPassed(true);

          // Fetch models after successful connection
          if (provider.supportsModels && url) {
            setLoadingModels(true);
            setModelError(null);
            try {
              const loadedModels = await loadModels(provider.id, url, null);
              if (cancelled) return;
              console.log('[ollama] Loaded', loadedModels.length, 'models');
              setModels(loadedModels);
              const prevIds2 = new Set(prevConfiguredModelIds.current);
              const availableIds = new Set(loadedModels.map((m: Model) => m.id));
              const matchingIds = [...prevIds2].filter((id) => availableIds.has(id));
              if (matchingIds.length > 0) {
                setSelectedModelIds(new Set(matchingIds));
              }
            } catch (err) {
              if (cancelled) return;
              setModelError(
                err instanceof Error ? err.message : '获取模型列表失败',
              );
            } finally {
              if (!cancelled) setLoadingModels(false);
            }
          }
        } catch (err) {
          if (cancelled) return;
          console.error('[ollama] Error:', err);
          setTestError(err instanceof Error ? err.message : '连接测试失败');
        } finally {
          if (!cancelled) setIsTesting(false);
        }
      }
    };

    loadPrevAndReset();
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [provider.id]);

  // Sub-choice change — reset test state so user can re-test manually
  useEffect(() => {
    if (!provider.subChoices || provider.subChoices.length <= 1) return;
    setTestPassed(false);
    setTestError(null);
    setModels([]);
    setSelectedModelIds(new Set());
  }, [selectedSubChoice, provider.subChoices]);

  // Delayed shield hint — shown when user types API key but doesn't click shield for 3 seconds
  useEffect(() => {
    if (!needsApiKey || testPassed || isTesting) {
      setShowShieldHint(false);
      return;
    }
    if (!apiKey.trim()) {
      setShowShieldHint(false);
      return;
    }
    const timer = setTimeout(() => {
      setShowShieldHint(true);
    }, 3000);
    return () => clearTimeout(timer);
  }, [apiKey, needsApiKey, testPassed, isTesting]);

  const handleTestConnection = useCallback(async () => {
    setShowShieldHint(false);
    setIsTesting(true);
    setTestError(null);
    setTestPassed(false);

    try {
      const config: ProviderConfig = {
        provider_id: provider.id,
        api_key: apiKey || null,
        base_url: baseUrl || null,
        display_name: provider.displayName,
      };
      await testProvider(config);
      setTestPassed(true);

      // Fetch models after successful connection
      if (provider.supportsModels && baseUrl) {
        setLoadingModels(true);
        setModelError(null);
        try {
          const loadedModels = await loadModels(provider.id, baseUrl, apiKey || null);
          setModels(loadedModels);

          // Pre-select previously configured models
          const prevIds = new Set(prevConfiguredModelIds.current);
          const availableIds = new Set(loadedModels.map((m: Model) => m.id));
          const matchingIds = [...prevIds].filter((id) => availableIds.has(id));
          if (matchingIds.length > 0) {
            setSelectedModelIds(new Set(matchingIds));
          }
        } catch (err) {
          setModelError(
            err instanceof Error ? err.message : '获取模型列表失败',
          );
        } finally {
          setLoadingModels(false);
        }
      }
    } catch (err) {
      setTestError(err instanceof Error ? err.message : '连接测试失败');
      // Trigger shake animation
      setIsShaking(true);
      setTimeout(() => setIsShaking(false), 500);
    } finally {
      setIsTesting(false);
    }
  }, [provider, apiKey, baseUrl, testProvider, loadModels]);

  const handleToggleModel = useCallback((modelId: string) => {
    setSelectedModelIds((prev) => {
      const next = new Set(prev);
      if (next.has(modelId)) {
        next.delete(modelId);
      } else {
        next.add(modelId);
      }
      return next;
    });
  }, []);

  const handleConfirm = useCallback(async () => {
    if (selectedModelIds.size === 0) return;

    setIsSaving(true);
    try {
      // Save provider config with all selected models in one call
      const config: ProviderConfig = {
        provider_id: provider.id,
        api_key: apiKey || null,
        base_url: baseUrl || null,
        display_name: provider.displayName,
      };
      await configureProviderWithModels(config, Array.from(selectedModelIds));

      // Show success animation briefly
      setJustSaved(true);

      // Notify parent that this provider is now configured
      onProviderConfigured(provider.id);

      // Close panel after brief animation (don't navigate — multi-provider flow)
      setTimeout(() => {
        setJustSaved(false);
        onClose();
      }, 600);
    } catch (err) {
      console.error('[provider] Failed to save config:', err);
      setIsSaving(false);
    }
  }, [provider, apiKey, baseUrl, selectedModelIds, configureProviderWithModels, onProviderConfigured, onClose]);

  const logoUrl = provider.logoAsset
    ? new URL(
        `../../../assets/ProviderLogos/${provider.logoAsset}`,
        import.meta.url,
      ).href
    : null;

  return (
    <div className="flex flex-col h-full font-sans overflow-y-auto px-3 py-4">
      {/* ═══ Provider Header Card ═══ */}
      <div
        className="mx-3 mt-3 rounded-2xl p-4"
        style={{
          background: 'rgba(255, 255, 255, 0.12)',
          backdropFilter: 'blur(8px)',
          border: '1px solid rgba(255, 255, 255, 0.15)',
        }}
      >
        <div className="flex items-center gap-3">
          {/* Provider icon */}
          <div
            className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border border-white/10 text-token-sm font-bold text-white shadow-sm"
            style={{ backgroundColor: logoUrl ? undefined : provider.logoColor }}
          >
            {logoUrl ? (
              <img
                src={logoUrl}
                alt={provider.displayName}
                className="h-6 w-6 object-contain"
                onError={(e) => {
                  (e.target as HTMLImageElement).style.display = 'none';
                }}
              />
            ) : (
              <span style={{ color: 'white' }}>{provider.logoLetter}</span>
            )}
          </div>

          {/* Provider name + status */}
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <h3 className="text-token-base font-semibold text-white truncate">
                {provider.displayName}
              </h3>
              {testPassed && (
                <span className="flex h-2 w-2 shrink-0 rounded-full bg-[#10B981] shadow-sm" />
              )}
            </div>
            <p className="text-token-xs mt-0.5" style={{ color: 'rgba(255,255,255,0.6)' }}>
              {testPassed
                ? '连接成功，请选择模型'
                : provider.subtitle}
            </p>
          </div>

          {/* Close button */}
          <button
            type="button"
            onClick={onClose}
            className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg transition-colors"
            style={{ color: 'rgba(255,255,255,0.6)' }}
            onMouseEnter={(e) => {
              (e.currentTarget as HTMLButtonElement).style.background = 'rgba(255,255,255,0.1)';
              (e.currentTarget as HTMLButtonElement).style.color = 'rgba(255,255,255,0.9)';
            }}
            onMouseLeave={(e) => {
              (e.currentTarget as HTMLButtonElement).style.background = 'transparent';
              (e.currentTarget as HTMLButtonElement).style.color = 'rgba(255,255,255,0.6)';
            }}
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* ═══ Sub-choices (认证方式) ═══ */}
        {provider.subChoices && provider.subChoices.length > 0 && (
          <div className="mt-4">
            <p className="text-token-xs font-medium mb-2" style={{ color: 'rgba(255,255,255,0.7)' }}>
              认证方式
            </p>
            <div className="flex flex-col gap-1.5">
              {provider.subChoices.map((choice) => {
                const isSelected = selectedSubChoice === choice.id;
                // Extract hostname from baseUrl for display
                const host = (() => {
                  try {
                    return new URL(choice.baseUrl).hostname;
                  } catch {
                    return choice.baseUrl;
                  }
                })();
                return (
                  <button
                    key={choice.id}
                    type="button"
                    onClick={() => {
                      if (choice.id === selectedSubChoice) return;
                      setSelectedSubChoice(choice.id);
                      setBaseUrl(choice.baseUrl);
                      setTestPassed(false);
                      setTestError(null);
                      setModels([]);
                      setSelectedModelIds(new Set());
                    }}
                    className={cn(
                      'flex items-center gap-3 rounded-xl px-4 py-2.5 text-left transition-all',
                      isSelected
                        ? 'bg-white/20 shadow-sm border border-white/30'
                        : 'bg-white/5 border border-transparent hover:bg-white/10',
                    )}
                  >
                    {/* Radio */}
                    <div className="flex shrink-0">
                      {isSelected ? (
                        <div className="flex h-4 w-4 items-center justify-center rounded-full bg-[#FF6B4D]">
                          <div className="h-1.5 w-1.5 rounded-full bg-white" />
                        </div>
                      ) : (
                        <Circle className="h-4 w-4" style={{ color: 'rgba(255,255,255,0.4)' }} />
                      )}
                    </div>
                    {/* Label + URL on single line */}
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="text-token-sm font-medium text-white">
                          {choice.label}
                        </span>
                        <span className="text-token-xs truncate" style={{ color: 'rgba(255,255,255,0.5)' }}>
                          {host}
                        </span>
                      </div>
                    </div>
                  </button>
                );
              })}
            </div>
          </div>
        )}

        {/* ═══ API Key ═══ */}
        {needsApiKey && (
          <div className="mt-3">
            <label className="text-token-xs mb-1.5 block font-medium" style={{ color: 'rgba(255,255,255,0.7)' }}>
              {currentSubChoice?.credentialLabel ?? 'API Key'}
            </label>
            <div className="relative">
              <input
                type={showApiKey ? 'text' : 'password'}
                value={apiKey}
                onChange={(e) => {
                  setApiKey(e.target.value);
                  setTestPassed(false);
                  setTestError(null);
                }}
                placeholder={currentSubChoice?.credentialPlaceholder ?? 'sk-...'}
                disabled={isTesting}
                className={cn(
                  'w-full rounded-xl border bg-white/10 px-4 py-2.5 pr-16 text-token-sm text-white placeholder-white/40 focus:outline-none focus:ring-2 disabled:bg-white/5',
                  isShaking && 'animate-shake',
                  testError
                    ? 'border-red-500/40 focus:ring-red-500/50'
                    : 'border-white/20 focus:ring-[#FF6B4D]/50',
                )}
              />
              {/* Action buttons inside input */}
              <div className="absolute right-1.5 top-1/2 flex -translate-y-1/2 items-center gap-0.5">
                {/* Eye toggle */}
                <button
                  type="button"
                  onClick={() => setShowApiKey(!showApiKey)}
                  className="flex h-7 w-7 items-center justify-center rounded-lg transition-colors"
                  style={{ color: 'rgba(255,255,255,0.4)' }}
                  onMouseEnter={(e) => {
                    (e.currentTarget as HTMLButtonElement).style.background = 'rgba(255,255,255,0.1)';
                    (e.currentTarget as HTMLButtonElement).style.color = 'rgba(255,255,255,0.7)';
                  }}
                  onMouseLeave={(e) => {
                    (e.currentTarget as HTMLButtonElement).style.background = 'transparent';
                    (e.currentTarget as HTMLButtonElement).style.color = 'rgba(255,255,255,0.4)';
                  }}
                >
                  {showApiKey ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                </button>
                {/* Shield / test connection button */}
                <div className="relative">
                  <button
                    type="button"
                    onClick={handleTestConnection}
                    disabled={isTesting || !apiKey.trim()}
                    className="group relative flex h-7 w-7 items-center justify-center rounded-lg transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
                    style={{
                      color: isTesting ? 'rgba(255,255,255,0.6)'
                        : testPassed ? 'rgba(16,185,129,1)'
                        : apiKey.trim() ? 'rgba(255,255,255,1)' : 'rgba(255,255,255,0.25)',
                    }}
                    onMouseEnter={(e) => {
                      if (!isTesting && apiKey.trim()) {
                        (e.currentTarget as HTMLButtonElement).style.background = 'rgba(255,255,255,0.1)';
                      }
                    }}
                    onMouseLeave={(e) => {
                      (e.currentTarget as HTMLButtonElement).style.background = 'transparent';
                    }}
                    title="测试连接并获取模型列表"
                  >
                    {isTesting ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : testPassed ? (
                      <Check className="h-4 w-4" />
                    ) : (
                      <Shield className="h-4 w-4" />
                    )}
                  </button>
                  {/* Tooltip hint — shown on hover OR after idle timeout */}
                  {!isTesting && !testPassed && apiKey.trim() && (
                    <div className="pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-1.5 whitespace-nowrap rounded-lg px-2 py-1 text-token-xs font-medium transition-opacity group-hover:opacity-100" style={{ background: 'rgba(0,0,0,0.85)', color: 'rgba(255,255,255,0.9)', opacity: showShieldHint ? 1 : 0 }}>
                      点击测试连接
                      <div className="absolute left-1/2 -translate-x-1/2 top-full h-0 w-0 border-l-4 border-r-4 border-t-4 border-transparent border-t-black/85" />
                    </div>
                  )}
                </div>
              </div>
            </div>

            {/* Error message below input */}
            {testError && (
              <div className="mt-2 flex items-center gap-1.5 rounded-lg px-3 py-2" style={{ background: 'rgba(255,76,48,0.12)' }}>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#FF4C30" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z" />
                  <line x1="12" y1="9" x2="12" y2="13" />
                  <line x1="12" y1="17" x2="12.01" y2="17" />
                </svg>
                <span className="text-token-xs" style={{ color: 'rgba(255,255,255,0.85)' }}>
                  鉴权失败，请检查 API Key 是否有效。
                </span>
              </div>
            )}
          </div>
        )}

        {/* ═══ Auto-connecting indicator (Ollama) ═══ */}
        {isOllama && !testPassed && !testError && (
          <div className="mt-3 flex items-center justify-center gap-2 py-3 rounded-xl bg-white/10">
            <Loader2 className="h-4 w-4 animate-spin" style={{ color: 'rgba(255,255,255,0.6)' }} />
            <span className="text-token-sm" style={{ color: 'rgba(255,255,255,0.7)' }}>
              正在连接本地 Ollama...
            </span>
          </div>
        )}
      </div>

      {/* ═══ Model Selection ═══ */}
      {testPassed && (
        <div className="flex-1 overflow-y-auto px-3 py-4">
          {loadingModels ? (
            <div className="flex flex-col items-center justify-center py-8">
              <Loader2 className="h-6 w-6 animate-spin mb-3" style={{ color: 'rgba(255,255,255,0.6)' }} />
              <span className="text-token-sm" style={{ color: 'rgba(255,255,255,0.7)' }}>
                正在获取模型列表...
              </span>
            </div>
          ) : modelError ? (
            <div className="text-center py-6">
              <p className="text-token-sm text-red-300 mb-2">{modelError}</p>
              <button
                type="button"
                onClick={() => handleTestConnection()}
                className="text-token-xs text-[#FF6B4D] hover:underline"
              >
                重试
              </button>
            </div>
          ) : models.length > 0 ? (
            <div>
              {/* Selection info — above the model list */}
              <div className="flex items-center justify-between px-2 mb-2">
                <div className="flex items-center gap-1.5">
                  <span className="flex h-1.5 w-1.5 rounded-full bg-[#10B981]" />
                  <span className="text-token-xs font-medium" style={{ color: 'rgba(255,255,255,0.7)' }}>
                    已选 {selectedModelIds.size} / {models.length} 个模型
                  </span>
                </div>
              </div>

              {/* Fixed-height model list container — same background as header card */}
              <div
                className="overflow-y-auto rounded-xl px-1 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
                style={{
                  maxHeight: '280px',
                  background: 'rgba(255, 255, 255, 0.12)',
                  backdropFilter: 'blur(8px)',
                  border: '1px solid rgba(255, 255, 255, 0.15)',
                }}
              >
                <div className="flex flex-col gap-0.5 py-1">
                  {models.map((model) => (
                    <ModelRow
                      key={model.id}
                      model={model}
                      isSelected={selectedModelIds.has(model.id)}
                      onToggle={() => handleToggleModel(model.id)}
                    />
                  ))}
                </div>
              </div>
            </div>
          ) : null}
        </div>
      )}

      {/* ═══ Confirm Button (bottom) ═══ */}
      <div className="px-3 pb-4 pt-2">
        <button
          type="button"
          onClick={handleConfirm}
          disabled={selectedModelIds.size === 0 || isSaving || justSaved}
          className={cn(
            'w-full flex items-center justify-center gap-2 rounded-2xl px-4 py-3 text-token-sm font-semibold transition-all duration-300',
            justSaved
              ? 'bg-[#10B981] text-white shadow-lg scale-[1.02]'
              : isSaving
                ? 'text-white/70 cursor-wait'
                : selectedModelIds.size > 0
                  ? 'text-white active:scale-[0.98]'
                  : 'text-white/40 cursor-not-allowed',
          )}
          style={
            justSaved
              ? {
                  background: 'linear-gradient(135deg, #10B981, #059669)',
                  boxShadow: '0 8px 24px rgba(16, 185, 129, 0.3)',
                }
              : selectedModelIds.size > 0 && (isOllama || testPassed)
                ? {
                    background: 'rgba(255, 255, 255, 0.12)',
                    backdropFilter: 'blur(8px)',
                    border: '1px solid rgba(255, 255, 255, 0.2)',
                  }
                : {
                    background: 'rgba(255, 255, 255, 0.06)',
                    border: '1px solid rgba(255, 255, 255, 0.1)',
                  }
          }
        >
          {justSaved ? (
            <>
              <Check className="h-5 w-5" />
              已添加，继续 →
            </>
          ) : isSaving ? (
            <>
              <Loader2 className="h-4 w-4 animate-spin" />
              保存中...
            </>
          ) : selectedModelIds.size > 0 ? (
            <>
              <Check className="h-4 w-4" />
              确认+{selectedModelIds.size}个模型
            </>
          ) : (
            '选择模型'
          )}
        </button>
      </div>
    </div>
  );
}

export function ProviderSetupStep({
  onNext,
  onPrev,
  onWindowDrag,
}: ProviderSetupStepProps) {
  const allProviders: ProviderCardData[] = [
    ...DOMESTIC_PROVIDERS,
    ...INTERNATIONAL_PROVIDERS,
  ];

  // Selected provider for right panel
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const selectedData = allProviders.find((p) => p.id === selectedId);

  // Closing animation: keeps old panel visible during fade-out
  const [closingPanel, setClosingPanel] = useState<{ data: ProviderCardData; key: string } | null>(null);

  // Track which providers have been configured — loaded from backend on mount
  const [configuredProviders, setConfiguredProviders] = useState<Set<string>>(new Set());
  // Model pool: providerId -> modelIds[]
  const [modelPool, setModelPool] = useState<Map<string, string[]>>(new Map());
  const [showModelPool, setShowModelPool] = useState(false);
  const { getConfiguredProviders, getAllConfiguredModels } = useOnboarding();

  // Load configured providers and model pool from backend on mount
  const refreshPool = useCallback(async () => {
    const [ids, pool] = await Promise.all([
      getConfiguredProviders(),
      getAllConfiguredModels(),
    ]);
    if (ids.length > 0) setConfiguredProviders(new Set(ids));
    setModelPool(pool);
  }, [getConfiguredProviders, getAllConfiguredModels]);

  useEffect(() => {
    refreshPool();
  }, [refreshPool]);

  const handleProviderClick = (providerId: string) => {
    console.log('[ProviderSetupStep] clicked provider:', providerId, 'current selectedId:', selectedId);
    setSelectedId(providerId === selectedId ? null : providerId);
  };

  const handleClosePanel = () => {
    // Capture current panel data before clearing selection
    if (selectedData) {
      setClosingPanel({
        data: selectedData,
        key: `closing-${selectedData.id}-${Date.now()}`,
      });
    }
    setSelectedId(null);
    refreshPool();
    // Clear closingPanel after animation completes
    setTimeout(() => setClosingPanel(null), 400);
  };

  const handleProviderConfigured = (providerId: string) => {
    setConfiguredProviders((prev) => new Set(prev).add(providerId));
    refreshPool();
  };

  const totalModels = Array.from(modelPool.values()).reduce(
    (sum, ids) => sum + ids.length,
    0,
  );
  const providerCount = modelPool.size;

  return (
    <OnboardingLayout
      rightPanel={
        <div className="relative flex flex-col h-full overflow-hidden">
          <RightPanelHeader
            stepLabel="STEP 4: 模型提供商"
            title="选定默认模型来源。"
            bullets={[
              '支持云端国产与本地方案',
              'Provider 凭证保存在本机',
              '默认模型可在主界面随时更改',
            ]}
          />
          {/* Divider */}
          <div
            className="mx-5 h-px shrink-0"
            style={{ background: 'rgba(255,255,255,0.1)' }}
          />
          {selectedData ? (
            <ProviderConfigPanel
              provider={selectedData}
              onClose={handleClosePanel}
              onProviderConfigured={handleProviderConfigured}
            />
          ) : (
            <ProviderEmptyState modelPool={modelPool} allProviders={allProviders} />
          )}
          {/* ═══ Closing animation overlay ═══ */}
          {closingPanel && (
            <div
              key={closingPanel.key}
              className="absolute inset-0 flex flex-col items-center justify-center"
              style={{
                animation: 'slideFadeOut 0.35s ease-in forwards',
                background: 'rgba(26, 26, 26, 0.95)',
                backdropFilter: 'blur(4px)',
                pointerEvents: 'none',
                zIndex: 50,
              }}
            >
              <div className="flex items-center gap-3 mb-3">
                <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-[#10B981]/20">
                  <Check className="h-6 w-6 text-[#10B981]" />
                </div>
                <div>
                  <p className="text-token-base font-semibold text-white">
                    已添加 {closingPanel.data.displayName} 模型
                  </p>
                  <p className="text-token-xs" style={{ color: 'rgba(255,255,255,0.6)' }}>
                    正在刷新模型列表…
                  </p>
                </div>
              </div>
            </div>
          )}
        </div>
      }
      onWindowDrag={onWindowDrag}
    >
      <StepProgressBar currentStep={4} />

      {/* Inline step badge + heading */}
      <div className="flex items-center gap-3 px-8 pb-2">
        <div className="flex h-8 w-8 items-center justify-center rounded-full bg-brand-orange text-token-md font-bold text-white shrink-0">
          4
        </div>
        <h1 className="text-token-3xl font-bold text-foreground font-sans tracking-tight">
          选定默认模型来源
        </h1>
      </div>

      {/* Left content area */}
      <div className="flex-1 overflow-y-auto px-8 pb-4">
        {/* Info banner */}
        <div className="mb-5 flex items-center gap-2 rounded-lg bg-[#E8F5E9]/80 px-4 py-2.5 font-sans">
          <Check className="h-4 w-4 text-status-success shrink-0" />
          <span className="text-token-xs text-foreground/80">
            选择后可随时在「设置 → 模型」中更换或添加新服务商，现在也可以跳过。
          </span>
        </div>

        {/* Domestic providers */}
        <div className="mb-5">
          <h3 className="text-token-sm font-semibold text-foreground mb-3 font-sans">
            国内推荐
          </h3>
          <div className="grid grid-cols-3 gap-2.5">
            {DOMESTIC_PROVIDERS.map((p) => (
              <ProviderGridCard
                key={p.id}
                data={p}
                isSelected={selectedId === p.id}
                isConfigured={configuredProviders.has(p.id)}
                onClick={() => handleProviderClick(p.id)}
              />
            ))}
          </div>
        </div>

        {/* International providers */}
        <div className="mb-5">
          <h3 className="text-token-sm font-semibold text-foreground mb-3 font-sans">
            国际主流
          </h3>
          <div className="grid grid-cols-3 gap-2.5">
            {INTERNATIONAL_PROVIDERS.map((p) => (
              <ProviderGridCard
                key={p.id}
                data={p}
                isSelected={selectedId === p.id}
                isConfigured={configuredProviders.has(p.id)}
                onClick={() => handleProviderClick(p.id)}
              />
            ))}
          </div>
        </div>
      </div>

      {/* Model Pool Summary Bar */}
      {providerCount > 0 && !showModelPool && (
        <div className="px-8 pb-3">
          <div
            className="flex items-center justify-between rounded-xl px-4 py-3 cursor-pointer transition-all hover:bg-[#F5F5F5]"
            onClick={() => setShowModelPool(true)}
          >
            <div className="flex items-center gap-2">
              <span className="flex h-6 w-6 items-center justify-center rounded-full bg-[#10B981]/10">
                <Check className="h-3.5 w-3.5 text-[#10B981]" />
              </span>
              <span className="text-token-sm text-foreground">
                已选 <strong className="font-semibold">{totalModels}</strong> 个模型，来自{' '}
                <strong className="font-semibold">{providerCount}</strong> 个服务商
              </span>
            </div>
            <span className="text-token-xs" style={{ color: 'rgba(0,0,0,0.4)' }}>
              点击查看 →
            </span>
          </div>
        </div>
      )}

      {/* Model Pool Viewer */}
      {showModelPool && (
        <div className="px-8 pb-3">
          <div className="rounded-xl border border-[#E5E7EB] bg-white px-4 py-3">
            <div className="flex items-center justify-between mb-3">
              <span className="text-token-sm font-semibold text-foreground">
                已选模型 ({totalModels})
              </span>
              <button
                type="button"
                onClick={() => setShowModelPool(false)}
                className="flex h-6 w-6 items-center justify-center rounded-lg text-[#9CA3AF] hover:bg-gray-100 hover:text-gray-600"
              >
                <X className="h-4 w-4" />
              </button>
            </div>
            <div className="flex flex-col gap-3 max-h-40 overflow-y-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
              {Array.from(modelPool.entries()).map(([provId, modelIds]) => {
                const provData = allProviders.find((p) => p.id === provId);
                const provName = provData?.displayName ?? provId;
                return (
                  <div key={provId}>
                    <div className="flex items-center gap-1.5 mb-1">
                      <span className="text-token-xs font-semibold" style={{ color: '#6B7280' }}>
                        {provName}
                      </span>
                      <span className="text-token-xs" style={{ color: '#9CA3AF' }}>
                        ({modelIds.length})
                      </span>
                    </div>
                    <div className="flex flex-wrap gap-1.5">
                      {modelIds.map((mid) => (
                        <span
                          key={mid}
                          className="rounded-md px-2 py-0.5 text-token-xs font-medium"
                          style={{
                            background: 'rgba(0,0,0,0.05)',
                            color: 'rgba(0,0,0,0.7)',
                          }}
                        >
                          {mid}
                        </span>
                      ))}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      )}

      {/* Bottom navigation */}
      <StepNavigation
        currentStep={4}
        onNext={onNext}
        onPrev={onPrev}
        canGoNext={configuredProviders.size > 0}
        canGoPrev
        nextLabel="再配置 →"
      />
    </OnboardingLayout>
  );
}
