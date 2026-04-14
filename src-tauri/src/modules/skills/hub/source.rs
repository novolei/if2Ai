#![allow(unused)]

//! SkillSource trait — hub adapter interface.
//!
//! Ported from Hermes `tools/skills_hub.py` lines 252-274.

use super::types::{HubError, HubResult, SkillBundle, SkillMeta};
use async_trait::async_trait;
use std::sync::Arc;

/// Trait for skill source adapters (hub implementations).
///
/// Implement this trait to add new skill sources like GitHub, ClawHub, etc.
#[async_trait]
pub trait SkillSource: Send + Sync {
    /// Search for skills matching a query.
    async fn search(&self, query: &str, limit: usize) -> HubResult<Vec<SkillMeta>>;

    /// Fetch a complete skill bundle by identifier.
    async fn fetch(&self, identifier: &str) -> HubResult<Option<SkillBundle>>;

    /// Inspect a skill (get metadata only, without downloading files).
    async fn inspect(&self, identifier: &str) -> HubResult<Option<SkillMeta>>;

    /// Unique identifier for this source (e.g., "github", "clawhub").
    fn source_id(&self) -> &'static str;

    /// Determine the trust level for a skill by identifier.
    fn trust_level_for(
        &self,
        identifier: &str,
    ) -> crate::modules::skills::guard::policy::TrustLevel;
}

/// Boxed SkillSource for dynamic dispatch.
pub type BoxedSkillSource = Arc<dyn SkillSource>;

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full tests require a concrete implementation
    // These basic tests verify the trait compiles correctly

    #[test]
    fn test_source_id_doc_example() {
        // This is a compile-time check that the trait methods have correct signatures
        fn assert_send_sync<T: Send + Sync>() {}
        fn assert_trait_object<T: SkillSource>() {}
        // If it compiles, the trait is well-formed
        let _: Option<BoxedSkillSource> = None;
    }
}
