//! Verifies HookRegistry executes hooks in priority order (highest first).

use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use if2ai_backend::modules::application::turn_service::hook_registry::{
    FailurePolicy, HookEntry, HookRegistry, TestHookCtx, TurnHookSimple,
};

struct RecordingHook {
    name: &'static str,
    log: Arc<Mutex<Vec<&'static str>>>,
}

#[async_trait]
impl TurnHookSimple for RecordingHook {
    async fn call(&self, _ctx: &TestHookCtx) -> Result<(), String> {
        self.log.lock().unwrap().push(self.name);
        Ok(())
    }
    fn name(&self) -> &str {
        self.name
    }
}

#[tokio::test]
async fn higher_priority_runs_first() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut reg = HookRegistry::default();
    reg.register(HookEntry {
        hook: Arc::new(RecordingHook {
            name: "low",
            log: log.clone(),
        }),
        priority: -5,
        timeout: Duration::from_secs(1),
        on_failure: FailurePolicy::FailOpen,
    });
    reg.register(HookEntry {
        hook: Arc::new(RecordingHook {
            name: "high",
            log: log.clone(),
        }),
        priority: 10,
        timeout: Duration::from_secs(1),
        on_failure: FailurePolicy::FailOpen,
    });
    reg.register(HookEntry {
        hook: Arc::new(RecordingHook {
            name: "mid",
            log: log.clone(),
        }),
        priority: 0,
        timeout: Duration::from_secs(1),
        on_failure: FailurePolicy::FailOpen,
    });

    reg.run_all(&TestHookCtx::default()).await.unwrap();
    assert_eq!(*log.lock().unwrap(), vec!["high", "mid", "low"]);
}
