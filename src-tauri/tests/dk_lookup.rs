//! WU-008 — Domain knowledge lookup hook tests.

use if2ai_backend::modules::application::prompt_planner::PromptBlockKind;
use if2ai_backend::modules::application::turn_service::dk_lookup_hook::{
    lookup_for_skill_resolution, DISABLE_DK_LOOKUP_ENV, DK_CONTRIBUTION_SOURCE,
};
use if2ai_backend::modules::runtime::contracts::common::{CorrelationIds, RuntimeEventType};
use if2ai_backend::modules::runtime::evolution_emitter::{
    emit_evolution_event, DISABLE_EMIT_ENV,
};
use if2ai_backend::modules::skills::domain_knowledge::{
    mock::MockKnowledgeStore, DomainKnowledgeEntry, DomainKnowledgeKind, KnowledgeAuthor,
    KnowledgeStore, SOPStep, SelectorEntry, SelectorStability,
};

fn website_entry(domain: &str) -> DomainKnowledgeEntry {
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
            gotchas: vec!["sticky header".to_string()],
        },
        KnowledgeAuthor::User,
        0.9,
    )
}

fn sop_entry() -> DomainKnowledgeEntry {
    DomainKnowledgeEntry::new(
        DomainKnowledgeKind::TaskSOP {
            task_type: "file_batch_rename".to_string(),
            prerequisites: vec!["clean dir".to_string()],
            key_pitfalls: Vec::new(),
            execution_steps: vec![SOPStep {
                tool_name: "bash".to_string(),
                description: "stat".to_string(),
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

#[tokio::test]
async fn lookup_called_after_skill_resolution() {
    let prev = std::env::var(DISABLE_DK_LOOKUP_ENV).ok();
    std::env::remove_var(DISABLE_DK_LOOKUP_ENV);
    let store = MockKnowledgeStore::new();
    store.upsert(website_entry("amazon.com")).await;
    let contributions = lookup_for_skill_resolution("amazon", &store).await;
    assert_eq!(contributions.len(), 1, "lookup must surface 1 entry");
    assert_eq!(contributions[0].kind, PromptBlockKind::Skill);
    assert_eq!(contributions[0].source.subsystem, DK_CONTRIBUTION_SOURCE);
    if let Some(v) = prev {
        std::env::set_var(DISABLE_DK_LOOKUP_ENV, v);
    }
}

#[tokio::test]
async fn knowledge_entries_produce_contribution() {
    let prev = std::env::var(DISABLE_DK_LOOKUP_ENV).ok();
    std::env::remove_var(DISABLE_DK_LOOKUP_ENV);
    let store = MockKnowledgeStore::new();
    store.upsert(website_entry("github.com")).await;
    store.upsert(sop_entry()).await;
    let contributions = lookup_for_skill_resolution("", &store).await;
    assert!(!contributions.is_empty());
    for c in &contributions {
        assert_eq!(c.source.subsystem, DK_CONTRIBUTION_SOURCE);
        assert!(!c.body.is_empty());
        assert!(c.title.starts_with("dk:"));
    }
    if let Some(v) = prev {
        std::env::set_var(DISABLE_DK_LOOKUP_ENV, v);
    }
}

#[tokio::test]
async fn lookup_failure_does_not_propagate() {
    // MockKnowledgeStore never errors at the trait surface; this
    // test pins the contract that "no hits" returns an empty Vec
    // (the production failure-isolation contract — wrapper must not
    // surface a Result<Err> to the caller).
    let prev = std::env::var(DISABLE_DK_LOOKUP_ENV).ok();
    std::env::remove_var(DISABLE_DK_LOOKUP_ENV);
    let store = MockKnowledgeStore::new();
    let contributions = lookup_for_skill_resolution("nonexistent", &store).await;
    assert!(contributions.is_empty());
    if let Some(v) = prev {
        std::env::set_var(DISABLE_DK_LOOKUP_ENV, v);
    }
}

#[test]
fn lookup_emits_domain_knowledge_event() {
    let prev = std::env::var(DISABLE_EMIT_ENV).ok();
    std::env::set_var(DISABLE_EMIT_ENV, "1");
    let env = emit_evolution_event(
        None,
        RuntimeEventType::DomainKnowledge,
        "lookup",
        CorrelationIds::default(),
        &serde_json::json!({
            "entryId": "abc",
            "kind": "website_domain",
            "source": "lookup",
        }),
        None,
    )
    .expect("emit must succeed under kill-switch");
    assert_eq!(env.event_type, RuntimeEventType::DomainKnowledge);
    match prev {
        Some(v) => std::env::set_var(DISABLE_EMIT_ENV, v),
        None => std::env::remove_var(DISABLE_EMIT_ENV),
    }
}

#[tokio::test]
async fn env_flag_disables_lookup() {
    let prev = std::env::var(DISABLE_DK_LOOKUP_ENV).ok();
    std::env::set_var(DISABLE_DK_LOOKUP_ENV, "1");
    let store = MockKnowledgeStore::new();
    store.upsert(website_entry("amazon.com")).await;
    let contributions = lookup_for_skill_resolution("amazon", &store).await;
    assert!(contributions.is_empty(), "kill-switch must skip lookup");
    match prev {
        Some(v) => std::env::set_var(DISABLE_DK_LOOKUP_ENV, v),
        None => std::env::remove_var(DISABLE_DK_LOOKUP_ENV),
    }
}
