/**
 * SystemCheckStep — Step 2 of the 6-step Onboarding flow.
 *
 * Automatically runs system checks on mount (CPU, GPU, Memory)
 * and displays Embedded model download UI. Only the model download
 * is a blocking check; CPU/GPU/Memory are info-only (always Pass).
 *
 * Design reference: docs/references/onboarding-steps/SystemCheck.png
 */

import type { MouseEvent as ReactMouseEvent } from 'react';
import { useEffect, useRef, useState } from 'react';
import { Cpu, Monitor, HardDrive, Download, RefreshCw } from 'lucide-react';
import { cn } from '@/lib/utils';
import { InfoPanel } from '../components/InfoPanel';
import { OnboardingLayout } from '../components/OnboardingLayout';
import { StepNavigation } from '../components/StepNavigation';
import { StepProgressBar } from '../components/StepProgressBar';
import { useOnboarding } from '../hooks/useOnboarding';
import type { CheckStatus } from '../types';

/** Display name for the embedded model (matches HuggingFace). */
const MODEL_NAME = 'intfloat/multilingual-e5-small';
const MODEL_DETAIL = '多语言向量化模型 · 384 维';

// Inject keyframes for download UI (shimmer pulse + progress stripe sweep).
// Idempotent: only adds the <style> once even across HMR / re-renders.
if (typeof document !== 'undefined' && !document.getElementById('onboarding-syscheck-keyframes')) {
  const style = document.createElement('style');
  style.id = 'onboarding-syscheck-keyframes';
  style.textContent = `
    @keyframes shimmer {
      0% { transform: translateX(-100%); }
      100% { transform: translateX(300%); }
    }
    @keyframes progressStripes {
      0% { background-position: 0 0; }
      100% { background-position: 24px 0; }
    }
  `;
  document.head.appendChild(style);
}

interface SystemCheckStepProps {
  onNext: () => void;
  onPrev: () => void;
  onWindowDrag?: (event: ReactMouseEvent<HTMLElement>) => void;
}

/** Extract human-readable status label. */
function statusLabel(status: CheckStatus): string {
  switch (status.status) {
    case 'Pass': return '通过';
    case 'Fail': return status.reason;
    case 'Running': return '检测中...';
    case 'Pending': return '等待检测';
  }
}

/** Icon for each check item. */
const CHECK_ICONS = {
  cpu: <Cpu className="h-4 w-4" />,
  gpu: <Monitor className="h-4 w-4" />,
  memory: <HardDrive className="h-4 w-4" />,
  model: <Download className="h-4 w-4" />,
} as const;

/** Status badge color based on CheckStatus. */
function statusBadgeClass(status: CheckStatus): string {
  switch (status.status) {
    case 'Pass': return 'bg-status-success text-white';
    case 'Fail': return 'bg-status-error text-white';
    case 'Running': return 'bg-brand-orange/20 text-brand-orange';
    case 'Pending': return 'bg-muted/50 text-muted-foreground/40';
  }
}

/** Text color based on CheckStatus. */
function statusTextClass(status: CheckStatus): string {
  switch (status.status) {
    case 'Pass': return 'text-status-success';
    case 'Fail': return 'text-status-error';
    case 'Running': return 'text-brand-orange';
    case 'Pending': return 'text-muted-foreground/40';
  }
}

/** Font size for status label — smaller than body text. */
const STATUS_FONT_SIZE = 'text-[10px]';

/** Format bytes to human-readable string with 1 decimal precision for MB. */
function formatBytes(bytes: number): string {
  if (!bytes || bytes <= 0) return '0 MB';
  const mb = bytes / (1024 * 1024);
  if (mb >= 1024) return `${(mb / 1024).toFixed(2)} GB`;
  if (mb >= 10) return `${Math.round(mb)} MB`;
  return `${mb.toFixed(1)} MB`;
}

/** Format seconds → "12s" / "1m 30s" */
function formatEta(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return '—';
  if (seconds < 60) return `${Math.ceil(seconds)} 秒`;
  const m = Math.floor(seconds / 60);
  const s = Math.ceil(seconds % 60);
  return s > 0 ? `${m}分${s}秒` : `${m}分`;
}

/**
 * Track download speed by sampling (downloadedBytes, ts) over time.
 * Returns smoothed bytes/sec and ETA seconds. Uses a 4-sample window so it
 * settles quickly but doesn't flicker.
 */
