//! TypeScript interfaces for Skills module.
//!
//! These types mirror the Rust backend types from:
//! - SkillsGuard Finding / ScanResult (guard/mod.rs)
//! - SkillMeta / SkillBundle (hub/types.rs)
//! - SkillConfigVar (config.rs)

/**
 * A security finding from SkillsGuard threat scan.
 * Mirrors the Rust `Finding` struct in guard/mod.rs.
 */
export interface SecurityFinding {
  /** Unique pattern identifier (e.g., "env_exfil_curl"). */
  pattern_id: string
  /** Severity level: "critical" | "high" | "medium" | "low". */
  severity: 'critical' | 'high' | 'medium' | 'low'
  /** Threat category (e.g., "exfiltration", "injection"). */
  category: string
  /** File path (relative to skill root). */
  file: string
  /** Line number where the pattern was found. */
  line: number
  /** The matched text (truncated to 120 chars). */
  match_text: string
  /** Human-readable description. */
  description: string
}

/**
 * Result of a full security scan.
 * Mirrors the Rust `ScanResult` struct in guard/mod.rs.
 */
export interface ScanResult {
  /** Skill directory name. */
  skill_name: string
  /** Source identifier (e.g., "community", "openai/skills"). */
  source: string
  /** Trust level: "builtin" | "trusted" | "community" | "agent-created" */
  trust_level: TrustLevel
  /** Overall verdict: "safe" | "caution" | "dangerous" */
  verdict: 'safe' | 'caution' | 'dangerous'
  /** All findings from the scan. */
  findings: SecurityFinding[]
  /** ISO 8601 timestamp of the scan. */
  scanned_at: string
  /** Human-readable summary. */
  summary: string
}

/**
 * Trust level of a skill source.
 * Mirrors the Rust `TrustLevel` enum in guard/policy.rs.
 */
export type TrustLevel = 'builtin' | 'trusted' | 'community' | 'agent-created'

/**
 * Metadata for a skill from a hub source.
 * Mirrors the Rust `SkillMeta` struct in hub/types.rs.
 */
export interface HubSearchResult {
  /** Skill name. */
  name: string
  /** Short description. */
  description: string
  /** Source identifier (e.g., "github", "clawhub"). */
  source: string
  /** Source-specific identifier (e.g., "owner/repo/path"). */
  identifier: string
  /** Trust level of this skill. */
  trust_level: TrustLevel
  /** Repository (e.g., "owner/repo"). */
  repo?: string
  /** Path within the repository. */
  path?: string
  /** Tags for categorization. */
  tags: string[]
}

/**
 * A complete skill bundle downloaded from a hub source.
 * Mirrors the Rust `SkillBundle` struct in hub/types.rs.
 */
export interface SkillBundle {
  /** Skill name. */
  name: string
  /** Files in the skill: relative_path -> content. */
  files: Record<string, string>
  /** Source identifier. */
  source: string
  /** Source-specific identifier. */
  identifier: string
  /** Trust level of this skill. */
  trust_level: TrustLevel
}

/**
 * Skill configuration variable.
 * Mirrors the Rust `SkillConfigVar` struct in config.rs.
 */
export interface SkillConfigVar {
  /** Variable key/name. */
  key: string
  /** Human-readable description. */
  description: string
  /** Default value if not set in config. */
  default?: string
}

/**
 * Frontmatter parsed from a SKILL.md file.
 */
export interface SkillFrontmatter {
  /** Skill name. */
  name?: string
  /** Skill description. */
  description?: string
  /** Version string. */
  version?: string
  /** License. */
  license?: string
  /** Supported platforms. */
  platforms?: string[]
  /** Config variables. */
  config?: SkillConfigVar[]
  /** Hermes metadata. */
  hermes?: {
    tags?: string[]
    related_skills?: string[]
    config?: SkillConfigVar[]
  }
}

/**
 * Severity level for security findings.
 */
export type Severity = 'critical' | 'high' | 'medium' | 'low'

/**
 * Map severity strings to display colors/classes.
 */
export const SEVERITY_COLORS: Record<Severity, { bg: string; text: string; border: string }> = {
  critical: { bg: 'bg-rose-100', text: 'text-rose-700', border: 'border-rose-300' },
  high: { bg: 'bg-orange-100', text: 'text-orange-700', border: 'border-orange-300' },
  medium: { bg: 'bg-amber-100', text: 'text-amber-700', border: 'border-amber-300' },
  low: { bg: 'bg-yellow-100', text: 'text-yellow-700', border: 'border-yellow-300' },
}

/**
 * Trust level display labels.
 */
export const TRUST_LABELS: Record<TrustLevel, string> = {
  builtin: '内置',
  trusted: '可信',
  community: '社区',
  'agent-created': 'Agent 创建',
}
