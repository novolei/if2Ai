use async_trait::async_trait;
use if2ai_backend::modules::application::turn_service::hook_registry::{
    FailurePolicy, HookEntry, HookRegistry, TestHookCtx, TurnHookSimple,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct FailingHook;
#[async_trait]
impl TurnHookSimple for FailingHook {
    async fn call(&self, _: &TestHookCtx) -> Result<(), String> {
        Err("boom".into())
    }
    fn name(&self) -> &str {
        "failing"
    }
}
struct ShouldNotRun {
    ran: Arc<Mutex<bool>>,
}
#[async_trait]
impl TurnHookSimple for ShouldNotRun {
    async fn call(&self, _: &TestHookCtx) -> Result<(), String> {
        *self.ran.lock().unwrap() = true;
        Ok(())
    }
    fn name(&self) -> &str {
        "should_not_run"
    }
}

#[tokio::test]
async fn fail_closed_aborts_chain() {
    let ran = Arc::new(Mutex::new(false));
    let mut reg = HookRegistry::default();
    reg.register(HookEntry {
        hook: Arc::new(FailingHook),
        priority: 10,
        timeout: Duration::from_secs(1),
        on_failure: FailurePolicy::FailClosed,
    });
    reg.register(HookEntry {
        hook: Arc::new(ShouldNotRun { ran: ran.clone() }),
        priority: 0,
        timeout: Duration::from_secs(1),
        on_failure: FailurePolicy::FailOpen,
    });
    let r = reg.run_all(&TestHookCtx::default()).await;
    assert!(r.is_err());
    assert!(!*ran.lock().unwrap(), "chain aborted before second hook");
}
