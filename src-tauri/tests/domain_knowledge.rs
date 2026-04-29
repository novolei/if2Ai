//! FEAT-DK-001..003 — Domain knowledge + working checkpoint tests.
//!
//! Each Pack appends its own marked section.

use if2ai_backend::modules::skills::domain_knowledge::{
    lookup_domain_knowledge, mock::MockKnowledgeStore, DomainKnowledgeEntry, DomainKnowledgeKind,
    KnowledgeAuthor, KnowledgeStore, SOPStep, SelectorEntry, SelectorStability,
};

fn website_entry(domain: &str, gotcha: &str) -> DomainKnowledgeEntry {
    DomainKnowledgeEntry::new(
        DomainKnowledgeKind::WebsiteDomain {
            domain: domain.to_string(),
            url_patterns: vec![format!("https://{domain}/*")],
            selectors: vec![SelectorEntry {
                selector: "div.main".to_string(),
                purpose: "main panel".to_string(),
                stability: SelectorStability::Stable,
                last_verified: None,
            }],
            gotchas: vec![gotcha.to_string()],
        },
        KnowledgeAuthor::User,
        0.9,
    )
}

fn sop_entry(task_type: &str) -> DomainKnowledgeEntry {
    DomainKnowledgeEntry::new(
        DomainKnowledgeKind::TaskSOP {
            task_type: task_type.to_string(),
            prerequisites: vec!["clean working dir".to_string()],
            key_pitfalls: vec!["watch out for line endings".to_string()],
            execution_steps: vec![SOPStep {
                tool_name: "bash".to_string(),
                description: "stat the file".to_string(),
                args_template: None,
                expected_result: "exit 0".to_string(),
            }],
        },
        KnowledgeAuthor::Agent {
            session_id: "test".to_string(),
        },
        0.7,
    )
}

// ---------------------------------------------------------------------------
// FEAT-DK-001: Domain Knowledge Repository
// ---------------------------------------------------------------------------

#[tokio::test]
async fn upsert_then_lookup_returns_entry() {
    let store = MockKnowledgeStore::new();
    let entry = website_entry("amazon.com", "search bar reloads page");
    store.upsert(entry.clone()).await;

    let hits = lookup_domain_knowledge("amazon", None, &store).await;
    assert_eq!(hits.len(), 1, "lookup must surface the upserted entry");
    assert_eq!(hits[0].id, entry.id);
}

#[tokio::test]
async fn kind_filter_excludes_other_kinds() {
    let store = MockKnowledgeStore::new();
    store.upsert(website_entry("github.com", "rate limit hits at 60/h")).await;
    store.upsert(sop_entry("file_batch_rename")).await;

    let websites = lookup_domain_knowledge("", Some("website_domain"), &store).await;
    assert_eq!(websites.len(), 1);
    assert_eq!(websites[0].kind.label(), "website_domain");

    let sops = lookup_domain_knowledge("", Some("task_sop"), &store).await;
    assert_eq!(sops.len(), 1);
    assert_eq!(sops[0].kind.label(), "task_sop");
}

#[tokio::test]
async fn empty_store_returns_empty() {
    let store = MockKnowledgeStore::new();
    let hits = lookup_domain_knowledge("anything", None, &store).await;
    assert!(hits.is_empty());
}

#[tokio::test]
async fn lookup_increments_access_count() {
    let store = MockKnowledgeStore::new();
    store.upsert(website_entry("example.com", "redirects on www")).await;

    let first = lookup_domain_knowledge("example", None, &store).await;
    assert_eq!(first[0].access_count, 1);
    let second = lookup_domain_knowledge("example", None, &store).await;
    assert_eq!(second[0].access_count, 2);
    let third = lookup_domain_knowledge("example", None, &store).await;
    assert_eq!(third[0].access_count, 3);
}

// ---------------------------------------------------------------------------
// FEAT-DK-002: Working Checkpoint System
// ---------------------------------------------------------------------------

use if2ai_backend::modules::api::{InputContentBlock, InputMessage};
use if2ai_backend::modules::runtime::working_checkpoint::{
    extract_checkpoint, inject_checkpoint, CheckpointInjectionConfig, InjectionPosition,
    WorkingCheckpoint, DEFAULT_CHECKPOINT_MAX_TOKENS,
};
use if2ai_backend::modules::runtime::budget::estimate_tokens;

fn checkpoint_with(key_info: &str) -> WorkingCheckpoint {
    WorkingCheckpoint {
        session_id: "sess-test".to_string(),
        key_info: key_info.to_string(),
        related_sop: None,
        turn_created: 1,
        turn_updated: 1,
    }
}

#[test]
fn extract_key_info_from_tagged_text() {
    let text = "thinking through this... <key_info>file=README.md, line=42</key_info> done.";
    let extraction = extract_checkpoint(text);
    assert_eq!(
        extraction.key_info,
        Some("file=README.md, line=42".to_string())
    );
    assert!(!extraction.should_clear);
}

#[test]
fn extract_returns_none_without_tag() {
    let extraction = extract_checkpoint("just some plain agent output");
    assert_eq!(extraction.key_info, None);
    assert_eq!(extraction.related_sop, None);
    assert!(!extraction.should_clear);
}

