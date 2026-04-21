use super::*;
use tempfile::TempDir;

fn create_test_provider() -> (SqliteMemoryProvider, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_memory.db");
    let provider = SqliteMemoryProvider::new(db_path).unwrap();
    (provider, temp_dir)
}

#[tokio::test]
async fn store_and_recall() {
    let (provider, _temp) = create_test_provider();

    provider
        .store("test_key", "Hello world", MemoryCategory::Core)
        .await
        .unwrap();

    let results = provider.recall("test_key", None, 10).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "test_key");
    assert_eq!(results[0].content, "Hello world");
    assert_eq!(results[0].importance, 0.5);
    assert_eq!(results[0].access_count, 0);
}

#[tokio::test]
async fn store_upserts_existing() {
    let (provider, _temp) = create_test_provider();

    provider
        .store("key", "First value", MemoryCategory::Core)
        .await
        .unwrap();
    provider
        .store("key", "Second value", MemoryCategory::Core)
        .await
        .unwrap();

    let results = provider.recall("key", None, 10).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].content, "Second value");
}

#[tokio::test]
async fn delete_entry() {
    let (provider, _temp) = create_test_provider();

    provider
        .store("to_delete", "value", MemoryCategory::Core)
        .await
        .unwrap();
    provider.delete("to_delete").await.unwrap();

    let results = provider.recall("to_delete", None, 10).await.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn purge_category() {
    let (provider, _temp) = create_test_provider();

    provider
        .store("a", "1", MemoryCategory::Core)
        .await
        .unwrap();
    provider
        .store("b", "2", MemoryCategory::Core)
        .await
        .unwrap();
    provider
        .store("c", "3", MemoryCategory::Daily)
        .await
        .unwrap();

    provider.purge_category("core").await.unwrap();

    let core = provider.recall("", Some("core"), 10).await.unwrap();
    assert!(core.is_empty());

    let daily = provider.recall("", Some("daily"), 10).await.unwrap();
    assert_eq!(daily.len(), 1);
}

#[tokio::test]
async fn filter_by_category() {
    let (provider, _temp) = create_test_provider();

    provider
        .store("x", "1", MemoryCategory::Core)
        .await
        .unwrap();
    provider
        .store("y", "2", MemoryCategory::Daily)
        .await
        .unwrap();

    let core_results = provider.recall("", Some("core"), 10).await.unwrap();
    assert_eq!(core_results.len(), 1);
    assert_eq!(core_results[0].key, "x");
}

#[tokio::test]
async fn recall_with_limit() {
    let (provider, _temp) = create_test_provider();

    for i in 0..5 {
        provider
            .store(
                &format!("key_{i}"),
                &format!("value_{i}"),
                MemoryCategory::Core,
            )
            .await
            .unwrap();
    }

    let all = provider.recall("", None, 10).await.unwrap();
    assert_eq!(all.len(), 5);
}

#[tokio::test]
async fn export_all() {
    let (provider, _temp) = create_test_provider();

    provider
        .store("a", "1", MemoryCategory::Core)
        .await
        .unwrap();
    provider
        .store("b", "2", MemoryCategory::Daily)
        .await
        .unwrap();

    let exported = provider.export(None).await.unwrap();
    assert_eq!(exported.len(), 2);

    let core_export = provider.export(Some("core")).await.unwrap();
    assert_eq!(core_export.len(), 1);
}

#[tokio::test]
async fn persists_across_reopens() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_memory.db");

    let provider = SqliteMemoryProvider::new(db_path.clone()).unwrap();
    provider
        .store("persist_key", "persist_value", MemoryCategory::Core)
        .await
        .unwrap();

    // Reopen the same database file
    let provider2 = SqliteMemoryProvider::new(db_path).unwrap();
    let results = provider2.recall("persist_key", None, 10).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].content, "persist_value");
}

