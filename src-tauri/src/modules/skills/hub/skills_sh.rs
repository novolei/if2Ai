#![allow(unused)]

//! skills.sh registry source adapter.
//!
//! Ported from Hermes `tools/skills_hub.py`.

use super::github::GitHubSource;
use super::source::SkillSource;
use super::types::{HubError, HubResult, SkillBundle, SkillMeta};
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;

/// skills.sh registry source.
///
/// Provides access to skills published on skills.sh registry.
#[derive(Debug, Clone)]
pub struct SkillsShSource {
    client: Client,
    github: GitHubSource,
}

impl Default for SkillsShSource {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillsShSource {
    /// Create a new SkillsShSource.
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("if2ai/1.0")
            .build()
            .expect("HTTP client must be constructible");
        Self {
            client,
            github: GitHubSource::default(),
        }
    }

    /// Base URL for skills.sh.
    const BASE_URL: &'static str = "https://skills.sh";

    /// Fetch the detail page for an identifier.
    async fn fetch_detail_page(&self, identifier: &str) -> HubResult<Option<String>> {
        let url = format!("{}/{}", Self::BASE_URL, identifier);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| HubError::Network(e.to_string()))?;
        if response.status().as_u16() != 200 {
            return Ok(None);
        }
        let body = response
            .text()
            .await
            .map_err(|e| HubError::Network(e.to_string()))?;
        Ok(Some(body))
    }

    /// Extract GitHub repo and skill name from the install command in the HTML.
    fn parse_install_command(html: &str) -> Option<(String, String)> {
        // Match npx skills add <repo> or npx skills add <url>
        // e.g., "npx skills add github.com/owner/repo --skill my-skill"
        // or "npx skills add https://github.com/owner/repo"
        // The repo capture should not include trailing punctuation like </p> or "
        let npx_re = regex::Regex::new(
            r#"npx\s+skills\s+add\s+(?:https?://github\.com/)?(?P<repo>[a-zA-Z0-9_.-]+/[a-zA-Z0-9_.-]+)(?:\s+--skill\s+(?P<skill>[^\s<">]+))?"#
        ).ok()?;

        let caps = npx_re.captures(html)?;
        let repo = caps.name("repo")?.as_str().to_string();
        let skill_from_caps = caps.name("skill").map(|s| s.as_str().to_string());
        let skill = skill_from_caps.unwrap_or_else(|| {
            repo.split('/')
                .next_back()
                .map(String::from)
                .unwrap_or_else(|| repo.clone())
        });
        Some((repo, skill))
    }

    /// Try multiple candidate paths to find the skill.
    async fn fetch_from_github(
        &self,
        repo: &str,
        skill_name: &str,
    ) -> HubResult<Option<SkillBundle>> {
        // Standard skill paths to try
        let candidate_paths = [
            format!("skills/{}", skill_name),
            format!(".agents/skills/{}", skill_name),
            format!(".claude/skills/{}", skill_name),
            skill_name.to_string(),
        ];

        for path in &candidate_paths {
            let identifier = format!("{}/{}", repo, path);
            if let Some(bundle) = self.github.fetch(&identifier).await? {
                return Ok(Some(bundle));
            }
        }

        // Try the skill name directly (for skills in repo root or non-standard paths)
        let tree_identifier = format!("{}/{}", repo, skill_name);
        if let Some(bundle) = self.github.fetch(&tree_identifier).await? {
            return Ok(Some(bundle));
        }

        Ok(None)
    }
}

#[async_trait]
impl SkillSource for SkillsShSource {
    async fn search(&self, query: &str, limit: usize) -> HubResult<Vec<SkillMeta>> {
        // Use the skills.sh API
        let url = format!("{}/api/search", Self::BASE_URL);
        let params = [("q", query), ("limit", &limit.to_string())];

        let response = self
            .client
            .get(&url)
            .query(&params)
            .send()
            .await
            .map_err(|e| HubError::Network(e.to_string()))?;

        if response.status().as_u16() != 200 {
            return Ok(Vec::new());
        }

        let data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| HubError::Parse(e.to_string()))?;