#[test]
fn extract_task_complete_sets_should_clear() {
    let extraction = extract_checkpoint("all done. <task_complete/>");
    assert!(extraction.should_clear);
    assert_eq!(extraction.key_info, None);
}

#[test]
fn inject_inserts_before_last_user_message() {
    let mut messages = vec![
        InputMessage::user_text("first user msg"),
        InputMessage {
            role: "assistant".to_string(),
            content: vec![InputContentBlock::Text {
                text: "ack".to_string(),
            }],
            thinking: None,
        },
        InputMessage::user_text("latest user msg"),
    ];
    let cp = checkpoint_with("the key info");
    let cfg = CheckpointInjectionConfig::default();
    inject_checkpoint(&cp, &mut messages, &cfg);
    assert_eq!(messages.len(), 4);
    let injected = &messages[2];
    assert_eq!(injected.role, "user");
    let text = match injected.content.first() {
        Some(InputContentBlock::Text { text }) => text.clone(),
        _ => String::new(),
    };
    assert!(text.contains("[checkpoint]"));
    assert!(text.contains("the key info"));
    let last_user_text = match messages[3].content.first() {
        Some(InputContentBlock::Text { text }) => text.clone(),
        _ => String::new(),
    };
    assert!(last_user_text.contains("latest user msg"));
}

#[test]
fn inject_truncates_to_max_tokens() {
    let huge = "blob ".repeat(2_000);
    let cp = checkpoint_with(&huge);
    let mut messages = vec![InputMessage::user_text("user q")];
    let cfg = CheckpointInjectionConfig::default();
    inject_checkpoint(&cp, &mut messages, &cfg);
    let injected_text = match messages[0].content.first() {
        Some(InputContentBlock::Text { text }) => text.clone(),
        _ => String::new(),
    };
    let tokens = estimate_tokens(&injected_text);
    assert!(
        tokens <= DEFAULT_CHECKPOINT_MAX_TOKENS,
        "injected tokens {tokens} must be ≤ budget {DEFAULT_CHECKPOINT_MAX_TOKENS}"
    );
    assert!(injected_text.contains("[checkpoint]"));
}

#[allow(dead_code)]
fn _silence_position(_p: InjectionPosition) {}

// ---------------------------------------------------------------------------
// FEAT-DK-003: Domain Knowledge Contributor
// ---------------------------------------------------------------------------

use if2ai_backend::modules::memory::{MemoryError, MockUtilityLlm, UtilityLlm};
use if2ai_backend::modules::skills::domain_knowledge::{
    extract_domain_knowledge_candidates, verify_knowledge_safety,
};

const CANDIDATE_JSON: &str = r#"{"kind":"website_domain","domain":"github.com","selector":"div.repo-content","purpose":"main panel","gotcha":"sticky header overlap"}"#;

#[tokio::test]
async fn empty_history_yields_no_candidates() {
    let mock = MockUtilityLlm::empty();
    let out = extract_domain_knowledge_candidates(&[], &mock).await;
    assert!(out.is_empty());
}

#[tokio::test]
async fn selector_history_yields_website_domain() {
    let mock = MockUtilityLlm::new(vec![CANDIDATE_JSON.to_string()]);
    let history = vec![
        InputMessage::user_text("scrape github.com"),
        InputMessage {
            role: "assistant".to_string(),
            content: vec![InputContentBlock::Text {
                text: "use selector div.repo-content".to_string(),
            }],
            thinking: None,
        },
    ];
    let out = extract_domain_knowledge_candidates(&history, &mock).await;
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].kind.label(), "website_domain");
}

struct FailingDkLlm;

#[async_trait::async_trait]
impl UtilityLlm for FailingDkLlm {
    async fn complete(
        &self,
        _system: &str,
        _user: &str,
        _max_tokens: u32,
        _temperature: f32,
    ) -> Result<String, MemoryError> {
        Err(MemoryError::Generic("simulated LLM outage".into()))
    }
}

#[tokio::test]
async fn llm_error_degrades_gracefully() {
    let history = vec![InputMessage::user_text("anything")];
    let out = extract_domain_knowledge_candidates(&history, &FailingDkLlm).await;
    assert!(out.is_empty(), "LLM Err must yield empty, got {}", out.len());
}

#[test]
fn malicious_selector_fails_safety_check() {
    let entry = DomainKnowledgeEntry::new(
        DomainKnowledgeKind::WebsiteDomain {
            domain: "evil.com".to_string(),
            url_patterns: vec!["https://evil.com/*".to_string()],
            selectors: vec![SelectorEntry {
                selector: "rm -rf /".to_string(),
                purpose: "wipe disk".to_string(),
                stability: SelectorStability::Untested,
                last_verified: None,
            }],
            gotchas: vec![],
        },
        KnowledgeAuthor::Agent {
            session_id: "evil-session".to_string(),
        },
        0.9,
    );
    let verdict = verify_knowledge_safety(&entry);
    assert!(!verdict.safe, "malicious selector must fail safety");
    assert!(
        verdict.violations.iter().any(|v| v == "ROOT_DELETE"),
        "expected ROOT_DELETE in violations: {:?}",
        verdict.violations
    );
}