/// Verifies that `apply_importance_decay` writes updated importance values
/// back to SQLite for all matching entries.
///
/// For brand-new entries, `WeibullDecay::compute_importance` computes:
///   `(base + trust_boost) * decay_factor`
/// where `trust_boost = (0.0 + 1.0) * 0.05 = 0.05` and `decay_factor ≈ 1.0`
/// for age≈0.  This results in importance ≈ 0.55, not ≤ 0.5.
/// The test asserts that the value is actually written back (changed from default).
#[tokio::test]
async fn apply_importance_decay_updates_entries() {
    let (provider, _temp) = create_test_provider();

    provider
        .store("decay_a", "data a", MemoryCategory::Core)
        .await
        .unwrap();
    provider
        .store("decay_b", "data b", MemoryCategory::Daily)
        .await
        .unwrap();

    // Use export to retrieve all entries and find by key — avoids recall's
    // LIMIT clause filtering out the target when multiple entries exist.
    let all_before = provider.export(None).await.unwrap();
    let before_a = all_before
        .iter()
        .find(|e| e.key == "decay_a")
        .expect("decay_a should exist before decay");
    assert!(
        (before_a.importance - 0.5).abs() < 1e-6,
        "initial importance should be 0.5, got {}",
        before_a.importance
    );

    // Apply decay with default parameters (7-day lambda, k=1.2).
    let updated = provider.apply_importance_decay(168.0, 1.2).await.unwrap();
    assert_eq!(updated, 2, "should have updated 2 entries");

    // For a brand-new entry (age≈0) with neutral trust (score=0), the
    // compute_importance formula gives ≈ 0.55 — different from the
    // stored default 0.5.
    let all_after = provider.export(None).await.unwrap();
    let after_a = all_after
        .iter()
        .find(|e| e.key == "decay_a")
        .expect("decay_a should exist after decay");
    assert!(
        (after_a.importance - before_a.importance).abs() > 1e-9,
        "importance should have changed after decay, before={} after={}",
        before_a.importance,
        after_a.importance
    );
}

/// Test that a store_scoped entry is visible to recall_scoped with the same session,
/// and invisible to recall_scoped with a different session.
///
/// This is the core P0 isolation property of the Memory Control Plane.
#[tokio::test]
async fn store_scoped_entry_is_visible_only_within_same_session() {
    use crate::modules::memory::scope::MemoryScopeResolver;

    let (provider, _temp) = create_test_provider();

    let scope_a = MemoryScopeResolver::resolve(Some("session-a"), None, None);
    let scope_b = MemoryScopeResolver::resolve(Some("session-b"), None, None);

    // Store entry scoped to session-a.
    provider
        .store_scoped(
            "secret_key",
            "session-a secret",
            MemoryCategory::Core,
            &scope_a,
        )
        .await
        .unwrap();

    // session-a can recall the entry.
    let results_a = provider
        .recall_scoped("secret_key", None, 10, &scope_a)
        .await
        .unwrap();
    assert_eq!(results_a.len(), 1, "session-a should see its own entry");
    assert_eq!(results_a[0].content, "session-a secret");

    // session-b cannot see session-a's entry.
    let results_b = provider
        .recall_scoped("secret_key", None, 10, &scope_b)
        .await
        .unwrap();
    assert!(
        results_b.is_empty(),
        "session-b must not see session-a's scoped entry; got: {:?}",
        results_b
    );
}

#[tokio::test]
async fn migrate_from_json_imports_entries_idempotently() {
    let (provider, temp) = create_test_provider();
    let json_path = temp.path().join("memory.json");

    let payload = r#"[
        {
            "key": "claw_a",
            "content": "claw content A",
            "category": "core",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-02T00:00:00Z"
        },
        {
            "key": "claw_b",
            "content": "claw content B",
            "category": "daily",
            "created_at": "2024-01-03T00:00:00Z",
            "updated_at": "2024-01-04T00:00:00Z"
        }
    ]"#;
    std::fs::write(&json_path, payload).unwrap();

    let imported = provider.migrate_from_json(&json_path).await.unwrap();
    assert_eq!(imported, 2);

    let exported = provider.export(None).await.unwrap();
    assert_eq!(exported.len(), 2);
    assert!(exported.iter().any(|e| e.key == "claw_a"));
    assert!(exported.iter().any(|e| e.key == "claw_b"));

    // Second migration is a no-op — ON CONFLICT DO NOTHING preserves originals.
    let imported_again = provider.migrate_from_json(&json_path).await.unwrap();
    assert_eq!(imported_again, 0);

    // Missing file returns Ok(0).
    let missing = provider
        .migrate_from_json(&temp.path().join("nope.json"))
        .await
        .unwrap();
    assert_eq!(missing, 0);
}

