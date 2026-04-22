//! MIG-001-c — end-to-end smoke test for the canonical
//! [`if2ai_backend::modules::application::TurnService::stream_turn`]
//! seam.
//!
//! Goal: prove the streaming IPC adapter
//! (`commands::agent::start_agent_stream`) is now a thin
//! pass-through and that the canonical streaming entry path lives
//! inside `TurnService`. We exercise the deterministic early-error
//! path: invoking `stream_turn` on a service constructed without a
//! Tauri `AppHandle` must fail fast with the documented "requires
//! an AppHandle" error string.
//!
//! The full streaming tool-loop exercise (real provider, channel
//! emission, retries, etc.) is out of scope for unit / integration
//! tests today and is covered indirectly via harness suites.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use if2ai_backend::modules::application::{StreamTurnRequest, TurnService, TurnServiceDeps};
use if2ai_backend::modules::memory::job_runner::JobRunner;
use if2ai_backend::modules::memory::llm::MockUtilityLlm;
use if2ai_backend::modules::memory::summary::store::NullSessionSummaryStore;
use if2ai_backend::modules::memory::summary::RollingSummarizer;
use if2ai_backend::modules::memory::{
    default_memory_provider, MemoryCompiler, MemoryTicker, NullPinnedStore, PinnedStore,
    SessionSummaryStore, UtilityLlm,
};
use if2ai_backend::modules::projects::ProjectManager;
use if2ai_backend::modules::runtime::budget::ContextBudget;
use if2ai_backend::modules::runtime::config::CompilerConfig;
use if2ai_backend::modules::session::SessionManager;
use if2ai_backend::modules::tools::{ToolContext, ToolRegistry};

fn unique_temp_root(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be valid")
        .as_nanos();
    std::env::temp_dir().join(format!("if2ai-{label}-{nanos}"))
}

fn make_memory_ticker() -> Arc<MemoryTicker> {
    let store: Arc<dyn SessionSummaryStore> = Arc::new(NullSessionSummaryStore::new());
    let llm: Arc<dyn UtilityLlm> = Arc::new(MockUtilityLlm::empty());
    let jobs_db = std::env::temp_dir().join(format!(
        "if2ai-jobs-stream-{}.db",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos()
    ));
    let job_runner =
        Arc::new(JobRunner::open(jobs_db.as_path(), 3, 2).expect("on-disk JobRunner constructs"));
    let summarizer = Arc::new(RollingSummarizer::new(
        store.clone(),
        llm.clone(),
        job_runner.clone(),
        None,
    ));
    let compiler = Arc::new(MemoryCompiler::new(
        store.clone(),
        llm,
        job_runner,
        CompilerConfig::default(),
    ));
    Arc::new(MemoryTicker::new(
        summarizer,
        compiler,
        store,
        Default::default(),
    ))
}

async fn make_service_without_app_handle() -> (TurnService, PathBuf) {
    let root = unique_temp_root("turn-service-stream-e2e");
    let sessions_dir = root.join("sessions");
    let projects_dir = root.join("projects");
    std::fs::create_dir_all(&sessions_dir).expect("create sessions dir");
    std::fs::create_dir_all(&projects_dir).expect("create projects dir");

    let workdir = root.join("workdir");
    std::fs::create_dir_all(&workdir).expect("create workdir");

    let context = std::sync::Arc::new(std::sync::Mutex::new(ToolContext::default_for_workdir(
        workdir,
    )));
    let tool_registry = Arc::new(ToolRegistry::new(context));
    let session_manager = Arc::new(SessionManager::new(sessions_dir, projects_dir.clone()));
    let project_manager = Arc::new(ProjectManager::new(projects_dir));
    let memory_provider = default_memory_provider().await;
    let pinned_store: Arc<dyn PinnedStore> = Arc::new(NullPinnedStore::new());

    let deps = TurnServiceDeps {
        tool_registry,
        pinned_store,
        memory_provider,
        active_retrieval_manager: None,
        session_manager,
        project_manager,
        harness: None,
        learning_module: None,
        context_budget: ContextBudget::default(),
        memory_ticker: make_memory_ticker(),
        trajectory_manager: None,
        // Intentionally `None`: this test verifies the canonical
        // entry path early-rejects the streaming request when the
        // IPC adapter forgot to plumb an `AppHandle` through.
        app_handle: None,
        learned_traits: None,
    };

    (TurnService::new(deps), root)
}

/// MIG-001-c — proves the canonical streaming entry path is owned
/// by `TurnService::stream_turn`: with no `AppHandle` injected, the
/// method surfaces the explicit
/// `"TurnService::stream_turn requires an AppHandle"` error
/// instead of panicking or silently spawning a broken task.
#[tokio::test(flavor = "multi_thread")]
async fn stream_turn_requires_app_handle() {
    let (service, _root) = make_service_without_app_handle().await;

    let request = StreamTurnRequest {
        session_id: "irrelevant-session".to_string(),
        user_message: "hello".to_string(),
        permission_mode: None,
        stream_cancel_senders: Arc::new(Mutex::new(HashMap::new())),
        permission_senders: Arc::new(Mutex::new(HashMap::new())),
        permission_overrides: Arc::new(Mutex::new(HashMap::new())),
    };

    let result = service.stream_turn(request).await;

    let err = match result {
        Ok(stream_id) => {
            panic!("stream_turn must fail without an AppHandle; got stream_id={stream_id:?}")
        }
        Err(e) => e,
    };
    assert!(
        err.contains("AppHandle"),
        "expected error to mention AppHandle, got: {err:?}"
    );
}
