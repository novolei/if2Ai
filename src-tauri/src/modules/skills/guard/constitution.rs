//! FEAT-SE-003 — Constitution layer for sedimented skill drafts.
//!
//! Every auto-generated [`SkillDraft`] passes through the 8 hard rules
//! below before it is allowed into dedup → vector index → persistence.
//! Rules are static (no LLM, no I/O) so the check is cheap, deterministic,
//! and unit-testable without infrastructure.
//!
//! Independent path: this module does NOT touch
//! [`crate::modules::skills::guard::SkillsGuard::scan_command`] or its
//! existing threat_patterns surface. Future wiring Pack will plug
//! `evaluate_constitution` into the sedimentation pipeline.
//!
//! Adding new rules: append to [`RULES`] and add a positive + negative
//! test in `tests/skill_evolution.rs`.

#![allow(dead_code)]

use crate::modules::skills::sedimentation::SkillDraft;

/// Severity of a [`Violation`]. Higher variants override lower in
/// [`evaluate_constitution`]'s sort order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Medium,
    High,
    Critical,
}

/// One concrete rule violation. Returned by
/// [`ConstitutionRule::predicate`] when the rule fires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub rule_id: &'static str,
    pub severity: Severity,
    pub matched_snippet: String,
    pub explanation: &'static str,
}

/// One constitution rule. `predicate` returns `Some(violation)` when
/// the rule fires; `None` when the draft passes.
#[derive(Debug, Clone, Copy)]
pub struct ConstitutionRule {
    pub id: &'static str,
    pub severity: Severity,
    pub predicate: fn(&SkillDraft) -> Option<Violation>,
}

/// Static rule registry. Keep entries ordered by `id` for stable
/// trace output; sort-by-severity happens in [`evaluate_constitution`].
pub const RULES: &[ConstitutionRule] = &[
    ConstitutionRule {
        id: "ROOT_DELETE",
        severity: Severity::Critical,
        predicate: rule_root_delete,
    },
    ConstitutionRule {
        id: "SUDO_ESCALATION",
        severity: Severity::Critical,
        predicate: rule_sudo_escalation,
    },
    ConstitutionRule {
        id: "OVERWRITE_IF2AI_CONFIG",
        severity: Severity::High,
        predicate: rule_overwrite_if2ai_config,
    },
    ConstitutionRule {
        id: "FORK_BOMB",
        severity: Severity::Critical,
        predicate: rule_fork_bomb,
    },
    ConstitutionRule {
        id: "NETWORK_SCAN",
        severity: Severity::High,
        predicate: rule_network_scan,
    },
    ConstitutionRule {
        id: "SECRET_LEAK",
        severity: Severity::High,
        predicate: rule_secret_leak,
    },
    ConstitutionRule {
        id: "EVAL_UNTRUSTED",
        severity: Severity::High,
        predicate: rule_eval_untrusted,
    },
    ConstitutionRule {
        id: "KEYBOARD_LOGGER",
        severity: Severity::Critical,
        predicate: rule_keyboard_logger,
    },
];

/// Evaluate every rule against `draft`. Returned violations are
/// sorted by severity descending (Critical → High → Medium); within
/// the same severity, original [`RULES`] order is preserved.
#[must_use]
pub fn evaluate_constitution(draft: &SkillDraft) -> Vec<Violation> {
    let mut hits: Vec<Violation> = RULES.iter().filter_map(|r| (r.predicate)(draft)).collect();
    hits.sort_by_key(|v| std::cmp::Reverse(v.severity));
    hits
}

// ---------------------------------------------------------------------------
// Rule predicates
// ---------------------------------------------------------------------------

fn rule_root_delete(draft: &SkillDraft) -> Option<Violation> {
    let body = lowercase_body(draft);
    let needles = [
        "rm -rf /",
        "rm -rf / ",
        "rm -fr /",
        "rm -rf /*",
        "rmdir /s /q c:\\",
        "format c:",
    ];
    let hit = needles.iter().find(|n| body.contains(*n))?;
    Some(Violation {
        rule_id: "ROOT_DELETE",
        severity: Severity::Critical,
        matched_snippet: hit.to_string(),
        explanation: "Skill attempts to delete the filesystem root.",
    })
}

