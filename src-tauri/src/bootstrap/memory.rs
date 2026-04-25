use std::path::Path;
use std::sync::Arc;

use crate::modules;

pub(super) struct MemoryBootstrap {
    pub job_runner: Arc<modules::memory::JobRunner>,
    pub utility_llm: Arc<dyn modules::memory::UtilityLlm>,
    pub summary_store: Arc<dyn modules::memory::SessionSummaryStore>,
    pub rolling_summarizer: Arc<modules::memory::summary::RollingSummarizer>,
    pub memory_compiler: Arc<modules::memory::MemoryCompiler>,
    pub memory_ticker: Arc<modules::memory::MemoryTicker>,
    pub pinned_store: Arc<dyn modules::memory::PinnedStore>,
    pub memory_provider: modules::memory::SharedMemoryProvider,
    /// MEM-MOD-P7 — cross-session learned-traits store; `None` only
    /// when its SQLite file failed to open (rare; in that case the
    /// IPCs degrade to "no traits" but the rest of the app keeps
    /// running).
    pub learned_traits: Option<modules::memory::learned_traits::LearnedTraitsStore>,
}

pub(super) fn build_memory_bootstrap(
    paths: &super::BootPaths,
    threat_scanner: Arc<modules::memory::security::ThreatScanner>,
) -> MemoryBootstrap {
    let job_runner = Arc::new(open_job_runner_with_fallback(&paths.memory_root));

    // MEM-MOD-WIRE-FIX — bind UtilityLlm to the live chat provider so
    // the entire memory pipeline (rolling summary → compile_today/
    // week/longterm/facts → reflection_loop → learned_traits
    // extractor → Mem0 decision tree) runs through the user's
    // configured chat model.  Pre-fix this was a `MockUtilityLlm::
    // empty()` that silently returned `""` for every call, which is
    // why memory.md was always blank, learned_traits stayed at 0,
    // and the compile pipeline reported every step SKIPPED.
    //
    // Per-role usage tagging: each shim instance carries its own
    // caller label so the durable usage store can break the per-role
    // dashboard down (chat / summarizer / compiler / utility /
    // utility_large).  RollingSummarizer + MemoryCompiler get
    // dedicated labels; everything else (reflection, learned-traits,
    // small one-shots) shares the default `utility` label.
    let utility_llm: Arc<dyn modules::memory::UtilityLlm> = Arc::new(
        modules::memory::ChatProviderUtilityLlm::new(paths.if2ai_dir.clone()),
    );
    let summarizer_llm: Arc<dyn modules::memory::UtilityLlm> = Arc::new(
        modules::memory::ChatProviderUtilityLlm::new(paths.if2ai_dir.clone())
            .with_caller(modules::usage::CALLER_SUMMARIZER),
    );
    let compiler_llm: Arc<dyn modules::memory::UtilityLlm> = Arc::new(
        modules::memory::ChatProviderUtilityLlm::new(paths.if2ai_dir.clone())
            .with_caller(modules::usage::CALLER_COMPILER),
    );
    tracing::info!(
        "[init] UtilityLlm bound to ChatProviderUtilityLlm (workdir={:?}, callers=chat/summarizer/compiler/utility)",
        paths.if2ai_dir
    );

    let summary_db_path = paths.memory_root.join("memory.db");
    let summary_store: Arc<dyn modules::memory::SessionSummaryStore> =
        match modules::memory::SqliteSessionSummaryStore::open(
            &summary_db_path,
            paths.memory_root.clone(),
        ) {
            Ok(store) => {
                tracing::info!(
                    "[init] SessionSummaryStore initialised at {:?}",
                    summary_db_path
                );
                Arc::new(store)
            }
            Err(e) => {
                tracing::error!(
                    "[init] SessionSummaryStore failed to open {summary_db_path:?}: {e}; using NullSessionSummaryStore (summaries will not persist)"
                );
                Arc::new(modules::memory::NullSessionSummaryStore::new())
            }
        };

    let rolling_summarizer = Arc::new(modules::memory::summary::RollingSummarizer::new(
        summary_store.clone(),
        summarizer_llm.clone(),
        job_runner.clone(),
        Some(threat_scanner.clone()),
    ));

    let memory_compiler = Arc::new(modules::memory::MemoryCompiler::new(
        summary_store.clone(),
        compiler_llm.clone(),
        job_runner.clone(),
        modules::runtime::config::current()
            .memory()
            .compiler()
            .clone(),
    ));

    // MEM-MOD-P5 — wire the reflection pulse using the same utility
    // LLM the rolling summarizer / compiler share.  `memory_provider`
    // is built later in this function so we attach the runtime *after*
    // it exists; until then the ticker stays without a runtime and the
    // reflection branch is a no-op.
    let memory_ticker_base = modules::memory::MemoryTicker::new(
        rolling_summarizer.clone(),
        memory_compiler.clone(),
        summary_store.clone(),
        modules::memory::TickerConfig::default(),
    );

    let pinned_store: Arc<dyn modules::memory::PinnedStore> =
        match modules::memory::SqlitePinnedStore::open(
            &summary_db_path,
            paths.memory_root.clone(),
            Some(threat_scanner.clone()),
        ) {
            Ok(store) => {
                tracing::info!(
                    "[init] PinnedStore initialised at {:?} (sidecar {:?}/pinned.md)",
                    summary_db_path,
                    paths.memory_root
                );
                Arc::new(store)
            }
            Err(e) => {
                tracing::error!(
                    "[init] PinnedStore failed to open {summary_db_path:?}: {e}; using NullPinnedStore (pin writes will error)"
                );
                Arc::new(modules::memory::NullPinnedStore::new())
            }
        };
    ensure_pinned_markdown_sentinel(&paths.memory_root);

    let memory_provider = create_memory_provider(threat_scanner);

    // MEM-MOD-P7 — open the learned-traits store against the same
    // SQLite db file the memory provider already manages.  Independent
    // connection on purpose: the trait extractor runs on session-end
    // tasks that should not contend with the recall hot path.
    let learned_traits =
        match modules::memory::learned_traits::LearnedTraitsStore::open(&summary_db_path) {
            Ok(store) => {
                tracing::info!(
                    "[init] LearnedTraitsStore initialised at {:?}",
                    summary_db_path
                );
                Some(store)
            }
            Err(e) => {
                tracing::error!(
                    "[init] LearnedTraitsStore failed to open {summary_db_path:?}: {e}; \
                 cross-session trait accumulation disabled this run"
                );
                None
            }
        };

    // MEM-MOD-P5 + P7 — finish the ticker by attaching every runtime
    // we now have.  `with_reflection_runtime` always wires; the P7
    // runtime is only attached when the store opened successfully.
    let mut ticker_finished =
        memory_ticker_base.with_reflection_runtime(utility_llm.clone(), memory_provider.clone());
    if let Some(ref store) = learned_traits {
        ticker_finished = ticker_finished.with_learned_traits_runtime(
            utility_llm.clone(),
            store.clone(),
            memory_provider.clone(),
        );
    }
    let memory_ticker = Arc::new(ticker_finished);

    MemoryBootstrap {
        job_runner,
        utility_llm,
        summary_store,
        rolling_summarizer,
        memory_compiler,
        memory_ticker,
        pinned_store,
        memory_provider,
        learned_traits,
    }
}

