#![allow(unused)]

//! GitHub skill source adapter.
//!
//! Ported from Hermes `tools/skills_hub.py` lines 284-405.

use super::source::SkillSource;
use super::types::{HubError, HubResult, SkillBundle, SkillMeta};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Default trusted skill repositories (Hermes DEFAULT_TAPS).
pub const DEFAULT_TAPS: &[&str] = &[
    "openai/skills",
    "anthropics/skills",
    "VoltAgent/awesome-agent-skills",
    "garrytan/gstack",
];

/// GitHub authentication methods.
#[derive(Debug, Clone, Default)]
pub enum GitHubAuth {
    /// Personal Access Token authentication.
    Pat(String),
    /// GitHub CLI authentication (checks `gh auth status`).
    GhCli,
    /// GitHub App authentication.
    GitHubApp {
        app_id: u64,
        installation_id: u64,
        private_key: String,
    },
    /// Anonymous access (read-only, rate limited).
    #[default]
    Anonymous,
}

impl GitHubAuth {
    /// Create a new GitHubAuth with a PAT.
    pub fn new(pat: Option<String>) -> Self {
        if let Some(token) = pat {
            if !token.is_empty() {
                return GitHubAuth::Pat(token);
            }
        }
        GitHubAuth::Anonymous
    }

    /// Get the authentication method name.
    pub fn auth_method(&self) -> &'static str {
        match self {
            GitHubAuth::Pat(_) => "pat",
            GitHubAuth::GhCli => "gh-cli",
            GitHubAuth::GitHubApp { .. } => "github-app",
            GitHubAuth::Anonymous => "anonymous",
        }
    }

    /// Check if authentication is available (not anonymous).
    pub fn is_authenticated(&self) -> bool {
        !matches!(self, GitHubAuth::Anonymous)
    }

    /// Get HTTP headers for requests.
    pub async fn get_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("Accept".into(), "application/vnd.github+json".into());

        match self {
            GitHubAuth::Pat(token) => {
                headers.insert("Authorization".into(), format!("Bearer {}", token));
            }
            GitHubAuth::GhCli => {
                // Try to get token from gh CLI
                if let Ok(token) = tokio::process::Command::new("gh")
                    .args(["auth", "token"])
                    .output()
                    .await
                {
                    if token.status.success() {
                        let token_str = String::from_utf8_lossy(&token.stdout).trim().to_string();
                        if !token_str.is_empty() {
                            headers.insert("Authorization".into(), format!("Bearer {}", token_str));
                        }
                    }
                }
            }
            GitHubAuth::GitHubApp { .. } => {
                // JWT generation would happen here in production
                // For now, we use a placeholder
                headers.insert("Authorization".into(), "Bearer <jwt>".into());
            }
            GitHubAuth::Anonymous => {
                // No auth header for anonymous
            }
        }

        headers
    }
}

/// GitHub-based skill source.
#[derive(Debug, Clone)]
pub struct GitHubSource {
    client: Client,
    auth: GitHubAuth,
    taps: Vec<String>,
}

impl Default for GitHubSource {
    fn default() -> Self {
        Self::new(
            GitHubAuth::default(),
            DEFAULT_TAPS.iter().map(|s| (*s).to_string()).collect(),
        )
    }
}

impl GitHubSource {
    /// Create a new GitHubSource with custom auth and trusted repos.
    pub fn new(auth: GitHubAuth, taps: Vec<String>) -> Self {
        let client = Client::builder()
            .user_agent("if2ai/1.0")
            .build()
            .expect("HTTP client must be constructible");

        Self { client, auth, taps }
    }

    /// Create a new GitHubSource with PAT authentication.
    pub fn with_pat(pat: String) -> Self {
        Self::new(
            GitHubAuth::Pat(pat),
            DEFAULT_TAPS.iter().map(|s| (*s).to_string()).collect(),
        )
    }

    /// Check if a repo is in the trusted taps list.
    fn is_trusted_repo(repo: &str) -> bool {
        DEFAULT_TAPS.contains(&repo)
    }

