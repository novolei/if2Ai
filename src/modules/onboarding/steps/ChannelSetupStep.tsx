/**
 * ChannelSetupStep — Step 5 of the 6-step Onboarding flow.
 *
 * Displays a grid of 13 channel cards (common + additional). Users click
 * a channel, fill in configuration (bot token, app secret, etc.), and
 * test the connection. At least one channel must be configured to proceed.
 *
 * Design reference: docs/references/onboarding-steps/ChannelSetup.png
 */

import { useState, useCallback, useEffect } from 'react';
import { Loader2, Check, X } from 'lucide-react';
import { cn } from '@/lib/utils';
import { InfoPanel, InfoCard } from '../components/InfoPanel';
import { OnboardingLayout } from '../components/OnboardingLayout';
import { StepHeader } from '../components/StepHeader';
import { StepNavigation } from '../components/StepNavigation';
import { ChannelCard } from '../components/ChannelCard';
import { useOnboarding } from '../hooks/useOnboarding';
import type { Channel, ChannelConfig, TestResult } from '../types';

interface ChannelSetupStepProps {
  onNext: () => void;
  onPrev: () => void;
}

/** Built-in channel list with display metadata. */
const BUILTIN_CHANNELS: Channel[] = [
  // Common channels
  { id: 'feishu', name: '飞书', category: 'Social', icon: '📱', logo_path: 'src/assets/ChannelLogos/channel_logo_feishu.png', requires_token: true, requires_secret: true, requires_webhook: false, node_version_required: null },
  { id: 'qq', name: 'QQ Bot', category: 'Social', icon: '🐧', logo_path: 'src/assets/ChannelLogos/channel_logo_qq.png', requires_token: true, requires_secret: false, requires_webhook: false, node_version_required: null },
  { id: 'wechat', name: 'WeChat (微信)', category: 'Social', icon: '💬', logo_path: 'src/assets/ChannelLogos/channel_logo_wechat.png', requires_token: true, requires_secret: false, requires_webhook: false, node_version_required: null },
  { id: 'telegram', name: 'Telegram (Bot API)', category: 'Social', icon: '✈️', logo_path: 'src/assets/ChannelLogos/channel_logo_telegram.png', requires_token: true, requires_secret: false, requires_webhook: false, node_version_required: null },
  // Additional channels
  { id: 'whatsapp', name: 'WhatsApp (QR link)', category: 'Messaging', icon: '📞', logo_path: 'src/assets/ChannelLogos/channel_logo_whatsapp.png', requires_token: false, requires_secret: false, requires_webhook: false, node_version_required: null },
  { id: 'teams', name: 'Microsoft Teams', category: 'Messaging', icon: '🏢', logo_path: 'src/assets/ChannelLogos/channel_logo_msteams.png', requires_token: true, requires_secret: true, requires_webhook: false, node_version_required: null },
  { id: 'discord', name: 'Discord (Bot API)', category: 'Messaging', icon: '🎮', logo_path: 'src/assets/ChannelLogos/channel_logo_discord.png', requires_token: true, requires_secret: false, requires_webhook: false, node_version_required: null },
  { id: 'slack', name: 'Slack (Socket Mode)', category: 'Messaging', icon: '💼', logo_path: 'src/assets/ChannelLogos/channel_logo_slack.png', requires_token: true, requires_secret: false, requires_webhook: false, node_version_required: null },
  { id: 'imessage', name: 'iMessage (imsg)', category: 'Messaging', icon: '💜', logo_path: 'src/assets/ChannelLogos/channel_logo_imessage.png', requires_token: false, requires_secret: false, requires_webhook: false, node_version_required: null },
  { id: 'line', name: 'LINE (Messaging API)', category: 'Messaging', icon: '🟢', logo_path: 'src/assets/ChannelLogos/channel_logo_line.png', requires_token: true, requires_secret: true, requires_webhook: false, node_version_required: null },
  { id: 'signal', name: 'Signal (signal-cli)', category: 'Desktop', icon: '🔒', logo_path: 'src/assets/ChannelLogos/channel_logo_signal.png', requires_token: false, requires_secret: false, requires_webhook: false, node_version_required: null },
  { id: 'mattermost', name: 'Mattermost (plugin)', category: 'Desktop', icon: '🔵', logo_path: 'src/assets/ChannelLogos/channel_logo_mattermost.png', requires_token: true, requires_secret: false, requires_webhook: false, node_version_required: null },
  { id: 'matrix', name: 'Matrix (glgnt)', category: 'Desktop', icon: '⬡', logo_path: 'src/assets/ChannelLogos/channel_logo_matrix.png', requires_token: true, requires_secret: false, requires_webhook: false, node_version_required: null },
];

