#![allow(unused)]

//! Install policy — trust-level-aware installation decisions.
//!
//! Ported from Hermes `tools/skills_guard.py` lines 39-47.

/// Trust level of a skill source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrustLevel {
    /// Built-in skills packaged with the app — never scanned, always trusted.
    Builtin,
    /// Trusted sources: openai/skills, anthropics/skills — caution verdicts allowed.
    Trusted,
    /// Community sources — any findings result in blocking.
    Community,
    /// Agent-created skills — dangerous verdict results in "ask" (not block).
    AgentCreated,
}

impl std::fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrustLevel::Builtin => write!(f, "builtin"),
            TrustLevel::Trusted => write!(f, "trusted"),
            TrustLevel::Community => write!(f, "community"),
            TrustLevel::AgentCreated => write!(f, "agent-created"),
        }
    }
}

impl TrustLevel {
    /// Parse a trust level from a string (matching Hermes source identifiers).
    pub fn from_source_identifier(source: &str) -> Self {
        // Normalize: remove leading/trailing whitespace and slashes
        let source = source.trim().trim_start_matches('/').trim_end_matches('/');

        if source == "builtin" {
            return TrustLevel::Builtin;
        }
        if source == "agent-created" || source == "agent_created" {
            return TrustLevel::AgentCreated;
        }
        if is_trusted_repo(source) {
            return TrustLevel::Trusted;
        }
        TrustLevel::Community
    }
}

/// Trusted repository identifiers (matching Hermes TRUSTED_REPOS).
pub const TRUSTED_REPOS: &[&str] = &["openai/skills", "anthropics/skills"];

/// Check if a repository identifier is trusted.
pub fn is_trusted_repo(repo: &str) -> bool {
    let normalized = repo.trim().trim_start_matches('/').trim_end_matches('/');
    TRUSTED_REPOS.contains(&normalized)
}

/// Verdict of a security scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// No threats found.
    Safe,
    /// Potential threat — requires user attention.
    Caution,
    /// Dangerous — high-severity threats detected.
    Dangerous,
}

impl std::fmt::Display for Verdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Verdict::Safe => write!(f, "safe"),
            Verdict::Caution => write!(f, "caution"),
            Verdict::Dangerous => write!(f, "dangerous"),
        }
    }
}

/// Installation action determined by policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallAction {
    /// Skill is allowed — proceed with installation.
    Allow,
    /// Skill is allowed but with a warning to the user.
    AllowWithWarning,
    /// Skill is blocked — do not install.
    Block,
    /// Skill requires user confirmation before installation.
    /// This is specific to agent-created skills with dangerous verdicts.
    Ask,
}

impl std::fmt::Display for InstallAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallAction::Allow => write!(f, "Allow"),
            InstallAction::AllowWithWarning => write!(f, "AllowWithWarning"),
            InstallAction::Block => write!(f, "Block"),
            InstallAction::Ask => write!(f, "Ask"),
        }
    }
}

/// Install policy table matching Hermes exactly.
///
/// Policy mapping: trust_level -> verdict_index -> action
/// verdict_index: 0 = safe, 1 = caution, 2 = dangerous
///
/// Hermès:
///
/// ```python
/// INSTALL_POLICY = {
///     "builtin":       ("allow",  "allow",   "allow"),
///     "trusted":       ("allow",  "allow",   "block"),
///     "community":     ("allow",  "block",   "block"),
///     "agent-created": ("allow",  "allow",   "ask"),
/// }
/// ```
pub struct InstallPolicy;

impl InstallPolicy {
    /// Determine the install action for a given trust level and verdict.
    pub fn get_action(trust_level: TrustLevel, verdict: Verdict) -> InstallAction {
        let verdict_idx = match verdict {
            Verdict::Safe => 0,
            Verdict::Caution => 1,
            Verdict::Dangerous => 2,
        };

        // Policy rows: [safe_idx, caution_idx, dangerous_idx]
        match trust_level {
            TrustLevel::Builtin => {
                let policy = ["allow", "allow", "allow"];
                Self::parse_action(policy[verdict_idx])
            }
            TrustLevel::Trusted => {
                let policy = ["allow", "allow", "block"];
                Self::parse_action(policy[verdict_idx])
            }
            TrustLevel::Community => {
                let policy = ["allow", "block", "block"];
                Self::parse_action(policy[verdict_idx])
            }
            TrustLevel::AgentCreated => {
                // Key: agent-created + dangerous = "ask" (not block)
                let policy = ["allow", "allow", "ask"];
                Self::parse_action(policy[verdict_idx])
            }
        }
    }

    fn parse_action(s: &str) -> InstallAction {
        match s {
            "allow" => InstallAction::Allow,
            "block" => InstallAction::Block,
            "ask" => InstallAction::Ask,
            _ => InstallAction::Block,
        }
    }

