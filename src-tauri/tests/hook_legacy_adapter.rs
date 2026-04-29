//! Verifies LegacyHookAdapter wraps an existing `TurnHook` impl into
//! HookRegistry's `TurnHookSimple`-style chain and forwards the borrowed
//! payload (`scope`, `session_id`, `messages`) intact.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use if2ai_backend::modules::application::turn_service::hook_registry::{
    FailurePolicy, HookCallCtx, HookEntry, HookRegistry, LegacyHookAdapter,
};
use if2ai_backend::modules::memory::scope::MemoryExecutionScope;
use if2ai_backend::modules::runtime::conversation::TurnHook;
use if2ai_backend::modules::runtime::session::ConversationMessage;

#[derive(Default)]
struct CountingHook {
    calls: Mutex<Vec<(String, usize)>>,
}

impl TurnHook for CountingHook {
    fn on_turn_complete(
        &self,
        _scope: &MemoryExecutionScope,
        session_id: &str,
        messages: &[ConversationMessage],
    ) {
        self.calls
            .lock()
            .unwrap()
            .push((session_id.to_string(), messages.len()));
    }
}

#[tokio::test]
async fn legacy_hook_invoked_through_registry() {
    let inner = Arc::new(CountingHook::default());
    let mut reg = HookRegistry::default();
    reg.register(HookEntry {
        hook: Arc::new(LegacyHookAdapter::new("legacy", inner.clone())),
        priority: 0,
        timeout: Duration::from_secs(1),
        on_failure: FailurePolicy::FailClosed,
    });

    let scope = MemoryExecutionScope {
        session_id: Some("sess-1".into()),
        project_id: None,
        workdir: None,
    };
    let messages: Vec<ConversationMessage> = Vec::new();
    let ctx = HookCallCtx {
        scope: Some(&scope),
        session_id: Some("sess-1"),
        messages: Some(&messages),
    };
    reg.run_all(&ctx).await.unwrap();

    let calls = inner.calls.lock().unwrap().clone();
    assert_eq!(calls, vec![("sess-1".to_string(), 0)]);
}

#[tokio::test]
async fn legacy_adapter_errors_when_ctx_missing_payload() {
    let inner = Arc::new(CountingHook::default());
    let mut reg = HookRegistry::default();
    reg.register(HookEntry {
        hook: Arc::new(LegacyHookAdapter::new("legacy", inner.clone())),
        priority: 0,
        timeout: Duration::from_secs(1),
        on_failure: FailurePolicy::FailClosed,
    });

    // Default ctx has all-None payload — adapter should refuse rather
    // than silently swallow a misuse.
    let r = reg.run_all(&HookCallCtx::default()).await;
    assert!(r.is_err(), "expected fail-closed Err on missing payload");
    assert!(inner.calls.lock().unwrap().is_empty());
}