    /// Search GitHub for skills matching a query.
    async fn search_repos(&self, query: &str, limit: usize) -> HubResult<Vec<SkillMeta>> {
        let url = "https://api.github.com/search/code";
        let params = [
            ("q", format!("{} fork:true path:skills", query)),
            ("per_page", limit.to_string()),
        ];

        let headers = self.auth.get_headers().await;
        let mut request = self.client.get(url).query(&params);

        for (key, value) in headers {
            request = request.header(&key, &value);
        }

        let response = request.send().await?;
        let search_result: GitHubSearchResponse = response.json().await?;

        let mut skills = Vec::new();
        for item in search_result.items.iter().take(limit) {
            let repo = item.repository.full_name.clone();
            let path = item.path.clone();

            // Parse skill name from path (e.g., "skills/my-skill/SKILL.md")
            let name = path
                .strip_prefix("skills/")
                .and_then(|p| p.strip_suffix("/SKILL.md"))
                .or_else(|| path.strip_prefix("skills/"))
                .unwrap_or(&path)
                .split('/')
                .next()
                .unwrap_or(&path)
                .to_string();

            let trust_level = if Self::is_trusted_repo(&repo) {
                crate::modules::skills::guard::policy::TrustLevel::Trusted
            } else {
                crate::modules::skills::guard::policy::TrustLevel::Community
            };

            skills.push(SkillMeta {
                name,
                description: format!("Skill from {}", repo),
                source: "github".into(),
                identifier: format!("{}/{}", repo, path),
                trust_level,
                repo: Some(repo),
                path: Some(path),
                tags: vec![],
                extra: HashMap::new(),
            });
        }

        Ok(skills)
    }
}

#[derive(Debug, Deserialize)]
struct GitHubSearchResponse {
    items: Vec<GitHubSearchItem>,
}

#[derive(Debug, Deserialize)]
struct GitHubSearchItem {
    #[serde(rename = "repository")]
    repository: GitHubRepository,
    path: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRepository {
    full_name: String,
    description: Option<String>,
    stargazers_count: u64,
}

#[async_trait]
impl SkillSource for GitHubSource {
    async fn search(&self, query: &str, limit: usize) -> HubResult<Vec<SkillMeta>> {
        self.search_repos(query, limit).await
    }

    async fn fetch(&self, identifier: &str) -> HubResult<Option<SkillBundle>> {
        // Parse identifier: "owner/repo/path" or just "owner/repo/skill-name"
        let parts: Vec<&str> = identifier.split('/').collect();
        if parts.len() < 3 {
            return Ok(None);
        }

        let owner = parts[0];
        let repo = parts[1];
        let path = parts[2..].join("/");

        // Fetch the SKILL.md file to get skill metadata
        let skill_md_url = format!(
            "https://api.github.com/repos/{}/{}/contents/{}/SKILL.md",
            owner, repo, path
        );

        let headers = self.auth.get_headers().await;
        let mut request = self.client.get(&skill_md_url);

        for (key, value) in headers {
            request = request.header(&key, &value);
        }

        let response = match request.send().await {
            Ok(r) if r.status().is_success() => r,
            _ => return Ok(None),
        };

        let content_resp: GitHubContentResponse = response.json().await?;

        // Decode base64 content
        let content_bytes = base64_decode(&content_resp.content)?;
        let content = String::from_utf8(content_bytes.to_vec())
            .map_err(|e| HubError::Parse(format!("invalid UTF-8: {}", e)))?;

        let trust_level = if Self::is_trusted_repo(&format!("{}/{}", owner, repo)) {
            crate::modules::skills::guard::policy::TrustLevel::Trusted
        } else {
            crate::modules::skills::guard::policy::TrustLevel::Community
        };

        let name = path.split('/').next_back().unwrap_or(&path).to_string();

        let mut files = HashMap::new();
        files.insert("SKILL.md".into(), content.into_bytes());

        Ok(Some(SkillBundle {
            name,
            files,
            source: "github".into(),
            identifier: identifier.into(),
            trust_level,
            metadata: HashMap::new(),
        }))
    }