    /// Determine if a skill should be allowed to install, returning (allowed, reason).
    ///
    /// If `force` is true, overrides blocked policy decisions (Hermes `--force` flag).
    pub fn should_allow_install(
        trust_level: TrustLevel,
        verdict: Verdict,
        finding_count: usize,
        force: bool,
    ) -> (Option<bool>, String) {
        let action = Self::get_action(trust_level, verdict);

        match action {
            InstallAction::Allow => (
                Some(true),
                format!("Allowed ({} source, {} verdict)", trust_level, verdict),
            ),
            InstallAction::AllowWithWarning => (
                Some(true),
                format!(
                    "Allowed with warning ({} source, {} verdict)",
                    trust_level, verdict
                ),
            ),
            InstallAction::Block => {
                if force {
                    (
                        Some(true),
                        format!(
                            "Force-installed despite {} verdict ({} findings)",
                            verdict, finding_count
                        ),
                    )
                } else {
                    (
                        Some(false),
                        format!(
                            "Blocked ({} source + {} verdict, {} findings). Use --force to override.",
                            trust_level, verdict, finding_count
                        ),
                    )
                }
            }
            InstallAction::Ask => {
                // Returns None to signal "needs user confirmation"
                (
                    None,
                    format!(
                        "Requires confirmation ({} source + {} verdict, {} findings)",
                        trust_level, verdict, finding_count
                    ),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trust_level_display() {
        assert_eq!(TrustLevel::Builtin.to_string(), "builtin");
        assert_eq!(TrustLevel::Trusted.to_string(), "trusted");
        assert_eq!(TrustLevel::Community.to_string(), "community");
        assert_eq!(TrustLevel::AgentCreated.to_string(), "agent-created");
    }

    #[test]
    fn test_trust_level_from_identifier() {
        assert_eq!(
            TrustLevel::from_source_identifier("openai/skills"),
            TrustLevel::Trusted
        );
        assert_eq!(
            TrustLevel::from_source_identifier("anthropics/skills"),
            TrustLevel::Trusted
        );
        assert_eq!(
            TrustLevel::from_source_identifier("community/some-skill"),
            TrustLevel::Community
        );
        assert_eq!(
            TrustLevel::from_source_identifier("builtin"),
            TrustLevel::Builtin
        );
        assert_eq!(
            TrustLevel::from_source_identifier("agent-created"),
            TrustLevel::AgentCreated
        );
    }

    #[test]
    fn test_is_trusted_repo() {
        assert!(is_trusted_repo("openai/skills"));
        assert!(is_trusted_repo("anthropics/skills"));
        assert!(!is_trusted_repo("some-other/repo"));
        assert!(!is_trusted_repo("clawhub/skill"));
    }

    #[test]
    fn test_install_policy_builtin() {
        // builtin: always allow regardless of verdict
        assert_eq!(
            InstallPolicy::get_action(TrustLevel::Builtin, Verdict::Safe),
            InstallAction::Allow
        );
        assert_eq!(
            InstallPolicy::get_action(TrustLevel::Builtin, Verdict::Caution),
            InstallAction::Allow
        );
        assert_eq!(
            InstallPolicy::get_action(TrustLevel::Builtin, Verdict::Dangerous),
            InstallAction::Allow
        );
    }

    #[test]
    fn test_install_policy_trusted() {
        // trusted: safe=allow, caution=allow, dangerous=block
        assert_eq!(
            InstallPolicy::get_action(TrustLevel::Trusted, Verdict::Safe),
            InstallAction::Allow
        );
        assert_eq!(
            InstallPolicy::get_action(TrustLevel::Trusted, Verdict::Caution),
            InstallAction::Allow
        );
        assert_eq!(
            InstallPolicy::get_action(TrustLevel::Trusted, Verdict::Dangerous),
            InstallAction::Block
        );
    }

    #[test]
    fn test_install_policy_community() {
        // community: safe=allow, caution=block, dangerous=block
        assert_eq!(
            InstallPolicy::get_action(TrustLevel::Community, Verdict::Safe),
            InstallAction::Allow
        );
        assert_eq!(
            InstallPolicy::get_action(TrustLevel::Community, Verdict::Caution),
            InstallAction::Block
        );
        assert_eq!(
            InstallPolicy::get_action(TrustLevel::Community, Verdict::Dangerous),
            InstallAction::Block
        );
    }

    #[test]
    fn test_install_policy_agent_created() {
        // agent-created: safe=allow, caution=allow, dangerous=ask (NOT block)
        assert_eq!(
            InstallPolicy::get_action(TrustLevel::AgentCreated, Verdict::Safe),
            InstallAction::Allow
        );
        assert_eq!(
            InstallPolicy::get_action(TrustLevel::AgentCreated, Verdict::Caution),
            InstallAction::Allow
        );
        assert_eq!(
            InstallPolicy::get_action(TrustLevel::AgentCreated, Verdict::Dangerous),
            InstallAction::Ask
        );
    }

    #[test]
    fn test_should_allow_install_blocked() {
        let (allowed, reason) = InstallPolicy::should_allow_install(
            TrustLevel::Community,
            Verdict::Dangerous,
            5,
            false,
        );
        assert_eq!(allowed, Some(false));
        assert!(reason.contains("Blocked"));
    }

    #[test]
    fn test_should_allow_install_force_override() {
        let (allowed, reason) =
            InstallPolicy::should_allow_install(TrustLevel::Community, Verdict::Dangerous, 5, true);
        assert_eq!(allowed, Some(true));
        assert!(reason.contains("Force-installed"));
    }

    #[test]
    fn test_should_allow_install_ask() {
        let (allowed, reason) = InstallPolicy::should_allow_install(
            TrustLevel::AgentCreated,
            Verdict::Dangerous,
            3,
            false,
        );
        assert_eq!(allowed, None); // None signals "ask"
        assert!(reason.contains("Requires confirmation"));
    }

    #[test]
    fn test_verdict_display() {
        assert_eq!(Verdict::Safe.to_string(), "safe");
        assert_eq!(Verdict::Caution.to_string(), "caution");
        assert_eq!(Verdict::Dangerous.to_string(), "dangerous");
    }
}
