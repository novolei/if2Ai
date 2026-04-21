use super::*;
use tempfile::tempdir;

fn open_store() -> (tempfile::TempDir, SqlitePinnedStore) {
    let tmp = tempdir().expect("tempdir");
    let db_path = tmp.path().join("memory.db");
    let store = SqlitePinnedStore::open(
        &db_path,
        tmp.path().to_path_buf(),
        Some(Arc::new(ThreatScanner::with_builtin_patterns())),
    )
    .expect("open store");
    (tmp, store)
}

fn open_store_no_scanner() -> (tempfile::TempDir, SqlitePinnedStore) {
    let tmp = tempdir().expect("tempdir");
    let db_path = tmp.path().join("memory.db");
    let store =
        SqlitePinnedStore::open(&db_path, tmp.path().to_path_buf(), None).expect("open store");
    (tmp, store)
}

#[tokio::test]
async fn add_returns_item_with_ulid_id() {
    let (_tmp, store) = open_store();
    let item = store
        .add("hello", PinScope::Global, PinSource::User, None)
        .await
        .expect("add");
    assert_eq!(item.content, "hello");
    assert_eq!(item.scope, PinScope::Global);
    assert!(item.project_id.is_none());
    // ULIDs are exactly 26 chars in Crockford-base32.
    assert_eq!(item.id.len(), 26, "expected ULID, got {}", item.id);
    assert!(Ulid::from_string(&item.id).is_ok());
}

#[tokio::test]
async fn add_dedup_returns_existing_id() {
    let (_tmp, store) = open_store();
    let first = store
        .add("same content", PinScope::Global, PinSource::User, None)
        .await
        .expect("add first");
    let second = store
        .add("same content", PinScope::Global, PinSource::User, None)
        .await
        .expect("add second");
    assert_eq!(first.id, second.id, "dedup must return same id");
    let all = store
        .list(PinScope::Global, None)
        .await
        .expect("list global");
    assert_eq!(all.len(), 1, "dedup must not insert second row");
}

#[tokio::test]
async fn add_dedup_case_insensitive_and_trim() {
    let (_tmp, store) = open_store();
    let first = store
        .add("Foo Bar", PinScope::Global, PinSource::User, None)
        .await
        .expect("add Foo Bar");
    let second = store
        .add("  foo bar  ", PinScope::Global, PinSource::User, None)
        .await
        .expect("add 'foo bar'");
    assert_eq!(first.id, second.id);
}

#[tokio::test]
async fn add_rejects_too_long_content() {
    let (_tmp, store) = open_store();
    let too_long: String = "x".repeat(MAX_PIN_CONTENT_CHARS + 1);
    let err = store
        .add(&too_long, PinScope::Global, PinSource::User, None)
        .await
        .expect_err("must reject");
    assert!(matches!(err, MemoryError::PinnedContentTooLong { .. }));
}

#[tokio::test]
async fn add_rejects_after_50_pins() {
    let (_tmp, store) = open_store_no_scanner();
    for i in 0..MAX_PINS_PER_SCOPE {
        store
            .add(
                &format!("pin number {i}"),
                PinScope::Global,
                PinSource::User,
                None,
            )
            .await
            .expect("add within cap");
    }
    let err = store
        .add("one too many", PinScope::Global, PinSource::User, None)
        .await
        .expect_err("must reject");
    assert!(matches!(err, MemoryError::PinnedLimitExceeded(50)));
}

#[tokio::test]
async fn scope_isolation_project_vs_global() {
    let (_tmp, store) = open_store();
    store
        .add("g1", PinScope::Global, PinSource::User, None)
        .await
        .expect("add global");
    store
        .add("p1", PinScope::Project, PinSource::User, Some("proj-A"))
        .await
        .expect("add project");
    let global = store.list(PinScope::Global, None).await.expect("list g");
    let project = store
        .list(PinScope::Project, Some("proj-A"))
        .await
        .expect("list p");
    assert_eq!(global.len(), 1);
    assert_eq!(global[0].content, "g1");
    assert_eq!(project.len(), 1);
    assert_eq!(project[0].content, "p1");
}

#[tokio::test]
async fn scope_isolation_two_projects() {
    let (_tmp, store) = open_store();
    store
        .add("a-pin", PinScope::Project, PinSource::User, Some("A"))
        .await
        .expect("A");
    store
        .add("b-pin", PinScope::Project, PinSource::User, Some("B"))
        .await
        .expect("B");
    let a = store
        .list(PinScope::Project, Some("A"))
        .await
        .expect("list A");
    let b = store
        .list(PinScope::Project, Some("B"))
        .await
        .expect("list B");
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].content, "a-pin");
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].content, "b-pin");
}