/// Test that an unscoped (global) entry stored via store() is visible to
/// recall_scoped with any session — backward-compat requirement.
#[tokio::test]
async fn global_entry_visible_to_all_sessions() {
    use crate::modules::memory::scope::MemoryScopeResolver;

    let (provider, _temp) = create_test_provider();

    let scope_any = MemoryScopeResolver::resolve(Some("session-x"), None, None);

    // Store a global (unscoped) entry via the legacy store() method.
    provider
        .store("global_key", "global fact", MemoryCategory::Core)
        .await
        .unwrap();

    // Any session can see global entries because recall_scoped includes
    // WHERE (session_id = ?1 OR session_id IS NULL).
    let results = provider
        .recall_scoped("global_key", None, 10, &scope_any)
        .await
        .unwrap();
    assert_eq!(
        results.len(),
        1,
        "global entry should be visible to all sessions"
    );
    assert_eq!(results[0].content, "global fact");
}

// -----------------------------------------------------------------------
// Three-tier scope visibility regression suite — covers the rules in
// `recall_scoped` / `scope_visibility_clause`.
//
// Each test stores entries via `store_scoped` with explicit scopes and
// asserts the visibility set seen from various caller scopes.  We assert
// exact key sets (sorted) to make accidental leaks very loud.

use crate::modules::memory::scope::MemoryScopeResolver;

/// Helper: store a scoped entry from a tuple `(key, session, project)`.
async fn store_scope(
    provider: &SqliteMemoryProvider,
    key: &str,
    session_id: Option<&str>,
    project_id: Option<&str>,
) {
    let scope = MemoryScopeResolver::resolve(session_id, project_id, None);
    provider
        .store_scoped(key, key, MemoryCategory::Core, &scope)
        .await
        .unwrap();
}

/// Helper: collect sorted keys returned by `recall_scoped`.
async fn recall_keys(
    provider: &SqliteMemoryProvider,
    session_id: Option<&str>,
    project_id: Option<&str>,
) -> Vec<String> {
    let scope = MemoryScopeResolver::resolve(session_id, project_id, None);
    let mut out: Vec<String> = provider
        .recall_scoped("", None, 100, &scope)
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.key)
        .collect();
    out.sort();
    out
}

/// (1) session-scoped entry is visible to its own session — and that
///     session also sees its project's project-level entries plus globals.
/// (2) session-scoped entry is invisible to other sessions in the same
///     project (sessions are leaves, not shared).
#[tokio::test]
async fn session_entry_visible_to_own_session_only() {
    let (provider, _temp) = create_test_provider();

    // Same-project, two sessions.
    store_scope(&provider, "sess_a_only", Some("sess-a"), Some("proj-1")).await;
    store_scope(&provider, "sess_b_only", Some("sess-b"), Some("proj-1")).await;
    // Project-level (no session) and global entries for cross-checks.
    store_scope(&provider, "proj_1_shared", None, Some("proj-1")).await;
    store_scope(&provider, "global_shared", None, None).await;

    let from_a = recall_keys(&provider, Some("sess-a"), Some("proj-1")).await;
    assert_eq!(
        from_a,
        vec![
            "global_shared".to_string(),
            "proj_1_shared".to_string(),
            "sess_a_only".to_string(),
        ],
        "sess-a should see its own + project-level + global; got {:?}",
        from_a
    );

    let from_b = recall_keys(&provider, Some("sess-b"), Some("proj-1")).await;
    assert!(
        !from_b.contains(&"sess_a_only".to_string()),
        "sess-b must NOT see sess-a's session entry; got {:?}",
        from_b
    );
}

