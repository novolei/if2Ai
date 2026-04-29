//! Skills module — skill discovery, management, and security scanning.
//!
//! This module provides the skill system including:
//! - SkillsGuard: threat scanner with 60+ patterns across 15 categories
//! - Skill management: CRUD operations for skill directories
//! - Skill hub: multi-source skill marketplace adapters
//! - Skill sync: manifest-based bundled skill synchronization
//! - Skill commands: slash command integration
//! - External dirs: external skills directories support
//! - Remote passthrough: remote backend environment variable passthrough
//! - Snapshot: skill export/import functionality

pub mod attenuation;
pub mod commands;
pub mod config;
pub mod domain_knowledge;
pub mod external_dirs;
pub mod guard;
pub mod hub;
pub mod manager;
pub mod remote_passthrough;
pub mod sedimentation;
pub mod snapshot;
pub mod sync;
pub mod vector_index;

/// Escape skill body content to prevent prompt injection via fake `<skill ...>`
/// XML tags. Mirrors Steward's `escape_skill_content`.
///
/// Apply this at EVERY injection point where untrusted skill body text is
/// written into a prompt contribution. Idempotent for already-escaped input
/// in the sense that a second pass is a no-op (the substrings the function
/// looks for no longer appear after the first pass).
pub fn escape_skill_content(raw: &str) -> String {
    raw.replace("</skill>", "<\\/skill>")
        .replace("<skill ", "<\\skill ")
}

#[cfg(test)]
mod escape_tests {
    use super::*;

    #[test]
    fn idempotent_on_already_escaped_input() {
        let once = escape_skill_content("</skill>");
        let twice = escape_skill_content(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn handles_unicode() {
        let raw = "中文 </skill> 測試";
        let out = escape_skill_content(raw);
        assert!(!out.contains("</skill>"));
        assert!(out.contains("中文"));
        assert!(out.contains("測試"));
    }
}
