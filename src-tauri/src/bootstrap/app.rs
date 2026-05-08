use std::sync::Arc;

use crate::commands;
use crate::modules;

pub fn build_app_bootstrap(paths: &super::BootPaths) -> super::AppBootstrap {
    let session_manager = modules::session::SessionManager::new(
        paths.sessions_dir.clone(),
        paths.projects_dir.clone(),
    );
    let default_tool_context =
        modules::tools::ToolContext::default_for_workdir(std::path::PathBuf::from("."));
    let tool_registry =
        modules::tools::ToolRegistry::new(Arc::new(std::sync::Mutex::new(default_tool_context)));
    let threat_scanner =
        Arc::new(modules::memory::security::ThreatScanner::with_builtin_patterns());
    let memory_bootstrap = super::memory::build_memory_bootstrap(paths, threat_scanner.clone());

    let scheduler_provider = modules::scheduler::default_scheduler();
    let browser_profile_mode =
        modules::browser::BrowserProfileMode::from_env_or_config(&paths.if2ai_dir);
    let browser_registry = modules::browser::BrowserRegistry::new(
        paths.if2ai_dir.join("browser-cold-state.json"),
        browser_profile_mode,
        paths.if2ai_dir.clone(),
    );
    modules::tools::register_builtin_tools(
        &tool_registry,
        memory_bootstrap.memory_provider.clone(),
        scheduler_provider,
        browser_registry.clone(),
        memory_bootstrap.pinned_store.clone(),
        // MEM-MOD-P4 — share the same utility-LLM the summarizer /
        // compiler use, so `memory_store`'s decision tree (when the
        // feature flag is on) routes through the configured provider.
        Some(memory_bootstrap.utility_llm.clone()),
    );

    let project_manager = modules::projects::ProjectManager::new(paths.projects_dir.clone());

    // GAP-004 (T-009): Bundle core services into a ServiceRegistry so
    // AppState holds a single Arc<ServiceRegistry> instead of ad-hoc
    // fields.  Individual Arc<> refs are extracted in AppState::new
    // for backward-compatible direct field access.
    let service_registry =
        modules::application::ServiceRegistry::new(session_manager, tool_registry, project_manager);

    let onboarding_flow = modules::onboarding::flow::OnboardingFlow;
    let context_budget = load_context_budget();
    let trajectory_manager = init_trajectory_manager(paths);
    let learning_module = init_learning_module(memory_bootstrap.memory_provider.clone());
    let active_retrieval_manager = Some(Arc::new(
        modules::memory::retrieval::ActiveRetrievalManager::with_defaults(),
    ));
    let harness = init_harness(paths);

    super::AppBootstrap {
        app_state_config: commands::AppStateConfig {
            service_registry,
            memory_provider: memory_bootstrap.memory_provider,
            context_budget,
            onboarding_flow,
            trajectory_manager,
            learning_module,
            active_retrieval_manager,
            harness,
            threat_scanner,
            job_runner: memory_bootstrap.job_runner,
            utility_llm: memory_bootstrap.utility_llm,
            summary_store: memory_bootstrap.summary_store,
            rolling_summarizer: memory_bootstrap.rolling_summarizer,
            pinned_store: memory_bootstrap.pinned_store,
            memory_compiler: memory_bootstrap.memory_compiler,
            memory_ticker: memory_bootstrap.memory_ticker.clone(),
            daydream_coordinator: memory_bootstrap.daydream_coordinator.clone(),
            trajectory_collector: memory_bootstrap.trajectory_collector.clone(),
        },
        browser_registry,
        memory_ticker: memory_bootstrap.memory_ticker,
        learned_traits: memory_bootstrap.learned_traits,
    }
}

fn init_harness(paths: &super::BootPaths) -> Option<Arc<modules::harness::HarnessState>> {
    if std::env::var("IF2AI_HARNESS_ENABLED").as_deref() == Ok("1") {
        let trace_dir = paths.if2ai_dir.join("traces");
        tracing::info!("[init] Harness enabled; traces → {:?}", trace_dir);
        Some(Arc::new(modules::harness::HarnessState::new(trace_dir)))
    } else {
        None
    }
}

fn init_trajectory_manager(
    paths: &super::BootPaths,
) -> Option<Arc<modules::learning::trajectory::TrajectoryManager>> {
    let trajectories_dir = paths.if2ai_dir.join("trajectories");
    match modules::learning::trajectory::TrajectoryManager::new(trajectories_dir.clone()) {
        Ok(manager) => {
            tracing::info!(
                "[init] TrajectoryManager initialised at {:?}",
                trajectories_dir
            );
            Some(Arc::new(manager))
        }
        Err(e) => {
            tracing::warn!(
                "[init] TrajectoryManager failed to initialise: {e}; trajectory recording disabled"
            );
            None
        }
    }
}

fn init_learning_module(
    memory_provider: modules::memory::SharedMemoryProvider,
) -> Option<Arc<tokio::sync::Mutex<modules::learning::LearningModule>>> {
    match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => match rt.block_on(modules::learning::LearningModule::new(memory_provider)) {
            Ok(module) => {
                tracing::info!("[init] LearningModule initialised");
                Some(Arc::new(tokio::sync::Mutex::new(module)))
            }
            Err(e) => {
                tracing::warn!(
                    "[init] LearningModule failed to initialise: {e}; self-learning disabled"
                );
                None
            }
        },
        Err(e) => {
            tracing::warn!("[init] Could not create tokio runtime for LearningModule init: {e}");
            None
        }
    }
}

fn load_context_budget() -> modules::runtime::budget::ContextBudget {
    let path = dirs::home_dir()
        .map(|h| h.join(".if2ai").join("budget.yaml"))
        .unwrap_or_else(|| std::path::PathBuf::from(".if2ai/budget.yaml"));
    modules::runtime::budget::BudgetConfig::load_or_default(&path)
}
