#![allow(unused)]

//! Marketplace source adapters (Claude Marketplace + LobeHub).
//!
//! Ported from Hermes `tools/skills_hub.py`.

use super::source::SkillSource;
use super::types::{HubError, HubResult, SkillBundle, SkillMeta};
use async_trait::async_trait;
use std::collections::HashMap;

/// Claude Marketplace source adapter.
#[derive(Debug, Clone, Default)]
pub struct ClaudeMarketplaceSource;

impl ClaudeMarketplaceSource {
    /// Create a new ClaudeMarketplaceSource.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SkillSource for ClaudeMarketplaceSource {
    async fn search(&self, _query: &str, _limit: usize) -> HubResult<Vec<SkillMeta>> {
        // TODO: Implement Claude Marketplace search
        Ok(Vec::new())
    }

    async fn fetch(&self, _identifier: &str) -> HubResult<Option<SkillBundle>> {
        // TODO: Implement Claude Marketplace fetch
        Ok(None)
    }

    async fn inspect(&self, _identifier: &str) -> HubResult<Option<SkillMeta>> {
        // TODO: Implement Claude Marketplace inspect
        Ok(None)
    }

    fn source_id(&self) -> &'static str {
        "claude-marketplace"
    }

    fn trust_level_for(
        &self,
        _identifier: &str,
    ) -> crate::modules::skills::guard::policy::TrustLevel {
        // Claude Marketplace skills are treated as community
        crate::modules::skills::guard::policy::TrustLevel::Community
    }
}

/// LobeHub source adapter.
#[derive(Debug, Clone, Default)]
pub struct LobeHubSource;

impl LobeHubSource {
    /// Create a new LobeHubSource.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SkillSource for LobeHubSource {
    async fn search(&self, _query: &str, _limit: usize) -> HubResult<Vec<SkillMeta>> {
        // TODO: Implement LobeHub search
        Ok(Vec::new())
    }

    async fn fetch(&self, _identifier: &str) -> HubResult<Option<SkillBundle>> {
        // TODO: Implement LobeHub fetch
        Ok(None)
    }

    async fn inspect(&self, _identifier: &str) -> HubResult<Option<SkillMeta>> {
        // TODO: Implement LobeHub inspect
        Ok(None)
    }

    fn source_id(&self) -> &'static str {
        "lobehub"
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
    fn test_claude_marketplace_source_id() {
        let source = ClaudeMarketplaceSource::new();
        assert_eq!(source.source_id(), "claude-marketplace");
    }

    #[test]
    fn test_lobehub_source_id() {
        let source = LobeHubSource::new();
        assert_eq!(source.source_id(), "lobehub");
    }

    #[test]
    fn test_trust_levels() {
        let claude = ClaudeMarketplaceSource::new();
        let lobe = LobeHubSource::new();
        assert_eq!(
            claude.trust_level_for("claude-marketplace/skill"),
            crate::modules::skills::guard::policy::TrustLevel::Community
        );
        assert_eq!(
            lobe.trust_level_for("lobehub/skill"),
            crate::modules::skills::guard::policy::TrustLevel::Community
        );
    }
}
