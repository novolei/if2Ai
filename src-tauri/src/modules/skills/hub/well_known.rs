#![allow(unused)]

//! Well-known skills source adapter.
//!
//! Provides access to skills published via /.well-known/skills/index.json.

use super::source::SkillSource;
use super::types::{HubError, HubResult, SkillBundle, SkillMeta};
use async_trait::async_trait;
use std::collections::HashMap;

/// Well-known skills source.
///
/// Provides access to skills published via a well-known endpoint.
#[derive(Debug, Clone, Default)]
pub struct WellKnownSource {
    index_url: String,
}

impl WellKnownSource {
    /// Create a new WellKnownSource with default URL.
    pub fn new() -> Self {
        Self {
            index_url: "/.well-known/skills/index.json".to_string(),
        }
    }

    /// Create a new WellKnownSource with custom index URL.
    pub fn with_url(url: String) -> Self {
        Self { index_url: url }
    }
}

#[async_trait]
impl SkillSource for WellKnownSource {
    async fn search(&self, _query: &str, _limit: usize) -> HubResult<Vec<SkillMeta>> {
        // TODO: Implement well-known search
        Ok(Vec::new())
    }

    async fn fetch(&self, _identifier: &str) -> HubResult<Option<SkillBundle>> {
        // TODO: Implement well-known fetch
        Ok(None)
    }

    async fn inspect(&self, _identifier: &str) -> HubResult<Option<SkillMeta>> {
        // TODO: Implement well-known inspect
        Ok(None)
    }

    fn source_id(&self) -> &'static str {
        "well-known"
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
        let source = WellKnownSource::new();
        assert_eq!(source.source_id(), "well-known");
    }

    #[test]
    fn test_trust_level() {
        let source = WellKnownSource::new();
        assert_eq!(
            source.trust_level_for("well-known/some-skill"),
            crate::modules::skills::guard::policy::TrustLevel::Community
        );
    }
}
