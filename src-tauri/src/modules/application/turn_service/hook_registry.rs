//! Prioritized, timed, failure-policy-aware hook chain.
//!
//! Mirrors Steward's `HookRegistry`. Existing `TurnHook` implementations
//! migrate (Task 4.2) by being wrapped in `HookEntry { priority: 0,
//! timeout: 30s, on_failure: FailOpen }` via `LegacyHookAdapter`.
//!
//! For 4.1 the trait is intentionally minimal (`TurnHookSimple` with a
//! single `call` method and a `TestHookCtx`) so unit tests can drive it
//! without depending on the wider runtime types. Production migration in
//! 4.2 may add a richer context type or a parallel trait specialization.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailurePolicy {
    /// Hook failure (Err or timeout) is logged via `tracing::warn!` and the
    /// chain continues.
    FailOpen,
    /// Hook failure aborts the chain and propagates `Err` to the caller.
    FailClosed,
}

/// Minimal context for unit tests. Production hook contexts (TurnContext,
/// TurnOutcome, etc.) layer on top via wrapper types in Task 4.2.
#[derive(Debug, Default, Clone)]
pub struct TestHookCtx;

#[async_trait]
pub trait TurnHookSimple: Send + Sync {
    async fn call(&self, ctx: &TestHookCtx) -> Result<(), String>;
    fn name(&self) -> &str {
        "anonymous"
    }
}

pub struct HookEntry {
    pub hook: Arc<dyn TurnHookSimple>,
    pub priority: i32,
    pub timeout: Duration,
    pub on_failure: FailurePolicy,
}

#[derive(Default)]
pub struct HookRegistry {
    entries: Vec<HookEntry>,
}

impl HookRegistry {
    pub fn register(&mut self, entry: HookEntry) {
        self.entries.push(entry);
        // Stable sort by priority descending so equal-priority hooks preserve
        // registration order (operationally easier to reason about).
        self.entries
            .sort_by_key(|entry| std::cmp::Reverse(entry.priority));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub async fn run_all(&self, ctx: &TestHookCtx) -> Result<(), String> {
        for entry in &self.entries {
            let res = tokio::time::timeout(entry.timeout, entry.hook.call(ctx)).await;
            let outcome: Result<(), String> = match res {
                Ok(Ok(())) => Ok(()),
                Ok(Err(e)) => Err(e),
                Err(_) => Err(format!("hook '{}' timed out", entry.hook.name())),
            };
            match (outcome, entry.on_failure) {
                (Ok(()), _) => continue,
                (Err(e), FailurePolicy::FailOpen) => {
                    tracing::warn!(
                        target: "turn_service::hook",
                        "hook '{}' failed (fail-open): {}", entry.hook.name(), e
                    );
                    continue;
                }
                (Err(e), FailurePolicy::FailClosed) => {
                    tracing::error!(
                        target: "turn_service::hook",
                        "hook '{}' failed (fail-closed): {}", entry.hook.name(), e
                    );
                    return Err(e);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_chain_returns_ok() {
        let r = HookRegistry::default()
            .run_all(&TestHookCtx::default())
            .await;
        assert!(r.is_ok());
    }

    #[tokio::test]
    async fn equal_priority_preserves_registration_order() {
        use std::sync::Mutex;
        let log: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));

        struct R {
            name: &'static str,
            log: Arc<Mutex<Vec<&'static str>>>,
        }
        #[async_trait]
        impl TurnHookSimple for R {
            async fn call(&self, _: &TestHookCtx) -> Result<(), String> {
                self.log.lock().unwrap().push(self.name);
                Ok(())
            }
            fn name(&self) -> &str {
                self.name
            }
        }

        let mut reg = HookRegistry::default();
        reg.register(HookEntry {
            hook: Arc::new(R {
                name: "first",
                log: log.clone(),
            }),
            priority: 5,
            timeout: Duration::from_secs(1),
            on_failure: FailurePolicy::FailOpen,
        });
        reg.register(HookEntry {
            hook: Arc::new(R {
                name: "second",
                log: log.clone(),
            }),
            priority: 5,
            timeout: Duration::from_secs(1),
            on_failure: FailurePolicy::FailOpen,
        });
        reg.run_all(&TestHookCtx::default()).await.unwrap();
        assert_eq!(*log.lock().unwrap(), vec!["first", "second"]);
    }
}
