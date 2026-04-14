#![allow(unused)]

//! skills.sh registry source adapter.
//!
//! Ported from Hermes `tools/skills_hub.py`.

use super::source::SkillSource;
use super::types::{HubError, HubResult, SkillBundle, SkillMeta};
use async_trait::async_trait;
use std::collections::HashMap;

/// skills.sh registry source.
///
/// Provides access to skills published on skills.sh registry.
#[derive(Debug, Clone, Default)]
pub struct SkillsShSource;

impl SkillsShSource {
    /// Create a new SkillsShSource.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SkillSource for SkillsShSource {
    async fn search(&self, _query: &str, _limit: usize) -> HubResult<Vec<SkillMeta>> {
        // TODO: Implement skills.sh search
        Ok(Vec::new())
    }

    async fn fetch(&self, _identifier: &str) -> HubResult<Option<SkillBundle>> {
        // TODO: Implement skills.sh fetch
        Ok(None)
    }

    async fn inspect(&self, _identifier: &str) -> HubResult<Option<SkillMeta>> {
        // TODO: Implement skills.sh inspect
        Ok(None)
    }

    fn source_id(&self) -> &'static str {
        "skills-sh"
    }

    fn trust_level_for(
        &self,
        _identifier: &str,
    ) -> crate::modules::skills::guard::policy::TrustLevel {
        crate::modules::skills::guard::policy::TrustLevel::Community
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_id() {
        let source = SkillsShSource::new();
        assert_eq!(source.source_id(), "skills-sh");
    }

    #[test]
    fn test_trust_level() {
        let source = SkillsShSource::new();
        assert_eq!(
            source.trust_level_for("skills-sh/some-skill"),
            crate::modules::skills::guard::policy::TrustLevel::Community
        );
    }
}
