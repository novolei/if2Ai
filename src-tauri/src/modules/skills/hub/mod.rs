#![allow(unused)]

//! Skills Hub — multi-source skill marketplace adapters.
//!
//! Ported from Hermes `tools/skills_hub.py`.
//!
//! Provides:
//! - SkillSource trait for implementing hub adapters
//! - Multiple source adapters: GitHub, skills.sh, ClawHub, Claude Marketplace, LobeHub, WellKnown, Optional
//! - HubError and result types

pub mod clawhub;
pub mod github;
pub mod marketplace;
pub mod optional;
pub mod skills_sh;
pub mod source;
pub mod types;
pub mod well_known;

pub use clawhub::ClawHubSource;
pub use github::{GitHubAuth, GitHubSource, DEFAULT_TAPS};
pub use marketplace::{ClaudeMarketplaceSource, LobeHubSource};
pub use optional::OptionalSkillSource;
pub use skills_sh::SkillsShSource;
pub use source::{BoxedSkillSource, SkillSource};
pub use types::{HubError, HubResult, SkillBundle, SkillMeta};
pub use well_known::WellKnownSource;

use std::sync::Arc;

/// Create a router that manages all skill sources.
///
/// Returns a vector of all available skill sources.
pub fn create_source_router() -> Vec<BoxedSkillSource> {
    vec![
        Arc::new(GitHubSource::default()) as BoxedSkillSource,
        Arc::new(SkillsShSource::new()) as BoxedSkillSource,
        Arc::new(ClawHubSource::new()) as BoxedSkillSource,
        Arc::new(ClaudeMarketplaceSource::new()) as BoxedSkillSource,
        Arc::new(LobeHubSource::new()) as BoxedSkillSource,
        Arc::new(WellKnownSource::new()) as BoxedSkillSource,
        Arc::new(OptionalSkillSource::new()) as BoxedSkillSource,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_taps() {
        assert_eq!(DEFAULT_TAPS.len(), 4);
        assert!(DEFAULT_TAPS.contains(&"openai/skills"));
        assert!(DEFAULT_TAPS.contains(&"anthropics/skills"));
    }

    #[test]
    fn test_create_source_router() {
        let sources = create_source_router();
        assert_eq!(sources.len(), 7);

        let source_ids: Vec<&str> = sources.iter().map(|s| s.source_id()).collect();
        assert!(source_ids.contains(&"github"));
        assert!(source_ids.contains(&"skills-sh"));
        assert!(source_ids.contains(&"clawhub"));
        assert!(source_ids.contains(&"claude-marketplace"));
        assert!(source_ids.contains(&"lobehub"));
        assert!(source_ids.contains(&"well-known"));
        assert!(source_ids.contains(&"optional"));
    }

    #[test]
    fn test_trust_levels_by_source() {
        let sources = create_source_router();

        // GitHub - trusted repos return Trusted, others return Community
        let github_source = sources.iter().find(|s| s.source_id() == "github").unwrap();
        assert_eq!(
            github_source.trust_level_for("openai/skills/test"),
            crate::modules::skills::guard::policy::TrustLevel::Trusted
        );
        assert_eq!(
            github_source.trust_level_for("some-user/some-repo/test"),
            crate::modules::skills::guard::policy::TrustLevel::Community
        );

        // Optional - always Builtin
        let optional_source = sources
            .iter()
            .find(|s| s.source_id() == "optional")
            .unwrap();
        assert_eq!(
            optional_source.trust_level_for("optional/any-skill"),
            crate::modules::skills::guard::policy::TrustLevel::Builtin
        );
    }
}