fn ensure_pinned_markdown_sentinel(memory_root: &Path) {
    let pinned_md = memory_root.join("pinned.md");
    if !pinned_md.exists() {
        if let Err(e) = std::fs::write(&pinned_md, "") {
            tracing::warn!(
                path = %pinned_md.display(),
                error = %e,
                "[init] failed to touch pinned.md sentinel; build_memory_injection will skip compiled section"
            );
        }
    }
}

fn open_job_runner_with_fallback(memory_root: &Path) -> modules::memory::JobRunner {
    let primary = memory_root.join("jobs.db");
    match modules::memory::JobRunner::open(&primary, 3, 3) {
        Ok(runner) => runner,
        Err(e) => {
            tracing::error!(
                "[init] JobRunner failed to open {primary:?}: {e}; falling back to ephemeral jobs.db (failure counts will not survive restart)"
            );
            let tmp =
                std::env::temp_dir().join(format!("if2ai-jobs-fallback-{}.db", std::process::id()));
            match modules::memory::JobRunner::open(&tmp, 3, 3) {
                Ok(runner) => runner,
                Err(e2) => {
                    tracing::error!(
                        "[init] Ephemeral JobRunner open at {tmp:?} also failed: {e2}; aborting startup"
                    );
                    std::process::exit(1);
                }
            }
        }
    }
}

