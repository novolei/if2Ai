//! Prioritized, timed, failure-policy-aware hook chain.
//!
//! Mirrors Steward's `HookRegistry`. Existing `TurnHook` implementations
//! migrate (Task 4.2) by being wrapped in `HookEntry { priority: 0,
//! timeout: 30s, on_failure: FailOpen }` via `LegacyHookAdapter`.
//!
//! The trait is intentionally minimal (`TurnHookSimple` with a single
//! `call` method and a `HookCallCtx`). `HookCallCtx` carries the same
//! borrowed payload the legacy
//! [`crate::modules::runtime::conversation::TurnHook`] receives
//! (`scope`, `session_id`, `messages`) so `LegacyHookAdapter` can wrap
//! existing impls without rewriting their internals. All fields are
//! `Option` so unit tests can construct an empty `HookCallCtx::default()`
//! when payload is irrelevant.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

use crate::modules::memory::scope::MemoryExecutionScope;
use crate::modules::runtime::session::ConversationMessage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailurePolicy {
    /// Hook failure (Err or timeout) is logged via `tracing::warn!` and the
    /// chain continues.
    FailOpen,
    /// Hook failure aborts the chain and propagates `Err` to the caller.
    FailClosed,
}

/// Context handed to each `TurnHookSimple::call`.
///
/// All fields are optional so tests can pass `HookCallCtx::default()`.
/// Production callers populate the borrowed slots so adapters such as
/// [`LegacyHookAdapter`] can forward them to the legacy
/// [`crate::modules::runtime::conversation::TurnHook`] surface.
#[derive(Debug, Default, Clone)]
pub struct HookCallCtx<'a> {
    pub scope: Option<&'a MemoryExecutionScope>,
    pub session_id: Option<&'a str>,
    pub messages: Option<&'a [ConversationMessage]>,
}

#[async_trait]
pub trait TurnHookSimple: Send + Sync {
    async fn call(&self, ctx: &HookCallCtx<'_>) -> Result<(), String>;
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

    pub async fn run_all(&self, ctx: &HookCallCtx<'_>) -> Result<(), String> {
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

/// Adapter that wraps the legacy
/// [`crate::modules::runtime::conversation::TurnHook`] trait into
/// `TurnHookSimple` so existing impls (e.g. `MemoryTicker`) drop into a
/// `HookRegistry` without rewriting their internals.
///
/// The legacy trait's `on_turn_complete` is sync and infallible
/// (returns `()`); the adapter therefore always returns `Ok(())` after
/// invoking it. The legacy hook expects `scope`, `session_id`, and
/// `messages` — if any are missing on the [`HookCallCtx`] the adapter
/// returns `Err`, which the surrounding `FailurePolicy` handles per the
/// usual chain semantics.
///
/// `on_session_end` is intentionally NOT routed through this adapter:
/// the registry's single `call` entry point models the per-turn signal.
/// Session-end fan-out remains a direct call site.
pub struct LegacyHookAdapter<H> {
    inner: Arc<H>,
    name: &'static str,
}

impl<H> LegacyHookAdapter<H> {
    pub fn new(name: &'static str, inner: Arc<H>) -> Self {
        Self { inner, name }
    }
}

#[async_trait]
impl<H> TurnHookSimple for LegacyHookAdapter<H>
where
    H: crate::modules::runtime::conversation::TurnHook + Send + Sync + 'static,
{
    async fn call(&self, ctx: &HookCallCtx<'_>) -> Result<(), String> {
        let scope = ctx
            .scope
            .ok_or_else(|| "LegacyHookAdapter: HookCallCtx.scope is required".to_string())?;
        let session_id = ctx
            .session_id
            .ok_or_else(|| "LegacyHookAdapter: HookCallCtx.session_id is required".to_string())?;
        let messages = ctx
            .messages
            .ok_or_else(|| "LegacyHookAdapter: HookCallCtx.messages is required".to_string())?;
        self.inner.on_turn_complete(scope, session_id, messages);
        Ok(())
    }
    fn name(&self) -> &str {
        self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_chain_returns_ok() {
        let r = HookRegistry::default()
            .run_all(&HookCallCtx::default())
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
            async fn call(&self, _: &HookCallCtx<'_>) -> Result<(), String> {
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
        reg.run_all(&HookCallCtx::default()).await.unwrap();
        assert_eq!(*log.lock().unwrap(), vec!["first", "second"]);
    }
}
