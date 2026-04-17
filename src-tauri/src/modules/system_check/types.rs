//! System check type definitions.
//!
//! Defines the core types for the system pre-check flow (Step 2):
//! - `CheckStatus`: Result of a single check (Pass/Fail/Running/Pending)
//! - `MemoryInfo`: System RAM info
//! - `EmbeddedModelStatus`: Embedded model download status
//! - `SystemReport`: Full system check report

use serde::{Deserialize, Serialize};

// ── CheckStatus ─────────────────────────────────────────────────────────────

/// Status of a single system check.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CheckStatus {
    /// Check passed successfully.
    Pass,
    /// Check failed with a reason.
    Fail { reason: String },
    /// Check is currently running.
    Running,
    /// Check has not been run yet.
    Pending,
}

impl CheckStatus {
    /// Returns `true` if the status is `Pass`.
    #[must_use]
    pub fn is_pass(&self) -> bool {
        matches!(self, Self::Pass)
    }

    /// Human-readable label for the status.
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Pass => "通过",
            Self::Fail { .. } => "失败",
            Self::Running => "检测中...",
            Self::Pending => "等待中",
        }
    }
}

// ── CpuInfo ─────────────────────────────────────────────────────────────────

/// CPU detection result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    /// CPU architecture, e.g. "x86_64" / "aarch64"
    pub architecture: String,
    /// Number of logical cores
    pub cores: u32,
    /// Check status
    pub status: CheckStatus,
}

// ── GpuInfo ─────────────────────────────────────────────────────────────────

/// GPU detection result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    /// Whether a GPU is available
    pub available: bool,
    /// GPU name, if detected
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Check status
    pub status: CheckStatus,
}

// ── MemoryInfo ──────────────────────────────────────────────────────────────

/// System memory (RAM) detection result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    /// Total RAM in MB
    pub total_mb: u64,
    /// Available (free) RAM in MB
    pub available_mb: u64,
    /// Check status — memory info is always Pass (info only)
    pub status: CheckStatus,
}

// ── EmbeddedModelStatus ─────────────────────────────────────────────────────

/// Embedded model download status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedModelStatus {
    /// Whether the model has been fully downloaded
    pub downloaded: bool,
    /// Download progress (0.0 - 1.0), `None` if not downloading
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
    /// Model size in MB
    pub size_mb: u64,
    /// Check status
    pub status: CheckStatus,
}

// ── SystemReport ────────────────────────────────────────────────────────────

/// Full system check report.
///
/// Returned by `run_full_check()` and consumed by the frontend
/// Step 2 (SystemCheck) to display the check results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemReport {
    /// CPU detection result
    pub cpu: CpuInfo,
    /// GPU detection result
    pub gpu: GpuInfo,
    /// System memory info
    pub memory: MemoryInfo,
    /// Embedded model status
    pub embedded_model: EmbeddedModelStatus,
    /// Overall status — Pass only if all critical checks pass
    pub overall: CheckStatus,
}

impl SystemReport {
    /// Returns `true` if all critical checks have passed.
    ///
    /// Note: Memory info is always Pass (info only), so it doesn't affect
    /// the overall pass status. Only embedded_model.download matters.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.embedded_model.downloaded
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_status_is_pass() {
        assert!(CheckStatus::Pass.is_pass());
        assert!(!CheckStatus::Fail {
            reason: "reason".to_string()
        }
        .is_pass());
        assert!(!CheckStatus::Running.is_pass());
        assert!(!CheckStatus::Pending.is_pass());
    }

    #[test]
    fn test_check_status_label() {
        assert_eq!(CheckStatus::Pass.label(), "通过");
        assert_eq!(CheckStatus::Running.label(), "检测中...");
        assert_eq!(CheckStatus::Pending.label(), "等待中");
        assert_eq!(
            CheckStatus::Fail {
                reason: "err".to_string()
            }
            .label(),
            "失败"
        );
    }

    #[test]
    fn test_system_report_all_passed() {
        let report = SystemReport {
            cpu: CpuInfo {
                architecture: "x86_64".to_string(),
                cores: 8,
                status: CheckStatus::Pass,
            },
            gpu: GpuInfo {
                available: false,
                name: None,
                status: CheckStatus::Pass,
            },
            memory: MemoryInfo {
                total_mb: 8192,
                available_mb: 4096,
                status: CheckStatus::Pass,
            },
            embedded_model: EmbeddedModelStatus {
                downloaded: true,
                progress: Some(1.0),
                size_mb: 100,
                status: CheckStatus::Pass,
            },
            overall: CheckStatus::Pass,
        };

        // Node.js missing doesn't block the overall pass
        assert!(report.all_passed());
    }

    #[test]
    fn test_system_report_cpu_fails() {
        let report = SystemReport {
            cpu: CpuInfo {
                architecture: "unknown".to_string(),
                cores: 0,
                status: CheckStatus::Fail {
                    reason: "无法检测 CPU".to_string(),
                },
            },
            gpu: GpuInfo {
                available: false,
                name: None,
                status: CheckStatus::Pass,
            },
            memory: MemoryInfo {
                total_mb: 8192,
                available_mb: 4096,
                status: CheckStatus::Pass,
            },
            embedded_model: EmbeddedModelStatus {
                downloaded: true,
                progress: Some(1.0),
                size_mb: 100,
                status: CheckStatus::Pass,
            },
            overall: CheckStatus::Fail {
                reason: "CPU 检测失败".to_string(),
            },
        };

        // all_passed() only checks model download (CPU/GPU/Memory are info-only)
        assert!(report.all_passed());
    }

    #[test]
    fn test_check_status_serialization() {
        let status = CheckStatus::Pass;
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("pass"));

        let status = CheckStatus::Fail {
            reason: "error".to_string(),
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("fail"));
        assert!(json.contains("error"));
    }
}
