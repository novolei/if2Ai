/**
 * ActivationChecklist — checklist of activation prerequisites.
 *
 * Displays the four pre-activation requirements (system check, security,
 * provider, channels) with pass/fail indicators. Used in the ActivationStep
 * left panel to show the configuration summary.
 */

import { Check, X, Loader2 } from 'lucide-react';
import { cn } from '@/lib/utils';
import type { ActivationChecklist } from '../types';

interface ActivationChecklistProps {
  checklist: ActivationChecklist;
  className?: string;
}

interface ChecklistItem {
  label: string;
  detail: string;
  value: boolean;
}

export function ActivationChecklist({
  checklist,
  className,
}: ActivationChecklistProps) {
  const items: ChecklistItem[] = [
    { label: '系统环境', detail: checklist.system_check ? '通过' : '未完成', value: checklist.system_check },
    { label: '安全确认', detail: checklist.security_confirmed ? '完成' : '未确认', value: checklist.security_confirmed },
    { label: '模型服务商', detail: checklist.provider_configured ? '已配置' : '未配置', value: checklist.provider_configured },
    { label: '通讯渠道', detail: checklist.channels_configured ? '已验证' : '未配置', value: checklist.channels_configured },
  ];

  return (
    <div className={cn('flex flex-col gap-2', className)}>
      {items.map((item) => (
        <div
          key={item.label}
          className={cn(
            'flex items-center gap-3 rounded-lg px-3.5 py-2.5',
            item.value ? 'bg-status-success-bg/30' : 'bg-muted/30',
          )}
        >
          {/* Status icon */}
          <div
            className={cn(
              'flex h-5 w-5 items-center justify-center rounded-full',
              item.value ? 'bg-status-success text-white' : 'bg-muted-foreground/20 text-muted-foreground/40',
            )}
          >
            {item.value ? <Check className="h-3 w-3" /> : <X className="h-3 w-3" />}
          </div>

          {/* Label */}
          <span
            className={cn(
              'text-token-sm font-medium flex-1',
              item.value ? 'text-foreground' : 'text-muted-foreground',
            )}
          >
            {item.label}
          </span>

          {/* Detail */}
          <span
            className={cn(
              'text-token-xs',
              item.value ? 'text-status-success' : 'text-muted-foreground/40',
            )}
          >
            {item.detail}
          </span>
        </div>
      ))}
    </div>
  );
}