fn rule_sudo_escalation(draft: &SkillDraft) -> Option<Violation> {
    let body = lowercase_body(draft);
    let needles = [
        "sudo ",
        "doas ",
        "su -",
        "runas /user:administrator",
        "pkexec ",
    ];
    let hit = needles.iter().find(|n| body.contains(*n))?;
    Some(Violation {
        rule_id: "SUDO_ESCALATION",
        severity: Severity::Critical,
        matched_snippet: hit.to_string(),
        explanation: "Skill escalates privileges via sudo / admin.",
    })
}

fn rule_overwrite_if2ai_config(draft: &SkillDraft) -> Option<Violation> {
    let body = lowercase_body(draft);
    // Match writes/deletes targeting the user's if2ai config dir.
    let patterns = [
        "~/.if2ai/",
        "$home/.if2ai",
        "$home/.if2ai/",
        "rm ~/.if2ai",
        "echo > ~/.if2ai",
        "tee ~/.if2ai",
        "cp ~/.if2ai",
    ];
    let hit = patterns.iter().find(|n| body.contains(*n))?;
    Some(Violation {
        rule_id: "OVERWRITE_IF2AI_CONFIG",
        severity: Severity::High,
        matched_snippet: hit.to_string(),
        explanation: "Skill mutates the user's ~/.if2ai/ configuration.",
    })
}

fn rule_fork_bomb(draft: &SkillDraft) -> Option<Violation> {
    let body = draft.body.to_lowercase();
    if body.contains(":(){ :|:& };:") || body.contains(":(){:|:&};:") {
        return Some(Violation {
            rule_id: "FORK_BOMB",
            severity: Severity::Critical,
            matched_snippet: ":(){ :|:& };:".to_string(),
            explanation: "Classic shell fork-bomb signature detected.",
        });
    }
    None
}

fn rule_network_scan(draft: &SkillDraft) -> Option<Violation> {
    let body = lowercase_body(draft);
    let needles = ["nmap ", "masscan ", "zmap ", "nikto "];
    let hit = needles.iter().find(|n| body.contains(*n))?;
    Some(Violation {
        rule_id: "NETWORK_SCAN",
        severity: Severity::High,
        matched_snippet: hit.to_string(),
        explanation: "Skill performs network reconnaissance.",
    })
}

fn rule_secret_leak(draft: &SkillDraft) -> Option<Violation> {
    let body = lowercase_body(draft);
    // Heuristic: prints / curls / posts an api key / token / secret env var.
    let exfil_verbs = ["curl ", "wget ", "echo $", "printenv ", "env | "];
    let secret_keys = ["api_key", "apikey", "secret", "token", "password"];
    let mentions_verb = exfil_verbs.iter().find(|v| body.contains(*v));
    let mentions_secret = secret_keys.iter().find(|k| body.contains(*k));
    match (mentions_verb, mentions_secret) {
        (Some(v), Some(k)) => Some(Violation {
            rule_id: "SECRET_LEAK",
            severity: Severity::High,
            matched_snippet: format!("{v} ... {k}"),
            explanation: "Skill exfiltrates secrets via shell.",
        }),
        _ => None,
    }
}

fn rule_eval_untrusted(draft: &SkillDraft) -> Option<Violation> {
    let body = lowercase_body(draft);
    let needles = [
        "eval $(curl",
        "eval `curl",
        "exec($_get",
        "exec($_post",
        "bash <(curl",
        "sh <(curl",
        "powershell -enc",
    ];
    let hit = needles.iter().find(|n| body.contains(*n))?;
    Some(Violation {
        rule_id: "EVAL_UNTRUSTED",
        severity: Severity::High,
        matched_snippet: hit.to_string(),
        explanation: "Skill evals/execs code fetched from untrusted source.",
    })
}

fn rule_keyboard_logger(draft: &SkillDraft) -> Option<Violation> {
    let body = lowercase_body(draft);
    let needles = [
        "pynput.keyboard",
        "keylogger",
        "import keyboard",
        "xinput test",
    ];
    let hit = needles.iter().find(|n| body.contains(*n))?;
    Some(Violation {
        rule_id: "KEYBOARD_LOGGER",
        severity: Severity::Critical,
        matched_snippet: hit.to_string(),
        explanation: "Skill installs / uses a keyboard logger.",
    })
}

fn lowercase_body(draft: &SkillDraft) -> String {
    draft.body.to_lowercase()
}
