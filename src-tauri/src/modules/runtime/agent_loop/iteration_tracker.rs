//! Periodic progress checks and three-stage failure escalation.
//!
//! [`IterationTracker`] is instantiated once per `run_agentic_loop` invocation
//! and consulted after each iteration. It injects system-level hints into
//! [`LoopContext`] when:
//!
//! 1. The loop has been running for a configurable number of iterations
//!    without a progress check (`progress_check_interval`).
//! 2. Working-memory should be re-injected (`memory_reinject_interval`).
//! 3. The loop approaches `max_iterations` (`wrap_up_threshold_pct`).
//! 4. The same tool has failed consecutively (three-stage escalation).
//!
//! All thresholds are driven by [`ProgressCheckConfig`].

use super::config::ProgressCheckConfig;

/// Tracks iteration progress and consecutive-failure state for one
/// `run_agentic_loop` invocation.
#[derive(Debug)]
pub struct IterationTracker {
    /// Total iterations configured for this loop run.
    max_iterations: usize,
    /// Snapshot of the progress-check config.
    cfg: ProgressCheckConfig,
    /// Number of consecutive failures for the *same* tool (or same error).
    consecutive_failures: usize,
    /// Tool name of the last failure (empty if none).
    last_failed_tool: String,
    /// Last error message (truncated to 256 chars for comparison).
    last_error_snippet: String,
    /// Whether the wrap-up warning has already been injected.
    wrap_up_warned: bool,
}

/// A hint that should be injected as a system message in `LoopContext`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressHint {
    pub content: String,
}

impl IterationTracker {
    /// Create a new tracker for a loop with the given `max_iterations`.
    pub fn new(max_iterations: usize, cfg: &ProgressCheckConfig) -> Self {
        Self {
            max_iterations,
            cfg: cfg.clone(),
            consecutive_failures: 0,
            last_failed_tool: String::new(),
            last_error_snippet: String::new(),
            wrap_up_warned: false,
        }
    }

    // ----- periodic progress checks ----------------------------------------

    /// Produce hints (if any) that should be injected *after* iteration
    /// `iteration` (0-based). The caller appends them to `LoopContext::injected`.
    pub fn check_progress(&mut self, iteration: usize) -> Vec<ProgressHint> {
        let mut hints = Vec::new();
        let iter_1based = iteration + 1; // human-readable

        // 1) Progress check every N iterations.
        if self.cfg.progress_check_interval > 0
            && iter_1based.is_multiple_of(self.cfg.progress_check_interval)
        {
            hints.push(ProgressHint {
                content: format!(
                    "[PROGRESS_CHECK] You have been running for {} iterations. \
                     Evaluate: are you making meaningful progress? If stuck, \
                     change your approach or ask for help.",
                    iter_1based,
                ),
            });
        }

        // 2) Memory re-injection every M iterations.
        if self.cfg.memory_reinject_interval > 0
            && iter_1based.is_multiple_of(self.cfg.memory_reinject_interval)
        {
            hints.push(ProgressHint {
                content: format!(
                    "[MEMORY_REINJECT] Iteration {}. Re-read your working memory \
                     and checkpoint information to avoid losing track of your plan.",
                    iter_1based,
                ),
            });
        }

        // 3) Approaching max_iterations — wrap-up warning (once).
        if !self.wrap_up_warned && self.max_iterations > 0 {
            let threshold = self.max_iterations / 100 * self.cfg.wrap_up_threshold_pct
                + (self.max_iterations % 100) * self.cfg.wrap_up_threshold_pct / 100;
            if iter_1based >= threshold {
                self.wrap_up_warned = true;
                hints.push(ProgressHint {
                    content: format!(
                        "[WRAP_UP_WARNING] You are at iteration {} of {}. \
                         Please wrap up your current task or summarize \
                         what you have accomplished so far.",
                        iter_1based, self.max_iterations,
                    ),
                });
            }
        }

        hints
    }

    // ----- three-stage failure escalation ----------------------------------