export function ChannelSetupStep({ onNext, onPrev }: ChannelSetupStepProps) {
  const {
    channels: backendChannels,
    configuredChannels,
    channelTestResult,
    testChannel,
    configureChannel,
  } = useOnboarding();

  const channels = backendChannels.length > 0 ? backendChannels : BUILTIN_CHANNELS;
  const commonChannels = channels.filter((c) => c.category === 'Social');
  const otherChannels = channels.filter((c) => c.category !== 'Social');

  // Local UI state
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [botToken, setBotToken] = useState('');
  const [appSecret, setAppSecret] = useState('');
  const [webhookUrl, setWebhookUrl] = useState('');
  const [isTesting, setIsTesting] = useState(false);
  const [testPassed, setTestPassed] = useState(false);
  const [testError, setTestError] = useState<string | null>(null);

  const selectedChannel = channels.find((c) => c.id === selectedId);

  // Track which channels are connected (from configuredChannels or local test results)
  const connectedIds = new Set(configuredChannels.map((c) => c.channel_id));

  // Reset form when selection changes
  useEffect(() => {
    setShowForm(false);
    setBotToken('');
    setAppSecret('');
    setWebhookUrl('');
    setIsTesting(false);
    setTestPassed(false);
    setTestError(null);
  }, [selectedId]);

  const handleChannelClick = (channelId: string) => {
    setSelectedId(channelId);
    setShowForm(true);
  };

  const handleSaveAndTest = useCallback(async () => {
    if (selectedChannel?.requires_token && !botToken.trim()) {
      setTestError('请输入 Bot Token');
      return;
    }

    setIsTesting(true);
    setTestError(null);
    setTestPassed(false);

    try {
      const config: ChannelConfig = {
        channel_id: selectedId ?? '',
        bot_token: botToken || null,
        app_secret: appSecret || null,
        webhook_url: webhookUrl || null,
        display_name: selectedChannel?.name ?? '',
      };
      await configureChannel(config);
      await testChannel(config);
      setTestPassed(true);
      connectedIds.add(selectedId ?? '');
    } catch (err) {
      setTestError(err instanceof Error ? err.message : '连接测试失败');
    } finally {
      setIsTesting(false);
    }
  }, [selectedId, selectedChannel, botToken, appSecret, webhookUrl, configureChannel, testChannel, connectedIds]);

  const hasAnyConfigured = connectedIds.size > 0 || configuredChannels.length > 0;

  return (
    <OnboardingLayout
      rightPanel={
        <InfoPanel
          stepLabel="STEP 5: 通讯渠道"
          title="把你常用的平台接入 if2AI。"
          bullets={[
            '支持 13 个消息平台',
            '配置后自动测试连接',
            '至少配置一个渠道',
          ]}
        >
          {/* Connected channels list */}
          {configuredChannels.length > 0 && (
            <div className="mt-4 flex flex-col gap-2">
              <span className="text-token-xs text-muted-foreground">
                已连接 {configuredChannels.length} 个
              </span>
              {configuredChannels.map((ch) => (
                <InfoCard key={ch.channel_id} className="flex items-center gap-2.5">
                  <div className="flex h-6 w-6 items-center justify-center rounded-full bg-status-success text-white">
                    <Check className="h-3.5 w-3.5" />
                  </div>
                  <span className="text-token-sm font-medium text-white">
                    {ch.display_name}
                  </span>
                </InfoCard>
              ))}
            </div>
          )}
        </InfoPanel>
      }
    >
      <StepHeader currentStep={5} title="接入社交渠道" />

      {/* Left content area */}
      <div className="flex-1 overflow-y-auto px-8 pb-4">
        <p className="text-token-sm text-muted-foreground mb-4 leading-relaxed">
          选择希望接入的渠道，确保每个"设置 &gt; 渠道"都能正确验证。
        </p>

        {/* Common channels (3 columns) */}
        <div className="mb-3">
          <span className="text-token-xs text-muted-foreground mb-2 block">
            常用渠道
          </span>
          <div className="grid grid-cols-3 gap-2.5">
            {commonChannels.map((channel) => (
              <ChannelCard
                key={channel.id}
                channel={channel}
                isSelected={selectedId === channel.id}
                isConnected={connectedIds.has(channel.id)}
                isTesting={isTesting && selectedId === channel.id}
                testResult={channelTestResult[channel.id] ?? null}
                onClick={() => handleChannelClick(channel.id)}
              />
            ))}
          </div>
        </div>

        {/* Other channels (4 columns) */}
        <div>
          <span className="text-token-xs text-muted-foreground mb-2 block">
            其他渠道
          </span>
          <div className="grid grid-cols-4 gap-2.5">
            {otherChannels.map((channel) => (
              <ChannelCard
                key={channel.id}
                channel={channel}
                isSelected={selectedId === channel.id}
                isConnected={connectedIds.has(channel.id)}
                isTesting={isTesting && selectedId === channel.id}
                testResult={channelTestResult[channel.id] ?? null}
                onClick={() => handleChannelClick(channel.id)}
              />
            ))}
          </div>
        </div>

        {/* Config form for selected channel */}
        {showForm && selectedChannel && (
          <div className="mt-4 rounded-lg border border-border bg-muted/20 px-4 py-4">
            <h3 className="text-token-sm font-semibold text-foreground mb-3">
              配置 {selectedChannel.name}
            </h3>

            <div className="flex flex-col gap-3">
              {/* Bot Token */}
              {selectedChannel.requires_token && (
                <div>
                  <label className="text-token-xs text-muted-foreground mb-1 block">
                    Bot Token
                  </label>
                  <input
                    type="password"
                    value={botToken}
                    onChange={(e) => setBotToken(e.target.value)}
                    placeholder="输入 Bot Token"
                    className="w-full rounded-md border border-border bg-input px-3 py-2 text-token-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-brand-orange"
                  />
                </div>
              )}

              {/* App Secret */}
              {selectedChannel.requires_secret && (
                <div>
                  <label className="text-token-xs text-muted-foreground mb-1 block">
                    App Secret
                  </label>
                  <input
                    type="password"
                    value={appSecret}
                    onChange={(e) => setAppSecret(e.target.value)}
                    placeholder="输入 App Secret"
                    className="w-full rounded-md border border-border bg-input px-3 py-2 text-token-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-brand-orange"
                  />
                </div>
              )}

              {/* Webhook URL */}
              {selectedChannel.requires_webhook && (
                <div>
                  <label className="text-token-xs text-muted-foreground mb-1 block">
                    Webhook URL
                  </label>
                  <input
                    type="text"
                    value={webhookUrl}
                    onChange={(e) => setWebhookUrl(e.target.value)}
                    placeholder="https://example.com/webhook"
                    className="w-full rounded-md border border-border bg-input px-3 py-2 text-token-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-brand-orange"
                  />
                </div>
              )}

              {/* Save and Test button */}
              <button
                type="button"
                onClick={handleSaveAndTest}
                disabled={
                  isTesting ||
                  (selectedChannel.requires_token && !botToken.trim())
                }
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
                    连接成功
                  </>
                ) : (
                  '保存并测试'
                )}
              </button>

              {/* Error message */}
              {testError && (
                <div className="flex items-center gap-2 text-token-xs text-status-error">
                  <X className="h-3.5 w-3.5 shrink-0" />
                  <span>{testError}</span>
                  <button
                    type="button"
                    onClick={handleSaveAndTest}
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
        currentStep={5}
        onNext={onNext}
        onPrev={onPrev}
        canGoNext={hasAnyConfigured}
        canGoPrev
        nextLabel="完成部署 →"
      />
    </OnboardingLayout>
  );
}
