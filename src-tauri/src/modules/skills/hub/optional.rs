#![allow(unused)]

//! Optional skills source adapter.
//!
//! Provides access to built-in optional skills.

use super::source::SkillSource;
use super::types::{HubError, HubResult, SkillBundle, SkillMeta};
use async_trait::async_trait;
use std::collections::HashMap;

/// Optional skills source.
///
/// Provides access to built-in optional skills.
/// These are built-in skills that users can optionally enable.
#[derive(Debug, Clone, Default)]
pub struct OptionalSkillSource;

impl OptionalSkillSource {
    /// Create a new OptionalSkillSource.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SkillSource for OptionalSkillSource {
    async fn search(&self, _query: &str, _limit: usize) -> HubResult<Vec<SkillMeta>> {
        // TODO: Implement optional skills search
        Ok(Vec::new())
    }

    async fn fetch(&self, _identifier: &str) -> HubResult<Option<SkillBundle>> {
        // TODO: Implement optional skills fetch
        Ok(None)
    }

    async fn inspect(&self, _identifier: &str) -> HubResult<Option<SkillMeta>> {
        // TODO: Implement optional skills inspect
        Ok(None)
    }

    fn source_id(&self) -> &'static str {
        "optional"
    }

    fn trust_level_for(
        &self,
        _identifier: &str,
    ) -> crate::modules::skills::guard::policy::TrustLevel {
        // Optional skills are built-in, so they're trusted
        crate::modules::skills::guard::policy::TrustLevel::Builtin
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_id() {
        let source = OptionalSkillSource::new();
        assert_eq!(source.source_id(), "optional");
    }

    #[test]
    fn test_trust_level_is_builtin() {
        let source = OptionalSkillSource::new();
        assert_eq!(
            source.trust_level_for("optional/some-skill"),
            crate::modules::skills::guard::policy::TrustLevel::Builtin
        );
    }
}