function useDownloadMetrics(downloadedBytes: number, totalBytes: number, isDownloading: boolean) {
  const samplesRef = useRef<Array<{ bytes: number; ts: number }>>([]);
  const [bytesPerSec, setBytesPerSec] = useState(0);
  const [eta, setEta] = useState(0);

  useEffect(() => {
    if (!isDownloading) {
      samplesRef.current = [];
      setBytesPerSec(0);
      setEta(0);
      return;
    }
    const now = performance.now();
    const samples = samplesRef.current;
    samples.push({ bytes: downloadedBytes, ts: now });
    // 保留最近 4 秒的样本
    while (samples.length > 1 && now - samples[0].ts > 4000) samples.shift();
    if (samples.length >= 2) {
      const first = samples[0];
      const last = samples[samples.length - 1];
      const dt = (last.ts - first.ts) / 1000;
      const db = last.bytes - first.bytes;
      const bps = dt > 0 ? db / dt : 0;
      setBytesPerSec(bps);
      const remaining = Math.max(0, totalBytes - downloadedBytes);
      setEta(bps > 0 ? remaining / bps : 0);
    }
  }, [downloadedBytes, totalBytes, isDownloading]);

  return { bytesPerSec, eta };
}

/** Render the status icon inside a badge. */
function StatusIcon({ status }: { status: CheckStatus }) {
  if (status.status === 'Pass') {
    return (
      <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
        <path d="M2 5L4.5 7.5L8 3" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    );
  }
  if (status.status === 'Fail') {
    return (
      <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
        <path d="M2.5 2.5L7.5 7.5M7.5 2.5L2.5 7.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      </svg>
    );
  }
  if (status.status === 'Running') {
    return (
      <svg width="10" height="10" viewBox="0 0 10 10" className="animate-spin">
        <circle cx="5" cy="5" r="4" stroke="currentColor" strokeWidth="1.5" fill="none" strokeDasharray="16" strokeDashoffset="4" />
      </svg>
    );
  }
  return <span className="text-[8px] font-bold text-muted-foreground/40">-</span>;
}

