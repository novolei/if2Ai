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
import { useEffect, useRef } from 'react';
import { Cpu, Monitor, HardDrive, Download } from 'lucide-react';
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

interface SystemCheckStepProps {
  onNext: () => void;
  onPrev: () => void;
  onWindowDrag?: (event: ReactMouseEvent<HTMLElement>) => void;
}

/** Determine if a check status counts as "passing" for the continue button. */
function isCheckPassing(status: CheckStatus): boolean {
  return status.status === 'Pass' || status.status === 'Fail';
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

/** Format bytes to human-readable string. */
function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 MB';
  const mb = bytes / (1024 * 1024);
  return `${Math.round(mb)} MB`;
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
    downloadProgress,
    isDownloading,
    runSystemCheck,
    downloadEmbeddedModel,
  } = useOnboarding();

  // Auto-run system check on mount
  useEffect(() => {
    runSystemCheck();
  }, [runSystemCheck]);

  const report = systemReport;

  // Only model download is blocking. CPU/GPU/Memory are info-only (always Pass).
  const modelDownloaded = report?.embedded_model.downloaded ?? false;
  const modelStatus = report?.embedded_model.status;
  const modelDone = modelDownloaded || (modelStatus && isCheckPassing(modelStatus));

  // allPassed only depends on model download completion
  const allPassed = modelDone;

  // Auto-download model on mount if not already downloaded
  useEffect(() => {
    if (report && !modelDownloaded && !isDownloading && modelStatus?.status === 'Pending') {
      downloadEmbeddedModel();
    }
  }, [report, modelDownloaded, isDownloading, modelStatus, downloadEmbeddedModel]);

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

  const modelRowStatus: CheckStatus = modelDownloaded
    ? { status: 'Pass' }
    : isDownloading || modelStatus?.status === 'Running'
      ? { status: 'Running' }
      : modelStatus?.status === 'Fail'
        ? modelStatus
        : { status: 'Pending' };

  return (
    <OnboardingLayout
      rightPanel={
        <InfoPanel
          stepLabel="STEP 2: 系统预检"
          title="先体检，再安装。"
          bullets={[
            '检测 CPU/GPU/内存 信息',
            '自动下载 multilingual-e5-small 模型',
            '模型下载完成后即可继续',
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
                  {allPassed ? '系统就绪，继续 →' : '系统检测中…'}
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
                    模型: {statusLabel(modelRowStatus)}
                  </span>
                </div>
              </div>

              {/* Footer hint */}
              <div className="px-4 py-2 border-t" style={{ borderColor: 'rgba(255,255,255,0.1)' }}>
                <span className="text-[10px]" style={{ color: 'rgba(255,255,255,0.5)' }}>
                  {allPassed
                    ? '全部检测通过，可以进入下一步'
                    : '请等待检测完成，模型将自动下载'}
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
          先完成系统预检
        </h1>
      </div>

      {/* Left content area */}
      <div className="flex-1 overflow-y-auto px-8 pb-6">
        <p className="text-token-sm text-muted-foreground mb-5 leading-relaxed">
          检查并安装必要的 Embedded 和 Embedding 模型，用于后续检索。
        </p>

        {/* ── Unified check card ── */}
        <div className="rounded-xl border border-border bg-card overflow-hidden">
          {/* Card header */}
          <div className="px-4 py-3 border-b border-border">
            <h2 className="text-token-sm font-semibold text-foreground font-sans">
              环境检测 &amp; 模型下载
            </h2>
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
          <div className="px-4 py-3 border-b border-border/60">
            <div className="flex items-center gap-3">
              {/* Icon */}
              <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-muted text-muted-foreground shrink-0">
                {CHECK_ICONS.model}
              </div>

              {/* Label + detail */}
              <div className="flex-1 min-w-0">
                <p className="text-token-sm font-medium text-foreground">
                  {MODEL_NAME}
                </p>
                <p className="text-token-xs text-muted-foreground mt-0.5">
                  {MODEL_DETAIL}
                </p>
              </div>

              {/* Status badge + label */}
              <div className="flex items-center gap-2 shrink-0">
                <span className={cn(STATUS_FONT_SIZE, statusTextClass(modelRowStatus))}>
                  {statusLabel(modelRowStatus)}
                </span>
                <div
                  className={cn(
                    'flex h-5 w-5 items-center justify-center rounded-full',
                    statusBadgeClass(modelRowStatus),
                  )}
                >
                  <StatusIcon status={modelRowStatus} />
                </div>
              </div>
            </div>

            {/* Progress bar (only show when downloading or complete) */}
            {(isDownloading || modelStatus?.status === 'Running' || modelDownloaded) && (
              <div className="mt-3">
                <div className="flex items-center justify-between mb-1.5">
                  <span className="text-token-xs text-muted-foreground">
                    {modelDownloaded ? '模型已就绪' : `正在下载 · ${formatBytes(modelSizeMb * 1024 * 1024)} 总量`}
                  </span>
                  <span className={cn('text-token-xs tabular-nums', modelDownloaded && 'text-status-success')}>
                    {Math.round(modelPercent)}%
                  </span>
                </div>
                <div className="h-1.5 rounded-full bg-muted overflow-hidden">
                  <div
                    className={cn(
                      'h-full rounded-full transition-all duration-300 motion-reduce:transition-none',
                      modelDownloaded && 'bg-status-success',
                      (isDownloading || modelStatus?.status === 'Running') && 'bg-brand-orange',
                    )}
                    style={{ width: `${modelPercent}%` }}
                  />
                </div>
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
        canGoNext={allPassed}
        canGoPrev
        nextLabel="预检完成 →"
      />
    </OnboardingLayout>
  );
}
