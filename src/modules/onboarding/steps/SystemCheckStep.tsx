/**
 * SystemCheckStep — Step 2 of the 6-step Onboarding flow.
 *
 * Automatically runs system checks on mount (CPU, GPU, Node.js)
 * and displays Embedded model download UI. All checks must pass
 * before the user can proceed.
 *
 * Design reference: docs/references/onboarding-steps/SystemCheck.png
 */

import { useEffect, useCallback } from 'react';
import { InfoPanel, InfoCard } from '../components/InfoPanel';
import { OnboardingLayout } from '../components/OnboardingLayout';
import { StepHeader } from '../components/StepHeader';
import { StepNavigation } from '../components/StepNavigation';
import { CheckItem } from '../components/CheckItem';
import { DownloadProgress } from '../components/DownloadProgress';
import { useOnboarding } from '../hooks/useOnboarding';
import type { CheckStatus } from '../types';

interface SystemCheckStepProps {
  onNext: () => void;
  onPrev: () => void;
}

/** Determine if a check status counts as "passing" for the continue button. */
function isCheckPassing(status: CheckStatus): boolean {
  return status.status === 'Pass' || status.status === 'Fail';
}

export function SystemCheckStep({ onNext, onPrev }: SystemCheckStepProps) {
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

  const handleDownload = useCallback(() => {
    downloadEmbeddedModel();
  }, [downloadEmbeddedModel]);

  const report = systemReport;

  // Determine if all checks are complete and passing (excluding embedded model)
  const cpuDone = report ? isCheckPassing(report.cpu.status) : false;
  const gpuDone = report ? isCheckPassing(report.gpu.status) : false;
  const nodeDone = report ? isCheckPassing(report.nodejs.status) : false;

  // Embedded model is considered done if downloaded=true or status is Pass/Fail
  const modelDownloaded = report?.embedded_model.downloaded ?? false;
  const modelStatus = report?.embedded_model.status;
  const modelDone = modelDownloaded || (modelStatus && isCheckPassing(modelStatus));

  // All checks passing → enable continue button
  const allPassed = cpuDone && gpuDone && nodeDone && modelDone;

  // Whether we should show the download trigger button
  const showDownloadButton = report && !modelDownloaded && !isDownloading && modelStatus?.status !== 'Running';

  return (
    <OnboardingLayout
      rightPanel={
        <InfoPanel
          stepLabel="STEP 2: 系统预检"
          title="先体检，再安装。"
          bullets={[
            '检测 CPU/GPU/Node.js 环境',
            '自动下载 Embedded 模型',
            '全部通过后即可继续',
          ]}
        >
          {/* Current operation card: download progress */}
          <div className="mt-4">
            <InfoCard>
              <div className="mb-1.5">
                <span className="text-token-xs text-muted-foreground">
                  1/1
                </span>
              </div>
              {report ? (
                <DownloadProgress
                  percent={
                    modelDownloaded
                      ? 100
                      : isDownloading
                        ? downloadProgress
                        : report.embedded_model.progress
                          ? report.embedded_model.progress * 100
                          : 0
                  }
                  downloadedBytes={
                    report.embedded_model.progress
                      ? Math.round(
                          report.embedded_model.progress *
                            report.embedded_model.size_mb *
                            1024 *
                            1024,
                        )
                      : 0
                  }
                  totalBytes={report.embedded_model.size_mb * 1024 * 1024}
                  isDownloading={isDownloading || report.embedded_model.status.status === 'Running'}
                  isComplete={modelDownloaded}
                />
              ) : (
                <div className="h-2 rounded-full bg-muted/50 animate-pulse" />
              )}
            </InfoCard>
          </div>
        </InfoPanel>
      }
    >
      <StepHeader currentStep={2} title="先完成系统预检" />

      {/* Left content area */}
      <div className="flex-1 overflow-y-auto px-8 pb-6">
        <p className="text-token-sm text-muted-foreground mb-5 leading-relaxed">
          检查并安装必要的 Embedded 和 Embedding 模型，用于后续检索。
        </p>

        {/* Check items list */}
        <div className="flex flex-col gap-2">
          <CheckItem
            label="CPU"
            detail={
              report
                ? `${report.cpu.architecture} · ${report.cpu.cores} 核`
                : '检测中...'
            }
            status={report?.cpu.status ?? { status: 'Running' }}
          />
          <CheckItem
            label="GPU"
            detail={
              report
                ? report.gpu.available
                  ? report.gpu.name ?? '已检测到 GPU'
                  : '未检测到 GPU（将使用 CPU 模式）'
                : '检测中...'
            }
            status={report?.gpu.status ?? { status: 'Running' }}
          />
          <CheckItem
            label="Node.js"
            detail={
              report
                ? report.nodejs.installed
                  ? `版本 ${report.nodejs.version}`
                  : '未安装 Node.js（需要 18+）'
                : '检测中...'
            }
            status={report?.nodejs.status ?? { status: 'Running' }}
          />
        </div>

        {/* Embedded model download section */}
        <div className="mt-5">
          <DownloadProgress
            percent={
              modelDownloaded
                ? 100
                : isDownloading
                  ? downloadProgress
                  : report?.embedded_model.progress
                    ? report.embedded_model.progress * 100
                    : 0
            }
            downloadedBytes={
              report?.embedded_model.progress
                ? Math.round(
                    report.embedded_model.progress *
                      report.embedded_model.size_mb *
                      1024 *
                      1024,
                  )
                : 0
            }
            totalBytes={(report?.embedded_model.size_mb ?? 0) * 1024 * 1024}
            isDownloading={isDownloading || report?.embedded_model.status.status === 'Running'}
            isComplete={modelDownloaded}
          />
        </div>

        {/* Download trigger button */}
        {showDownloadButton && (
          <div className="mt-3 flex justify-center">
            <button
              type="button"
              onClick={handleDownload}
              className="px-5 py-2 rounded-md text-token-sm font-medium text-white bg-brand-orange hover:bg-brand-orange-dark"
            >
              开始下载 Embedded 模型
            </button>
          </div>
        )}
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
