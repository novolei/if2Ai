#![allow(unused)]

//! ClawHub source adapter.
//!
//! Ported from Hermes `tools/skills_hub.py`.

use super::source::SkillSource;
use super::types::{HubError, HubResult, SkillBundle, SkillMeta};
use async_trait::async_trait;
use std::collections::HashMap;

/// ClawHub source adapter.
///
/// Provides access to skills published on clawhub.ai.
#[derive(Debug, Clone, Default)]
pub struct ClawHubSource {
    api_url: String,
}

impl ClawHubSource {
    /// Create a new ClawHubSource with default API URL.
    pub fn new() -> Self {
        Self {
            api_url: "https://api.clawhub.ai/v1".to_string(),
        }
    }

    /// Create a new ClawHubSource with custom API URL.
    pub fn with_url(api_url: String) -> Self {
        Self { api_url }
    }
}

#[async_trait]
impl SkillSource for ClawHubSource {
    async fn search(&self, _query: &str, _limit: usize) -> HubResult<Vec<SkillMeta>> {
        // TODO: Implement ClawHub search
        Ok(Vec::new())
    }

    async fn fetch(&self, _identifier: &str) -> HubResult<Option<SkillBundle>> {
        // TODO: Implement ClawHub fetch
        Ok(None)
    }

    async fn inspect(&self, _identifier: &str) -> HubResult<Option<SkillMeta>> {
        // TODO: Implement ClawHub inspect
        Ok(None)
    }

    fn source_id(&self) -> &'static str {
        "clawhub"
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
        let source = ClawHubSource::new();
        assert_eq!(source.source_id(), "clawhub");
    }

    #[test]
    fn test_trust_level() {
        let source = ClawHubSource::new();
        assert_eq!(
            source.trust_level_for("clawhub/some-skill"),
            crate::modules::skills::guard::policy::TrustLevel::Community
        );
    }
}