#[tokio::test]
async fn list_all_for_prompt_merges_global_and_project() {
    let (_tmp, store) = open_store();
    store
        .add("g", PinScope::Global, PinSource::User, None)
        .await
        .expect("g");
    store
        .add("p", PinScope::Project, PinSource::User, Some("X"))
        .await
        .expect("p");
    let scope = MemoryExecutionScope {
        session_id: None,
        project_id: Some("X".into()),
        workdir: None,
    };
    let merged = store
        .list_all_for_prompt(&scope)
        .await
        .expect("list_all_for_prompt");
    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].content, "g", "global must come first");
    assert_eq!(merged[1].content, "p");
}

#[tokio::test]
async fn delete_returns_true_then_false() {
    let (_tmp, store) = open_store();
    let item = store
        .add("ephemeral", PinScope::Global, PinSource::User, None)
        .await
        .expect("add");
    assert!(store.delete(&item.id).await.expect("first delete"));
    assert!(!store.delete(&item.id).await.expect("second delete"));
}

#[tokio::test]
async fn reorder_changes_list_order() {
    let (_tmp, store) = open_store();
    let a = store
        .add("alpha", PinScope::Global, PinSource::User, None)
        .await
        .expect("a");
    let b = store
        .add("beta", PinScope::Global, PinSource::User, None)
        .await
        .expect("b");
    let c = store
        .add("gamma", PinScope::Global, PinSource::User, None)
        .await
        .expect("c");
    store
        .reorder(&[c.id.clone(), a.id.clone(), b.id.clone()])
        .await
        .expect("reorder");
    let listed = store.list(PinScope::Global, None).await.expect("list");
    let ids: Vec<&str> = listed.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, vec![c.id.as_str(), a.id.as_str(), b.id.as_str()]);
}

#[tokio::test]
async fn pii_in_pin_content_is_scrubbed_and_audited() {
    let (_tmp, store) = open_store();
    let secret = format!("note: sk-{}", "a".repeat(40));
    let item = store
        .add(&secret, PinScope::Global, PinSource::User, None)
        .await
        .expect("add");
    assert!(
        item.content.contains("[REDACTED:ApiKey]"),
        "expected redacted marker, got {:?}",
        item.content
    );
    assert!(!item.content.contains("sk-aaaa"));
}

#[tokio::test]
async fn sidecar_markdown_matches_sqlite_after_add() {
    let (tmp, store) = open_store_no_scanner();
    store
        .add("first pin", PinScope::Global, PinSource::User, None)
        .await
        .expect("add 1");
    store
        .add("second pin", PinScope::Global, PinSource::User, None)
        .await
        .expect("add 2");
    let path = tmp.path().join("pinned.md");
    let body = std::fs::read_to_string(&path).expect("read sidecar");
    assert!(body.contains("- first pin"), "missing first; got: {body}");
    assert!(body.contains("- second pin"), "missing second; got: {body}");
}

#[tokio::test]
async fn sidecar_atomic_no_partial_file() {
    let (tmp, store) = open_store_no_scanner();
    store
        .add("p", PinScope::Global, PinSource::User, None)
        .await
        .expect("add");
    let mut stray = false;
    for entry in std::fs::read_dir(tmp.path()).expect("read_dir") {
        let entry = entry.expect("dir entry");
        let name = entry.file_name();
        if name.to_string_lossy().ends_with(".tmp") {
            stray = true;
        }
    }
    assert!(!stray, "stray .tmp file left in scope_root");
}

#[tokio::test]
async fn null_store_is_inert() {
    let store = NullPinnedStore::new();
    assert!(store.list(PinScope::Global, None).await.unwrap().is_empty());
    let scope = MemoryExecutionScope::global();
    assert!(store.list_all_for_prompt(&scope).await.unwrap().is_empty());
    assert!(store
        .add("ignored", PinScope::Global, PinSource::User, None)
        .await
        .is_err());
    assert!(!store.delete("anything").await.unwrap());
    store.reorder(&[]).await.unwrap();
}

#[tokio::test]
async fn tool_source_round_trips_through_sqlite() {
    let (_tmp, store) = open_store_no_scanner();
    let src = PinSource::Tool {
        tool_name: "pin_memory".into(),
        session_id: "sess-42".into(),
    };
    let item = store
        .add("agent pin", PinScope::Global, src.clone(), None)
        .await
        .expect("add");
    let listed = store.list(PinScope::Global, None).await.expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, item.id);
    assert_eq!(listed[0].created_by, src);
}