export function SystemCheckStep({ onNext, onPrev, onWindowDrag }: SystemCheckStepProps) {
  const {
    systemReport,
    isChecking,
    downloadProgress,
    downloadedBytes,
    totalBytes,
    downloadError,
    isDownloading,
    runSystemCheck,
    downloadEmbeddedModel,
  } = useOnboarding();
  const { bytesPerSec, eta } = useDownloadMetrics(downloadedBytes, totalBytes, isDownloading);
  const autoDownloadRequestedRef = useRef(false);
  const [downloadWatchdogExpired, setDownloadWatchdogExpired] = useState(false);
  const [autoDownloadAttempted, setAutoDownloadAttempted] = useState(false);

  // Auto-run system check on mount
  useEffect(() => {
    runSystemCheck();
  }, [runSystemCheck]);

  const report = systemReport;

  // Only model download is blocking. CPU/GPU/Memory are info-only (always Pass).
  const modelDownloaded = report?.embedded_model.downloaded ?? false;
  const modelStatus = report?.embedded_model.status;
  const modelDone = modelDownloaded || modelStatus?.status === 'Pass';

  // allPassed only depends on model download completion
  const allPassed = modelDone;
  const canContinue = Boolean(report) && !isChecking;
  const canDeferModelDownload =
    !allPassed &&
    !isChecking &&
    (Boolean(downloadError) || modelStatus?.status === 'Fail' || downloadWatchdogExpired);

  // Auto-download once after the first report confirms the model is missing.
  // Stale backend progress used to report "Running" without an active task; the
  // Rust side now distinguishes active downloads, but this guard also prevents
  // duplicate starts under React StrictMode.
  useEffect(() => {
    if (
      report &&
      !autoDownloadRequestedRef.current &&
      !modelDownloaded &&
      !downloadError &&
      !isDownloading &&
      modelStatus?.status === 'Pending'
    ) {
      autoDownloadRequestedRef.current = true;
      setAutoDownloadAttempted(true);
      downloadEmbeddedModel();
    }
  }, [report, modelDownloaded, downloadError, isDownloading, modelStatus, downloadEmbeddedModel]);

  // Product guardrail: model downloads depend on network/HuggingFace availability,
  // so Step 2 must never become a dead end. After a short grace period, expose a
  // clear "continue and download later" path while keeping the download running.
  useEffect(() => {
    if (allPassed) {
      setDownloadWatchdogExpired(false);
      return;
    }
    if (!autoDownloadAttempted && !isDownloading && modelStatus?.status !== 'Running') {
      setDownloadWatchdogExpired(false);
      return;
    }

    const timer = window.setTimeout(() => {
      setDownloadWatchdogExpired(true);
    }, 30_000);
    return () => window.clearTimeout(timer);
  }, [allPassed, autoDownloadAttempted, isDownloading, modelStatus?.status]);

  // Refresh system report after download finishes → enables continue button
  const prevDownloadingRef = useRef(isDownloading);
  useEffect(() => {
    if (prevDownloadingRef.current && !isDownloading) {
      // Download just finished (success or failure) — re-check to update status
      runSystemCheck();
    }
    prevDownloadingRef.current = isDownloading;
  }, [isDownloading, runSystemCheck]);

  // Hardware info items — all always Pass (info gathering only)
  const checkItems = report
    ? [
        {
          key: 'cpu',
          label: 'CPU',
          icon: CHECK_ICONS.cpu,
          detail: `${report.cpu.architecture} · ${report.cpu.cores} 核`,
          status: { status: 'Pass' as const } satisfies CheckStatus,
        },
        {
          key: 'gpu',
          label: 'GPU',
          icon: CHECK_ICONS.gpu,
          detail: report.gpu.available
            ? report.gpu.name ?? '已检测到 GPU'
            : '未检测到 GPU（将使用 CPU 模式）',
          status: { status: 'Pass' as const } satisfies CheckStatus,
        },
        {
          key: 'memory',
          label: '内存',
          icon: CHECK_ICONS.memory,
          detail: `总量 ${report.memory.total_mb} MB · 可用 ${report.memory.available_mb} MB`,
          status: { status: 'Pass' as const } satisfies CheckStatus,
        },
      ]
    : [
        { key: 'cpu', label: 'CPU', icon: CHECK_ICONS.cpu, detail: '检测中...', status: { status: 'Running' as const } },
        { key: 'gpu', label: 'GPU', icon: CHECK_ICONS.gpu, detail: '检测中...', status: { status: 'Running' as const } },
        { key: 'memory', label: '内存', icon: CHECK_ICONS.memory, detail: '检测中...', status: { status: 'Running' as const } },
      ];

  // Model download data
  const modelPercent =
    modelDownloaded
      ? 100
      : isDownloading
        ? downloadProgress
        : report?.embedded_model.progress
          ? report.embedded_model.progress * 100
          : 0;

  const modelSizeMb = report?.embedded_model.size_mb ?? 120;
  const modelTotalBytes = totalBytes > 0 ? totalBytes : modelSizeMb * 1024 * 1024;
  const modelDownloadedBytes = downloadedBytes > 0
    ? downloadedBytes
    : modelDownloaded
      ? modelTotalBytes
      : Math.round((modelPercent / 100) * modelTotalBytes);

  const modelRowStatus: CheckStatus = modelDownloaded
    ? { status: 'Pass' }
    : isDownloading || modelStatus?.status === 'Running'
      ? { status: 'Running' }
      : modelStatus?.status === 'Fail'
        ? modelStatus
        : { status: 'Pending' };
  const modelStatusLabel =
    modelRowStatus.status === 'Running'
      ? '后台下载中'
      : modelRowStatus.status === 'Pending'
        ? '准备中'
        : statusLabel(modelRowStatus);
  const setupHeadline = allPassed
    ? '系统就绪，继续 →'
    : report
      ? '环境已就绪，模型后台准备中'
      : '正在检测环境…';
  const setupHint = allPassed
    ? '✓ 全部检测通过，可以进入下一步'
    : downloadError
      ? '下载遇到问题，可重试或稍后在设置 · 模型配置中继续'
      : downloadWatchdogExpired
        ? '下载比预期更久，可以先继续，模型会在后台/设置中继续准备'
        : isChecking
          ? '检测硬件环境中…'
          : isDownloading || modelStatus?.status === 'Running'
            ? `后台下载中 · ${bytesPerSec > 0 ? `${formatBytes(bytesPerSec)}/s` : '建立连接中'}`
            : report
              ? '环境检测完成，正在准备后台下载'
              : '检测完成后会自动开始下载';

  return (
    <OnboardingLayout
      rightPanel={
        <InfoPanel
          stepLabel="STEP 2: 系统预检"
          title="先体检，再安装。"
          bullets={[
            '先快速读取 CPU/GPU/内存',
            '随后自动准备 FastEmbed 向量模型',
            '网络慢时可先继续，稍后在设置里补下载',
          ]}
        >
          {/* Summary card — semi-transparent on orange bg, matching Step 4 style */}
          <div className="mt-4">
            <div
              className="rounded-2xl overflow-hidden"
              style={{
                background: 'rgba(255, 255, 255, 0.12)',
                backdropFilter: 'blur(8px)',
                border: '1px solid rgba(255, 255, 255, 0.15)',
              }}
            >
              {/* Header bar */}
              <div className={cn(
                'flex items-center gap-2 px-4 py-3',
                allPassed ? 'bg-white/20' : 'bg-white/10',
              )}>
                <div
                  className={cn(
                    'flex h-5 w-5 items-center justify-center rounded-full',
                    allPassed ? 'bg-[#10B981]' : 'bg-white/20',
                  )}
                >
                  {allPassed ? (
                    <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
                      <path d="M2 5L4.5 7.5L8 3" stroke="white" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
                    </svg>
                  ) : (
                    <span className="text-[10px] font-bold" style={{ color: 'rgba(255,255,255,0.6)' }}>…</span>
                  )}
                </div>
                <span className="text-[13px] font-semibold" style={{ color: 'rgba(255,255,255,0.95)' }}>
                  {setupHeadline}
                </span>
              </div>

              {/* Status items */}
              <div className="px-4 py-2.5 space-y-1.5">
                {checkItems.map((c) => (
                  <div key={c.key} className="flex items-center gap-2">
                    <span
                      className={cn(
                        'inline-block h-1.5 w-1.5 rounded-full shrink-0',
                        c.status.status === 'Pass' && 'bg-white',
                        c.status.status === 'Running' && 'bg-[#FF6B4D] animate-pulse',
                      )}
                    />
                    <span className="text-[12px]" style={{ color: 'rgba(255,255,255,0.9)' }}>
                      {c.label}: {statusLabel(c.status)}
                    </span>
                  </div>
                ))}
                {/* Model status row */}
                <div className="flex items-center gap-2">
                  <span
                    className={cn(
                      'inline-block h-1.5 w-1.5 rounded-full shrink-0',
                      modelRowStatus.status === 'Pass' && 'bg-white',
                      modelRowStatus.status === 'Fail' && 'bg-red-400',
                      modelRowStatus.status === 'Running' && 'bg-[#FF6B4D] animate-pulse',
                      modelRowStatus.status === 'Pending' && 'bg-white/30',
                    )}
                  />
                  <span className="text-[12px]" style={{ color: 'rgba(255,255,255,0.9)' }}>
                    模型: {modelStatusLabel}
                  </span>
                </div>
              </div>

              {/* Live download progress mini-bar in right panel */}
              {!modelDownloaded && (isDownloading || modelStatus?.status === 'Running') && (
                <div className="px-4 pb-2.5">
                  <div className="h-1 rounded-full overflow-hidden" style={{ background: 'rgba(255,255,255,0.18)' }}>
                    <div
                      className="h-full rounded-full transition-[width] duration-300 motion-reduce:transition-none"
                      style={{
                        width: `${modelPercent}%`,
                        background: 'linear-gradient(90deg, #FF6B4D 0%, #FFAA85 100%)',
                      }}
                    />
                  </div>
                  <div className="mt-1 flex items-center justify-between text-[9.5px] tabular-nums" style={{ color: 'rgba(255,255,255,0.55)' }}>
                    <span>
                      {modelTotalBytes > 0
                        ? `${formatBytes(modelDownloadedBytes)} / ${formatBytes(modelTotalBytes)}`
                        : '准备下载…'}
                    </span>
                    <span>{Math.round(modelPercent)}%{eta > 0 && eta < 3600 ? ` · ${formatEta(eta)}` : ''}</span>
                  </div>
                </div>
              )}

              {/* Footer hint */}
              <div className="px-4 py-2 border-t" style={{ borderColor: 'rgba(255,255,255,0.1)' }}>
                <span className="text-[10px]" style={{ color: 'rgba(255,255,255,0.55)' }}>
                  {setupHint}
                </span>
              </div>
            </div>
          </div>
        </InfoPanel>
      }
      onWindowDrag={onWindowDrag}
    >
      <StepProgressBar currentStep={2} />

      {/* Inline step badge + heading */}
      <div className="flex items-center gap-3 px-8 pb-2">
        <div className="flex h-8 w-8 items-center justify-center rounded-full bg-brand-orange text-token-md font-bold text-white shrink-0">
          2
        </div>
        <h1 className="text-token-3xl font-bold text-foreground font-sans tracking-tight">
          准备本地向量模型
        </h1>
      </div>

      {/* Left content area */}
      <div className="flex-1 overflow-y-auto px-8 pb-6">
        <p className="text-token-sm text-muted-foreground mb-5 leading-relaxed">
          If2Ai 会先确认设备信息，然后在后台准备 FastEmbed 向量模型。你可以继续完成 onboarding；模型准备好后，本地记忆检索会自动可用。
        </p>

        {/* ── Unified check card ── */}
        <div className="rounded-xl border border-border bg-card overflow-hidden">
          {/* Card header */}
          <div className="px-4 py-3 border-b border-border">
            <h2 className="text-token-sm font-semibold text-foreground font-sans">
              自动准备流程
            </h2>
            <p className="mt-1 text-token-xs text-muted-foreground">
              环境检测不会阻塞继续使用；模型下载依赖网络，慢的时候会自动转入后台。
            </p>
          </div>

          {/* Hardware check items */}
          {checkItems.map((item) => (
            <div
              key={item.key}
              className={cn(
                'flex items-center gap-3 px-4 py-2.5',
                'border-b border-border/60',
              )}
            >
              {/* Icon */}
              <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-muted text-muted-foreground shrink-0">
                {item.icon}
              </div>

              {/* Label + detail */}
              <div className="flex-1 min-w-0">
                <p className="text-token-sm font-medium text-foreground">
                  {item.label}
                </p>
                <p className="text-token-xs text-muted-foreground mt-0.5">
                  {item.detail}
                </p>
              </div>

              {/* Status badge + label */}
              <div className="flex items-center gap-2 shrink-0">
                <span className={cn(STATUS_FONT_SIZE, statusTextClass(item.status))}>
                  {statusLabel(item.status)}
                </span>
                <div
                  className={cn(
                    'flex h-5 w-5 items-center justify-center rounded-full',
                    statusBadgeClass(item.status),
                  )}
                >
                  <StatusIcon status={item.status} />
                </div>
              </div>
            </div>
          ))}

          {/* ── Model download row ── */}
          <div
            className={cn(
              'px-4 py-3 border-b border-border/60 transition-colors',
              modelDownloaded && 'bg-status-success/[0.04]',
              downloadError && !modelDownloaded && 'bg-status-error/[0.04]',
            )}
          >
            <div className="flex items-center gap-3">
              {/* Icon — animates pulse when waiting, spin when downloading, scale when done */}
              <div
                className={cn(
                  'flex h-8 w-8 items-center justify-center rounded-lg shrink-0 transition-all duration-300',
                  modelDownloaded
                    ? 'bg-status-success/15 text-status-success scale-110'
                    : isDownloading || modelStatus?.status === 'Running'
                      ? 'bg-brand-orange/15 text-brand-orange'
                      : downloadError
                        ? 'bg-status-error/15 text-status-error'
                        : 'bg-muted text-muted-foreground',
                )}
              >
                {isDownloading || modelStatus?.status === 'Running' ? (
                  <Download className="h-4 w-4 animate-bounce" style={{ animationDuration: '1.6s' }} />
                ) : (
                  CHECK_ICONS.model
                )}
              </div>

              {/* Label + detail */}
              <div className="flex-1 min-w-0">
                <p className="text-token-sm font-medium text-foreground">
                  {MODEL_NAME}
                </p>
                <p className="text-token-xs text-muted-foreground mt-0.5">
                  {MODEL_DETAIL} · 约 {modelSizeMb} MB · 单次下载
                </p>
              </div>

              {/* Status badge + label */}
              <div className="flex items-center gap-2 shrink-0">
                <span className={cn(STATUS_FONT_SIZE, statusTextClass(modelRowStatus))}>
                  {modelStatusLabel}
                </span>
                <div
                  className={cn(
                    'flex h-5 w-5 items-center justify-center rounded-full transition-transform',
                    statusBadgeClass(modelRowStatus),
                    modelDownloaded && 'scale-110',
                  )}
                >
                  <StatusIcon status={modelRowStatus} />
                </div>
              </div>
            </div>

            {/* Pending shimmer line — before download starts */}
            {!isDownloading && !modelDownloaded && !downloadError && modelStatus?.status === 'Pending' && (
              <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-muted">
                <div
                  className="h-full w-1/3 rounded-full bg-brand-orange/40"
                  style={{
                    animation: 'shimmer 1.6s ease-in-out infinite',
                  }}
                />
              </div>
            )}

            {/* Progress bar — animated stripe while downloading; solid green when done */}
            {(isDownloading || modelStatus?.status === 'Running' || modelDownloaded) && (
              <div className="mt-3">
                <div className="flex items-baseline justify-between mb-1.5 gap-2">
                  <span className="text-token-xs text-muted-foreground tabular-nums">
                    {modelDownloaded
                      ? '模型已就绪 · 可继续下一步'
                      : totalBytes > 0
                        ? `${formatBytes(modelDownloadedBytes)} / ${formatBytes(modelTotalBytes)}`
                        : `后台准备中 · ${formatBytes(modelTotalBytes)} 总量`}
                  </span>
                  <span className={cn('text-token-xs tabular-nums font-medium', modelDownloaded && 'text-status-success')}>
                    {Math.round(modelPercent)}%
                  </span>
                </div>
                <div className="h-1.5 rounded-full bg-muted overflow-hidden">
                  <div
                    className={cn(
                      'h-full rounded-full transition-all duration-300 motion-reduce:transition-none',
                      modelDownloaded ? 'bg-status-success' : 'bg-brand-orange',
                    )}
                    style={{
                      width: `${modelPercent}%`,
                      backgroundImage:
                        !modelDownloaded && (isDownloading || modelStatus?.status === 'Running')
                          ? 'linear-gradient(45deg, rgba(255,255,255,0.3) 25%, transparent 25%, transparent 50%, rgba(255,255,255,0.3) 50%, rgba(255,255,255,0.3) 75%, transparent 75%, transparent)'
                          : undefined,
                      backgroundSize: '12px 12px',
                      animation:
                        !modelDownloaded && (isDownloading || modelStatus?.status === 'Running')
                          ? 'progressStripes 1.2s linear infinite'
                          : undefined,
                    }}
                  />
                </div>
                {/* Live speed + ETA — only while actively downloading */}
                {!modelDownloaded && isDownloading && bytesPerSec > 0 && (
                  <div className="mt-1.5 flex items-center gap-3 text-[10px] text-muted-foreground tabular-nums">
                    <span>速度 {formatBytes(bytesPerSec)}/s</span>
                    {eta > 0 && eta < 3600 && <span>剩余约 {formatEta(eta)}</span>}
                  </div>
                )}
              </div>
            )}

            {/* Failure / retry */}
            {(downloadError || modelStatus?.status === 'Fail') && !modelDownloaded && !isDownloading && (
              <div className="mt-3 flex items-start gap-2 rounded-lg bg-status-error/10 px-3 py-2">
                <span className="mt-0.5 text-status-error">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <circle cx="12" cy="12" r="10" /><line x1="12" y1="8" x2="12" y2="12" /><line x1="12" y1="16" x2="12.01" y2="16" />
                  </svg>
                </span>
                <div className="flex-1 min-w-0">
                  <p className="text-[11px] text-status-error font-medium">下载失败</p>
                  <p className="mt-0.5 text-[10.5px] text-muted-foreground line-clamp-2">
                    {downloadError ?? (modelStatus?.status === 'Fail' ? modelStatus.reason : '请检查网络后重试')}
                  </p>
                </div>
                <button
                  type="button"
                  onClick={() => downloadEmbeddedModel()}
                  className="flex shrink-0 items-center gap-1 rounded-lg bg-status-error px-2.5 py-1 text-[10.5px] font-semibold text-white hover:bg-status-error/90 transition-colors"
                >
                  <RefreshCw className="h-3 w-3" />
                  重试
                </button>
              </div>
            )}

            {downloadWatchdogExpired && !modelDownloaded && !downloadError && (
              <div className="mt-3 rounded-lg border border-brand-orange/25 bg-brand-orange/10 px-3 py-2">
                <p className="text-[11px] font-semibold text-brand-orange">
                  下载时间比预期更久
                </p>
                <p className="mt-0.5 text-[10.5px] text-muted-foreground">
                  这通常是 HuggingFace 网络较慢或首次连接建立中。你可以保持下载继续，也可以先进入下一步，之后在「设置 · 模型配置」继续下载。
                </p>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Bottom navigation */}
      <StepNavigation
        currentStep={2}
        onNext={onNext}
        onPrev={onPrev}
        canGoNext={canContinue}
        canGoPrev
        nextLabel={allPassed ? '预检完成 →' : '继续，稍后下载 →'}
        onSkip={canDeferModelDownload && !canContinue ? onNext : undefined}
        skipLabel="稍后在设置·模型配置中下载 →"
      />
    </OnboardingLayout>
  );
}