    async fn inspect(&self, identifier: &str) -> HubResult<Option<SkillMeta>> {
        // For GitHub, inspect is similar to fetch but only returns metadata
        let parts: Vec<&str> = identifier.split('/').collect();
        if parts.len() < 3 {
            return Ok(None);
        }

        let owner = parts[0];
        let repo = parts[1];
        let path = parts[2..].join("/");

        let skill_md_url = format!(
            "https://api.github.com/repos/{}/{}/contents/{}/SKILL.md",
            owner, repo, path
        );

        let headers = self.auth.get_headers().await;
        let mut request = self.client.get(&skill_md_url);

        for (key, value) in headers {
            request = request.header(&key, &value);
        }

        let response = match request.send().await {
            Ok(r) if r.status().is_success() => r,
            _ => return Ok(None),
        };

        let content_resp: GitHubContentResponse = response.json().await?;

        let trust_level = if Self::is_trusted_repo(&format!("{}/{}", owner, repo)) {
            crate::modules::skills::guard::policy::TrustLevel::Trusted
        } else {
            crate::modules::skills::guard::policy::TrustLevel::Community
        };

        let name = path.split('/').next_back().unwrap_or(&path).to_string();

        Ok(Some(SkillMeta {
            name,
            description: format!("Skill from {}/{}", owner, repo),
            source: "github".into(),
            identifier: identifier.into(),
            trust_level,
            repo: Some(format!("{}/{}", owner, repo)),
            path: Some(path),
            tags: vec![],
            extra: HashMap::new(),
        }))
    }

    fn source_id(&self) -> &'static str {
        "github"
    }

    fn trust_level_for(
        &self,
        identifier: &str,
    ) -> crate::modules::skills::guard::policy::TrustLevel {
        let parts: Vec<&str> = identifier.split('/').collect();
        if parts.len() >= 2 {
            let repo = format!("{}/{}", parts[0], parts[1]);
            if Self::is_trusted_repo(&repo) {
                return crate::modules::skills::guard::policy::TrustLevel::Trusted;
            }
        }
        crate::modules::skills::guard::policy::TrustLevel::Community
    }
}

#[derive(Debug, Deserialize)]
struct GitHubContentResponse {
    name: String,
    path: String,
    content: String, // Base64 encoded
    encoding: String,
}

/// Decode base64 content from GitHub API.
fn base64_decode(encoded: &str) -> HubResult<Vec<u8>> {
    // Remove newlines from base64 string
    let cleaned = encoded.replace('\n', "");
    let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &cleaned)
        .map_err(|e| HubError::Parse(format!("base64 decode error: {}", e)))?;
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_auth_methods() {
        let pat_auth = GitHubAuth::new(Some("test-token".into()));
        assert_eq!(pat_auth.auth_method(), "pat");
        assert!(pat_auth.is_authenticated());

        let anon_auth = GitHubAuth::new(None);
        assert_eq!(anon_auth.auth_method(), "anonymous");
        assert!(!anon_auth.is_authenticated());
    }

    #[test]
    fn test_is_trusted_repo() {
        assert!(GitHubSource::is_trusted_repo("openai/skills"));
        assert!(GitHubSource::is_trusted_repo("anthropics/skills"));
        assert!(GitHubSource::is_trusted_repo(
            "VoltAgent/awesome-agent-skills"
        ));
        assert!(GitHubSource::is_trusted_repo("garrytan/gstack"));
        assert!(!GitHubSource::is_trusted_repo("some-user/some-repo"));
    }

    #[tokio::test]
    async fn test_github_source_default() {
        let source = GitHubSource::default();
        assert_eq!(source.source_id(), "github");
        assert_eq!(
            source.trust_level_for("openai/skills/test"),
            crate::modules::skills::guard::policy::TrustLevel::Trusted
        );
        assert_eq!(
            source.trust_level_for("some-user/some-repo/test"),
            crate::modules::skills::guard::policy::TrustLevel::Community
        );
    }
}
