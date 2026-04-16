/**
 * SecurityConfirmStep — Step 3 of the 6-step Onboarding flow.
 *
 * Displays 8 security risk disclosures that the user must read and
 * confirm (all 8 must be checked) before proceeding to provider setup.
 *
 * Design reference: docs/references/onboarding-steps/SecurityConfirm.png
 */

import { useState } from 'react';
import { cn } from '@/lib/utils';
import { RiskItem } from '../components/RiskItem';
import { InfoPanel, InfoCard } from '../components/InfoPanel';
import { OnboardingLayout } from '../components/OnboardingLayout';
import { StepHeader } from '../components/StepHeader';
import { StepNavigation } from '../components/StepNavigation';

interface SecurityConfirmStepProps {
  onConfirm: () => void;
  onPrev: () => void;
}

/** 8 security risk disclosure items. */
const RISK_ITEMS = [
  {
    number: 1,
    description:
      'if2AI 默认以个人模式运行，所有 AI 请求都会使用你配置的账号。',
  },
  {
    number: 2,
    description:
      '安装过程在 ~/.if2ai/ 下创建并修改配置文件。',
  },
  {
    number: 3,
    description:
      '安装完成后会自动启动各服务进程（LocalServer / Gateway）。',
  },
  {
    number: 4,
    description:
      '本应用会读取并存储你的 AI 凭据（API Key / OAuth Token）。',
  },
  {
    number: 5,
    description:
      '通讯 Bot 配置需要 App ID、App Secret 等敏感权限。',
  },
  {
    number: 6,
    description: 'if2AI 需要 Node.js 18+ 环境。',
  },
  {
    number: 7,
    description:
      '首次安装可能需要额外权限，或等待后台服务初始化。',
  },
  {
    number: 8,
    description: '我已阅读并理解以上说明，同意继续配置',
    isConfirmation: true,
  },
];

export function SecurityConfirmStep({ onConfirm, onPrev }: SecurityConfirmStepProps) {
  // Track which items are checked (items 0-6 auto-checked as "read", item 7 is the actual confirm)
  const [itemsChecked, setItemsChecked] = useState<boolean[]>(
    Array(RISK_ITEMS.length - 1).fill(true).concat([false]),
  );

  const allChecked = itemsChecked.every(Boolean);

  const handleItemCheck = (index: number, checked: boolean) => {
    setItemsChecked((prev) => {
      const next = [...prev];
      next[index] = checked;
      return next;
    });
  };

  return (
    <OnboardingLayout
      rightPanel={
        <InfoPanel
          stepLabel="STEP 3: 安全确认"
          title="先讲清规则，再开跑。"
          bullets={[
            '先确认权限，操作可继续',
            '权限安全审查',
            '开发者通道',
          ]}
        >
          {/* Security summary card */}
          <div className="mt-4">
            <InfoCard
              className={
                allChecked
                  ? 'border-status-success/30 bg-status-success-bg/20'
                  : ''
              }
            >
              <div className="flex items-center gap-2 mb-2">
                <div
                  className={cn(
                    'flex h-6 w-6 items-center justify-center rounded-full text-token-xs font-bold',
                    allChecked
                      ? 'bg-status-success text-white'
                      : 'bg-border text-muted-foreground',
                  )}
                >
                  {allChecked ? '✓' : '!'}
                </div>
                <span className="text-token-sm font-semibold text-white">
                  {allChecked ? '已确认，继续 →' : '请阅读并确认'}
                </span>
              </div>
              <div
                className="text-token-xs space-y-1"
                style={{ color: 'rgba(255,255,255,0.7)' }}
              >
                <div>• 可控权限，零滥用</div>
                <div>• 可控部署</div>
              </div>
            </InfoCard>
          </div>
        </InfoPanel>
      }
    >
      <StepHeader currentStep={3} title="请确认安全说明" />

      {/* Left content area */}
      <div className="flex-1 overflow-y-auto px-8 pb-6">
        <p className="text-token-sm text-muted-foreground mb-5 leading-relaxed">
          AI 配置涉及权限和第三方接入，请仔细阅读以下说明，完成勾选后再继续后续配置。
        </p>

        {/* Risk items list */}
        <div className="flex flex-col gap-2.5">
          {RISK_ITEMS.map((item, i) => (
            <RiskItem
              key={item.number}
              number={item.number}
              description={item.description}
              isConfirmation={item.isConfirmation}
              checked={itemsChecked[i]}
              onCheck={(checked) => handleItemCheck(i, checked)}
            />
          ))}
        </div>
      </div>

      {/* Bottom navigation */}
      <StepNavigation
        currentStep={3}
        onNext={onConfirm}
        onPrev={onPrev}
        canGoNext={allChecked}
        canGoPrev
        nextLabel="确认并继续 →"
      />
    </OnboardingLayout>
  );
}