fn create_memory_provider(
    scanner: Arc<modules::memory::security::ThreatScanner>,
) -> modules::memory::SharedMemoryProvider {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            tracing::error!(
                "[memory] Failed to create tokio runtime for memory init: {e}, falling back to SQLite"
            );
            return create_sqlite_provider(scanner);
        }
    };

    let hrr_enabled = std::env::var("IF2AI_HRR_ENABLED")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);

    if hrr_enabled {
        match runtime.block_on(create_hybrid_provider()) {
            Ok(provider) => {
                tracing::info!("[memory] HybridMemoryProvider (HRR + Vector) initialized");
                return provider;
            }
            Err(e) => {
                tracing::warn!("[memory] HybridMemoryProvider failed: {e}, falling back to Vector");
            }
        }
    }

    let vector_result = runtime.block_on(async {
        tokio::time::timeout(std::time::Duration::from_secs(30), async {
            // MEM-MOD-PATH-FIX — single root via if2ai_data_root().
            let memory_root = modules::config::store::if2ai_data_root().join("memory");
            let db_path = memory_root.join("vector_db");
            let sqlite_path = Some(memory_root.join("memory.db"));
            tracing::info!(
                memory_root = %memory_root.display(),
                vector_db_path = %db_path.display(),
                sqlite_path = %sqlite_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "none".to_string()),
                "[memory] resolving VectorMemoryProvider paths"
            );

            let config = modules::memory::VectorProviderConfig {
                db_path,
                vector_search_enabled: true,
                sqlite_path,
            };

            modules::memory::VectorMemoryProvider::new(config).await
        })
        .await
    });

    match vector_result {
        Ok(Ok(provider)) => {
            tracing::info!("[memory] VectorMemoryProvider initialized successfully");
            let provider = provider.with_scanner(scanner);
            Arc::new(provider) as modules::memory::SharedMemoryProvider
        }
        Ok(Err(e)) => {
            tracing::warn!(
                "[memory] VectorMemoryProvider initialization failed: {e}, falling back to SQLite"
            );
            create_sqlite_provider(scanner)
        }
        Err(_) => {
            tracing::warn!(
                "[memory] VectorMemoryProvider timed out after 30s, falling back to SQLite"
            );
            create_sqlite_provider(scanner)
        }
    }
}

async fn create_hybrid_provider() -> Result<modules::memory::SharedMemoryProvider, String> {
    use crate::modules::memory::hrr::integration::HybridConfig;

    let hrr_config = HybridConfig {
        hrr_enabled: true,
        hrr_capacity: 0,
    };

    let provider = modules::memory::hrr::integration::HybridMemoryProvider::new(hrr_config)
        .await
        .map_err(|e| e.to_string())?;

    Ok(Arc::new(provider) as modules::memory::SharedMemoryProvider)
}

fn create_sqlite_provider(
    scanner: Arc<modules::memory::security::ThreatScanner>,
) -> modules::memory::SharedMemoryProvider {
    // MEM-MOD-PATH-FIX — single root via if2ai_data_root().
    let db_path = modules::config::store::if2ai_data_root()
        .join("memory")
        .join("memory.db");
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match modules::memory::SqliteMemoryProvider::new(db_path) {
        Ok(provider) => {
            Arc::new(provider.with_scanner(scanner)) as modules::memory::SharedMemoryProvider
        }
        Err(e) => {
            tracing::error!(
                "[memory] Failed to create SqliteMemoryProvider: {e}, falling back to in-memory"
            );
            #[allow(deprecated)]
            let fallback = modules::memory::InMemoryMemoryProvider::new();
            Arc::new(fallback) as modules::memory::SharedMemoryProvider
        }
    }
}
