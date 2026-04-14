//! SkillSecurityReport — displays SkillsGuard scan findings with severity/category.
//!
//! Mirrors the Rust `SkillsGuard::format_report()` output but renders as a React component.
//!
//! Props interface matches the Rust `ScanResult` + `Finding` structs.

import { cn } from '@/lib/utils'
import { ShieldAlert, AlertTriangle, Info, CheckCircle } from 'lucide-react'
import type { SecurityFinding, ScanResult, Severity } from './types'
import { SEVERITY_COLORS } from './types'

interface SkillSecurityReportProps {
  /** The full scan result to display. */
  scanResult: ScanResult
  /** Optional className for styling */
  className?: string
  /** Whether to show the full report or compact version */
  compact?: boolean
}

const SEVERITY_ICONS: Record<Severity, typeof ShieldAlert> = {
  critical: ShieldAlert,
  high: AlertTriangle,
  medium: AlertTriangle,
  low: Info,
}

function SeverityIcon({ severity }: { severity: Severity }) {
  const Icon = SEVERITY_ICONS[severity]
  return <Icon className="h-4 w-4" />
}

function VerdictBadge({ verdict }: { verdict: ScanResult['verdict'] }) {
  const configs = {
    safe: {
      bg: 'bg-emerald-100',
      text: 'text-emerald-700',
      icon: CheckCircle,
      label: '安全',
    },
    caution: {
      bg: 'bg-amber-100',
      text: 'text-amber-700',
      icon: AlertTriangle,
      label: '注意',
    },
    dangerous: {
      bg: 'bg-rose-100',
      text: 'text-rose-700',
      icon: ShieldAlert,
      label: '危险',
    },
  }
  const config = configs[verdict]
  const Icon = config.icon

  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-medium',
        config.bg,
        config.text
      )}
    >
      <Icon className="h-3 w-3" />
      {config.label}
    </span>
  )
}

function FindingRow({ finding }: { finding: SecurityFinding }) {
  const colors = SEVERITY_COLORS[finding.severity as Severity] || SEVERITY_COLORS.low

  return (
    <div
      className={cn(
        'flex items-start gap-3 rounded-lg border p-3',
        colors.border,
        colors.bg
      )}
    >
      <SeverityIcon severity={finding.severity as Severity} />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="font-mono text-xs font-medium">{finding.pattern_id}</span>
          <span
            className={cn(
              'rounded px-1.5 py-0.5 text-[10px] font-medium uppercase',
              colors.bg,
              colors.text
            )}
          >
            {finding.severity}
          </span>
          <span className="text-xs text-muted-foreground">{finding.category}</span>
        </div>
        <p className="mt-1 text-sm text-foreground">{finding.description}</p>
        <div className="mt-1.5 flex items-center gap-3 text-xs text-muted-foreground">
          <span className="font-mono">
            {finding.file}:{finding.line}
          </span>
          {finding.match_text && (
            <code className="rounded bg-black/5 px-1 py-0.5 font-mono text-[11px]">
              {finding.match_text.length > 60
                ? `${finding.match_text.slice(0, 57)}...`
                : finding.match_text}
            </code>
          )}
        </div>
      </div>
    </div>
  )
}

export function SkillSecurityReport({
  scanResult,
  className,
  compact = false,
}: SkillSecurityReportProps) {
  const { skill_name, source, trust_level, verdict, findings, summary } = scanResult

  if (compact) {
    if (findings.length === 0) {
      return (
        <span className="inline-flex items-center gap-1 text-xs text-emerald-600">
          <CheckCircle className="h-3 w-3" />
          无安全问题
        </span>
      )
    }
    return (
      <span className="inline-flex items-center gap-1 text-xs text-amber-600">
        <AlertTriangle className="h-3 w-3" />
        {findings.length} 个问题
      </span>
    )
  }

  return (
    <div className={cn('space-y-3', className)}>
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <ShieldAlert className="h-5 w-5 text-muted-foreground" />
          <span className="font-medium">{skill_name}</span>
          <span className="text-xs text-muted-foreground">({source})</span>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-xs text-muted-foreground">{trust_level}</span>
          <VerdictBadge verdict={verdict} />
        </div>
      </div>

      {/* Summary */}
      <p className="text-sm text-muted-foreground">{summary}</p>

      {/* Findings */}
      {findings.length > 0 ? (
        <div className="space-y-2">
          <div className="text-xs font-medium text-muted-foreground">
            安全发现问题 ({findings.length})
          </div>
          {findings
            .sort((a, b) => {
              const order: Record<Severity, number> = {
                critical: 0,
                high: 1,
                medium: 2,
                low: 3,
              }
              return (order[a.severity as Severity] ?? 3) - (order[b.severity as Severity] ?? 3)
            })
            .map((finding, idx) => (
              <FindingRow key={idx} finding={finding} />
            ))}
        </div>
      ) : (
        <div className="flex items-center gap-2 rounded-lg border border-emerald-200 bg-emerald-50 p-3">
          <CheckCircle className="h-5 w-5 text-emerald-600" />
          <span className="text-sm text-emerald-700">未发现安全威胁</span>
        </div>
      )}
    </div>
  )
}

export default SkillSecurityReport