    /// Record a tool failure. Returns a hint if escalation is warranted.
    ///
    /// `tool_name` is the name of the tool that failed; `error_msg` is the
    /// (potentially long) error string. Only the first 256 chars of
    /// `error_msg` are used for dedup comparison.
    pub fn record_failure(&mut self, tool_name: &str, error_msg: &str) -> Option<ProgressHint> {
        let snippet: String = error_msg.chars().take(256).collect();

        if tool_name == self.last_failed_tool && snippet == self.last_error_snippet {
            self.consecutive_failures += 1;
        } else {
            self.consecutive_failures = 1;
            self.last_failed_tool = tool_name.to_owned();
            self.last_error_snippet = snippet;
        }

        self.failure_escalation_hint()
    }

    /// Reset the failure counter (e.g. after a successful tool call).
    pub fn reset_failures(&mut self) {
        self.consecutive_failures = 0;
        self.last_failed_tool.clear();
        self.last_error_snippet.clear();
    }

    /// Current consecutive failure count (for testing / diagnostics).
    pub fn consecutive_failures(&self) -> usize {
        self.consecutive_failures
    }

    // ----- internal --------------------------------------------------------

    fn failure_escalation_hint(&self) -> Option<ProgressHint> {
        match self.consecutive_failures {
            1 => Some(ProgressHint {
                content: "[RETRY_HINT] The previous tool call failed. \
                          Read the error message carefully and adjust your approach."
                    .into(),
            }),
            2 => Some(ProgressHint {
                content: "[PROBE_HINT] This tool has failed twice consecutively. \
                          Probe the environment state first — verify that \
                          preconditions are met before retrying."
                    .into(),
            }),
            n if n >= 3 => Some(ProgressHint {
                content: format!(
                    "[STRATEGY_SWITCH] This approach has failed {} times in a row. \
                     Switch to a completely different strategy or ask the user for help.",
                    n,
                ),
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_cfg() -> ProgressCheckConfig {
        ProgressCheckConfig::default()
    }

    #[test]
    fn progress_check_fires_at_interval() {
        let mut t = IterationTracker::new(50, &default_cfg());
        // iteration 6 (7th, 1-based) should trigger progress check.
        assert!(t.check_progress(5).is_empty());
        let hints = t.check_progress(6);
        assert!(hints.iter().any(|h| h.content.contains("[PROGRESS_CHECK]")));
    }

    #[test]
    fn memory_reinject_fires_at_interval() {
        let mut t = IterationTracker::new(50, &default_cfg());
        let hints = t.check_progress(9); // iteration 10
        assert!(hints
            .iter()
            .any(|h| h.content.contains("[MEMORY_REINJECT]")));
    }

    #[test]
    fn wrap_up_warning_fires_once() {
        let mut t = IterationTracker::new(50, &default_cfg());
        // threshold = 50 * 80 / 100 = 40 → fires at iteration 39 (1-based 40).
        let hints = t.check_progress(39);
        assert!(hints
            .iter()
            .any(|h| h.content.contains("[WRAP_UP_WARNING]")));
        // Second call should not fire again.
        let hints2 = t.check_progress(40);
        assert!(!hints2
            .iter()
            .any(|h| h.content.contains("[WRAP_UP_WARNING]")));
    }

    #[test]
    fn failure_escalation_three_stages() {
        let mut t = IterationTracker::new(50, &default_cfg());

        let h1 = t.record_failure("read_file", "not found");
        assert!(h1
            .as_ref()
            .is_some_and(|h| h.content.contains("[RETRY_HINT]")));

        let h2 = t.record_failure("read_file", "not found");
        assert!(h2
            .as_ref()
            .is_some_and(|h| h.content.contains("[PROBE_HINT]")));

        let h3 = t.record_failure("read_file", "not found");
        assert!(h3
            .as_ref()
            .is_some_and(|h| h.content.contains("[STRATEGY_SWITCH]")));
    }

    #[test]
    fn different_tool_resets_escalation() {
        let mut t = IterationTracker::new(50, &default_cfg());
        t.record_failure("read_file", "not found");
        t.record_failure("read_file", "not found");
        assert_eq!(t.consecutive_failures(), 2);

        // Different tool resets.
        t.record_failure("write_file", "permission denied");
        assert_eq!(t.consecutive_failures(), 1);
    }

    #[test]
    fn reset_failures_clears_state() {
        let mut t = IterationTracker::new(50, &default_cfg());
        t.record_failure("read_file", "not found");
        t.record_failure("read_file", "not found");
        t.reset_failures();
        assert_eq!(t.consecutive_failures(), 0);
    }
}