/// (3) project-scoped entry is visible across different sessions of the
///     SAME project.
#[tokio::test]
async fn project_entry_visible_across_sessions_of_same_project() {
    let (provider, _temp) = create_test_provider();

    store_scope(&provider, "proj_1_shared", None, Some("proj-1")).await;

    let from_a = recall_keys(&provider, Some("sess-a"), Some("proj-1")).await;
    let from_b = recall_keys(&provider, Some("sess-b"), Some("proj-1")).await;

    assert!(
        from_a.contains(&"proj_1_shared".to_string()),
        "sess-a should see project entry; got {:?}",
        from_a
    );
    assert!(
        from_b.contains(&"proj_1_shared".to_string()),
        "sess-b should see project entry too; got {:?}",
        from_b
    );
}

/// (4) project-scoped entry is invisible to OTHER projects, regardless of
///     whether the caller is session-bound.
#[tokio::test]
async fn project_entry_isolated_from_other_project() {
    let (provider, _temp) = create_test_provider();

    store_scope(&provider, "proj_1_secret", None, Some("proj-1")).await;
    store_scope(&provider, "proj_2_secret", None, Some("proj-2")).await;

    // Caller in proj-1 must not see proj-2's entry, and vice versa.
    let from_proj1 = recall_keys(&provider, Some("sess-x"), Some("proj-1")).await;
    assert!(
        !from_proj1.contains(&"proj_2_secret".to_string()),
        "proj-1 caller leaked proj-2 entry; got {:?}",
        from_proj1
    );

    let from_proj2 = recall_keys(&provider, Some("sess-y"), Some("proj-2")).await;
    assert!(
        !from_proj2.contains(&"proj_1_secret".to_string()),
        "proj-2 caller leaked proj-1 entry; got {:?}",
        from_proj2
    );

    // Project-only scope (no session) should also enforce isolation.
    let proj1_only = recall_keys(&provider, None, Some("proj-1")).await;
    assert!(
        proj1_only.contains(&"proj_1_secret".to_string())
            && !proj1_only.contains(&"proj_2_secret".to_string()),
        "project-only scope failed isolation; got {:?}",
        proj1_only
    );
}

/// (5) global entry is visible from every scope flavour: session+project,
///     project-only, and global.
#[tokio::test]
async fn global_entry_visible_from_every_scope() {
    let (provider, _temp) = create_test_provider();

    store_scope(&provider, "global_x", None, None).await;

    let from_session = recall_keys(&provider, Some("sess-a"), Some("proj-1")).await;
    let from_project = recall_keys(&provider, None, Some("proj-1")).await;
    let from_global = recall_keys(&provider, None, None).await;

    assert!(from_session.contains(&"global_x".to_string()));
    assert!(from_project.contains(&"global_x".to_string()));
    assert!(from_global.contains(&"global_x".to_string()));
}

/// Project-aware ranking: when a session caller can see entries from all
/// three tiers, the session-tier rows must surface FIRST, then project,
/// then global — regardless of `updated_at` order.
#[tokio::test]
async fn recall_ranks_session_above_project_above_global() {
    let (provider, _temp) = create_test_provider();

    // Insert in reverse-priority order so a naive `ORDER BY updated_at`
    // would put global first; the scope-tier ranking must override that.
    store_scope(&provider, "g_first", None, None).await;
    store_scope(&provider, "p_then", None, Some("proj-1")).await;
    store_scope(&provider, "s_last", Some("sess-a"), Some("proj-1")).await;

    let scope = MemoryScopeResolver::resolve(Some("sess-a"), Some("proj-1"), None);
    let ordered: Vec<String> = provider
        .recall_scoped("", None, 10, &scope)
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.key)
        .collect();

    assert_eq!(
        ordered,
        vec![
            "s_last".to_string(),
            "p_then".to_string(),
            "g_first".to_string(),
        ],
        "expected session > project > global ordering, got {:?}",
        ordered
    );
}