        let skills = data
            .get("skills")
            .and_then(|s| s.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        let name = item.get("name")?.as_str()?.to_string();
                        let repo = item.get("source")?.as_str()?.to_string();
                        let skill_id = item.get("skillId")?.as_str()?.to_string();
                        Some(SkillMeta {
                            name,
                            description: format!("From {}", repo),
                            source: "skills.sh".into(),
                            identifier: format!("{}/{}", repo, skill_id),
                            trust_level:
                                crate::modules::skills::guard::policy::TrustLevel::Community,
                            repo: Some(repo),
                            path: Some(skill_id),
                            tags: vec![],
                            extra: HashMap::new(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(skills)
    }

    async fn fetch(&self, identifier: &str) -> HubResult<Option<SkillBundle>> {
        // Fetch the detail page to discover the GitHub repo
        let detail = match self.fetch_detail_page(identifier).await? {
            Some(d) => d,
            None => return Ok(None),
        };

        // Parse the install command to find GitHub repo and skill name
        let (repo, skill_name) = match Self::parse_install_command(&detail) {
            Some(pair) => pair,
            None => return Ok(None),
        };

        // Try to fetch from GitHub with discovered repo and skill name
        let mut bundle = self.fetch_from_github(&repo, &skill_name).await?;

        // If successful, update source to skills.sh
        if let Some(ref mut b) = bundle {
            b.source = "skills.sh".into();
            b.identifier = identifier.into();
        }

        Ok(bundle)
    }

    async fn inspect(&self, identifier: &str) -> HubResult<Option<SkillMeta>> {
        // Fetch detail page
        let detail = match self.fetch_detail_page(identifier).await? {
            Some(d) => d,
            None => return Ok(None),
        };

        // Parse install command
        let (repo, skill_name) = match Self::parse_install_command(&detail) {
            Some(pair) => pair,
            None => return Ok(None),
        };

        // Delegate to GitHub for inspection
        let github_id = format!("{}/skills/{}", repo, skill_name);
        let meta = self.github.inspect(&github_id).await?;

        if let Some(mut m) = meta {
            m.source = "skills.sh".into();
            m.identifier = identifier.into();
            Ok(Some(m))
        } else {
            Ok(None)
        }
    }

    fn source_id(&self) -> &'static str {
        "skills-sh"
    }

    fn trust_level_for(
        &self,
        identifier: &str,
    ) -> crate::modules::skills::guard::policy::TrustLevel {
        // Delegate to GitHub to check if repo is trusted
        self.github.trust_level_for(identifier)
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
    fn test_parse_install_command() {
        let html = r#"<p>Run: npx skills add owner/repo --skill my-skill</p>"#;
        let result = SkillsShSource::parse_install_command(html);
        assert!(result.is_some());
        let (repo, skill) = result.unwrap();
        assert_eq!(repo, "owner/repo");
        assert_eq!(skill, "my-skill");
    }

    #[test]
    fn test_parse_install_command_url() {
        let html = r#"<p>Run: npx skills add https://github.com/owner/repo</p>"#;
        let result = SkillsShSource::parse_install_command(html);
        assert!(result.is_some());
        let (repo, skill) = result.unwrap();
        assert_eq!(repo, "owner/repo");
        assert_eq!(skill, "repo");
    }

    #[test]
    fn test_trust_level() {
        let source = SkillsShSource::new();
        assert_eq!(
            source.trust_level_for("anthropics/skills/some-skill"),
            crate::modules::skills::guard::policy::TrustLevel::Trusted
        );
        assert_eq!(
            source.trust_level_for("someone/some-skill"),
            crate::modules::skills::guard::policy::TrustLevel::Community
        );
    }
}
