use async_trait::async_trait;
use if2ai_backend::modules::application::turn_service::hook_registry::{
    FailurePolicy, HookCallCtx, HookEntry, HookRegistry, TurnHookSimple,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct FailingHook;
#[async_trait]
impl TurnHookSimple for FailingHook {
    async fn call(&self, _ctx: &HookCallCtx<'_>) -> Result<(), String> {
        Err("boom".into())
    }
    fn name(&self) -> &str {
        "failing"
    }
}
struct RecordingHook {
    ran: Arc<Mutex<bool>>,
}
#[async_trait]
impl TurnHookSimple for RecordingHook {
    async fn call(&self, _ctx: &HookCallCtx<'_>) -> Result<(), String> {
        *self.ran.lock().unwrap() = true;
        Ok(())
    }
    fn name(&self) -> &str {
        "recording"
    }
}

#[tokio::test]
async fn fail_open_continues_chain() {
    let ran = Arc::new(Mutex::new(false));
    let mut reg = HookRegistry::default();
    reg.register(HookEntry {
        hook: Arc::new(FailingHook),
        priority: 10,
        timeout: Duration::from_secs(1),
        on_failure: FailurePolicy::FailOpen,
    });
    reg.register(HookEntry {
        hook: Arc::new(RecordingHook { ran: ran.clone() }),
        priority: 0,
        timeout: Duration::from_secs(1),
        on_failure: FailurePolicy::FailOpen,
    });
    let r = reg.run_all(&HookCallCtx::default()).await;
    assert!(r.is_ok(), "fail-open chain returns Ok overall");
    assert!(*ran.lock().unwrap(), "subsequent hook ran");
}

#[tokio::test]
async fn fail_open_timeout_continues_chain() {
    struct SlowHook;
    #[async_trait]
    impl TurnHookSimple for SlowHook {
        async fn call(&self, _: &HookCallCtx<'_>) -> Result<(), String> {
            tokio::time::sleep(Duration::from_secs(60)).await;
            Ok(())
        }
        fn name(&self) -> &str {
            "slow"
        }
    }
    let ran = Arc::new(Mutex::new(false));
    let mut reg = HookRegistry::default();
    reg.register(HookEntry {
        hook: Arc::new(SlowHook),
        priority: 10,
        timeout: Duration::from_millis(50),
        on_failure: FailurePolicy::FailOpen,
    });
    reg.register(HookEntry {
        hook: Arc::new(RecordingHook { ran: ran.clone() }),
        priority: 0,
        timeout: Duration::from_secs(1),
        on_failure: FailurePolicy::FailOpen,
    });
    let r = reg.run_all(&HookCallCtx::default()).await;
    assert!(r.is_ok());
    assert!(*ran.lock().unwrap());
}