/// Within the same scope tier, higher `importance` wins over recency.
/// We use `apply_importance_decay` then a manual UPDATE to set deterministic
/// importance values, since `store_scoped` always defaults to 0.5.
#[tokio::test]
async fn recall_within_tier_orders_by_importance_then_access_count() {
    let (provider, _temp) = create_test_provider();

    // Two same-tier entries.
    store_scope(&provider, "low_imp", None, None).await;
    store_scope(&provider, "high_imp", None, None).await;

    // Bump high_imp's importance and access_count via direct SQL — this is
    // a test-only path that bypasses store_scoped to set up a deterministic
    // ranking signal.
    {
        let c = provider.conn.lock().unwrap();
        c.execute(
            "UPDATE memory_entries SET importance = 0.95, access_count = 10 WHERE key = ?1",
            params!["high_imp"],
        )
        .unwrap();
        c.execute(
            "UPDATE memory_entries SET importance = 0.10, access_count = 0 WHERE key = ?1",
            params!["low_imp"],
        )
        .unwrap();
    }

    let scope = MemoryExecutionScope::global();
    let ordered: Vec<String> = provider
        .recall_scoped("", None, 10, &scope)
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.key)
        .collect();

    assert_eq!(
        ordered,
        vec!["high_imp".to_string(), "low_imp".to_string()],
        "high-importance entry must surface first within the same tier; got {:?}",
        ordered
    );
}

/// Belt-and-braces: global scope must NOT see any session- or project-
/// scoped entries, only true globals.
#[tokio::test]
async fn global_scope_sees_only_global_entries() {
    let (provider, _temp) = create_test_provider();

    store_scope(&provider, "sess_only", Some("sess-a"), Some("proj-1")).await;
    store_scope(&provider, "proj_only", None, Some("proj-1")).await;
    store_scope(&provider, "true_global", None, None).await;

    let from_global = recall_keys(&provider, None, None).await;
    assert_eq!(
        from_global,
        vec!["true_global".to_string()],
        "global scope must only see truly unscoped entries; got {:?}",
        from_global
    );
}

/// `demote_scope` is wired through the trait default to `promote_scope`,
/// so a successful demote must persist the narrower scope tags exactly
/// like a promote does.  Round-trip: `global → project → session`.
#[tokio::test]
async fn demote_scope_round_trip_global_to_project_to_session() {
    let (provider, _t) = create_test_provider();

    // Seed: a global entry (no session/project tags).
    let global_scope = MemoryExecutionScope {
        session_id: None,
        project_id: None,
        workdir: None,
    };
    provider
        .store_scoped(
            "demoteable",
            "secret",
            MemoryCategory::Conversation,
            &global_scope,
        )
        .await
        .unwrap();

    // Demote 1: global → project P1.
    let project_scope = MemoryExecutionScope {
        session_id: None,
        project_id: Some("P1".to_string()),
        workdir: None,
    };
    provider
        .demote_scope("demoteable", &project_scope)
        .await
        .unwrap();
    let after_p = provider.export(None).await.unwrap();
    let entry_p = after_p.iter().find(|e| e.key == "demoteable").unwrap();
    assert_eq!(entry_p.session_id, None);
    assert_eq!(entry_p.project_id.as_deref(), Some("P1"));

    // Demote 2: project → session S1 (still owned by P1).
    let session_scope = MemoryExecutionScope {
        session_id: Some("S1".to_string()),
        project_id: Some("P1".to_string()),
        workdir: None,
    };
    provider
        .demote_scope("demoteable", &session_scope)
        .await
        .unwrap();
    let after_s = provider.export(None).await.unwrap();
    let entry_s = after_s.iter().find(|e| e.key == "demoteable").unwrap();
    assert_eq!(entry_s.session_id.as_deref(), Some("S1"));
    assert_eq!(entry_s.project_id.as_deref(), Some("P1"));

    // Sanity: a non-existent key surfaces KeyNotFound, not silent success.
    let err = provider
        .demote_scope("missing", &session_scope)
        .await
        .unwrap_err();
    assert!(
        matches!(err, MemoryError::KeyNotFound(ref k) if k == "missing"),
        "expected KeyNotFound for missing key, got {err:?}"
    );
}
