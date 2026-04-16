//! System environment detection functions.
//!
//! Implements hardware and runtime detection for the onboarding system check:
//! - `detect_cpu()`: CPU architecture and core count via sysinfo crate
//! - `detect_gpu()`: GPU availability and name via platform-specific commands
//! - `detect_nodejs()`: Node.js installation via `node --version`
//! - `run_full_check()`: Orchestrates all checks into a `SystemReport`

use std::process::Stdio;
use tokio::process::Command;

use super::types::{CheckStatus, CpuInfo, EmbeddedModelStatus, GpuInfo, NodeJsInfo, SystemReport};

// ── CPU Detection ───────────────────────────────────────────────────────────

/// Detect CPU architecture and core count.
///
/// Uses `sysinfo::System` for cross-platform detection.
/// Falls back to `std::env::consts::ARCH` if sysinfo fails.
pub fn detect_cpu() -> CpuInfo {
    let mut sys = sysinfo::System::new();
    sys.refresh_cpu_all();

    let architecture = std::env::consts::ARCH.to_string();
    let cores = sys.cpus().len() as u32;

    if cores == 0 {
        CpuInfo {
            architecture,
            cores: 0,
            status: CheckStatus::Fail {
                reason: "无法检测 CPU".to_string(),
            },
        }
    } else {
        CpuInfo {
            architecture,
            cores,
            status: CheckStatus::Pass,
        }
    }
}

// ── GPU Detection ───────────────────────────────────────────────────────────

/// Detect GPU availability and name.
///
/// Platform-specific:
/// - macOS: `system_profiler SPDisplaysDataType -json`
/// - Linux: `lspci | grep -i vga`
/// - Other / failure: returns unavailable
pub async fn detect_gpu() -> GpuInfo {
    let result = detect_gpu_platform_specific().await;
    match result {
        Some(name) => GpuInfo {
            available: true,
            name: Some(name),
            status: CheckStatus::Pass,
        },
        None => GpuInfo {
            available: false,
            name: None,
            status: CheckStatus::Fail {
                reason: "未检测到 GPU".to_string(),
            },
        },
    }
}

#[cfg(target_os = "macos")]
async fn detect_gpu_platform_specific() -> Option<String> {
    let output = Command::new("system_profiler")
        .args(["SPDisplaysDataType", "-json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse JSON for GPU model name
    parse_macos_gpu_json(&stdout)
}

#[cfg(target_os = "macos")]
fn parse_macos_gpu_json(json: &str) -> Option<String> {
    // Simple parsing: look for "sppci_model" or "_name" field
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let items = value.get("SPDisplaysDataType")?.as_array()?;

    for item in items {
        if let Some(name) = item.get("sppci_model").and_then(|v| v.as_str()) {
            return Some(name.to_string());
        }
        if let Some(name) = item.get("_name").and_then(|v| v.as_str()) {
            return Some(name.to_string());
        }
    }

    // Fallback: check top-level "_name"
    value
        .get("_name")
        .and_then(|v| v.as_str())
        .map(String::from)
}

#[cfg(target_os = "linux")]
async fn detect_gpu_platform_specific() -> Option<String> {
    let output = Command::new("sh")
        .arg("-c")
        .arg("lspci 2>/dev/null | grep -i vga | head -1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        None
    } else {
        // lspci output format: "01:00.0 VGA compatible controller: NVIDIA ..."
        // Extract everything after the colon
        trimmed.splitn(2, ':').nth(1).map(|s| s.trim().to_string())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
async fn detect_gpu_platform_specific() -> Option<String> {
    // Unsupported platform — return None (no GPU detected)
    None
}

// ── Node.js Detection ───────────────────────────────────────────────────────

/// Detect Node.js installation and version.
///
/// Runs `node --version` and parses the output (e.g. "v18.17.0").
/// Node.js missing is a **non-blocking warning**, not a failure.
pub async fn detect_nodejs() -> NodeJsInfo {
    let output = Command::new("node")
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await;

    match output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let version = stdout.trim().to_string();
            // Strip leading 'v' if present
            let clean_version = version.strip_prefix('v').unwrap_or(&version).to_string();

            NodeJsInfo {
                installed: true,
                version: Some(clean_version),
                status: CheckStatus::Pass,
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            NodeJsInfo {
                installed: false,
                version: None,
                status: CheckStatus::Fail {
                    reason: format!("Node.js 不可用: {}", stderr.trim()),
                },
            }
        }
        Err(e) => NodeJsInfo {
            installed: false,
            version: None,
            status: CheckStatus::Fail {
                reason: format!("未检测到 Node.js: {}", e),
            },
        },
    }
}

// ── Full Check ──────────────────────────────────────────────────────────────

/// Run all system checks and produce a full report.
///
/// Executes CPU, GPU, Node.js, and embedded model checks in parallel where
/// possible. The embedded model check is synchronous (file existence only).
pub async fn run_full_check() -> SystemReport {
    // CPU is synchronous (fast)
    let cpu = detect_cpu();

    // GPU and Node.js are async (subprocess calls)
    let (gpu, nodejs) = tokio::join!(detect_gpu(), detect_nodejs());

    // Embedded model check is synchronous
    let model_exists = super::model_download::embedded_model_exists();
    let model_progress = super::model_download::get_download_progress();

    let embedded_model = if model_exists {
        EmbeddedModelStatus {
            downloaded: true,
            progress: Some(1.0),
            size_mb: 0, // Will be populated during actual download
            status: CheckStatus::Pass,
        }
    } else if model_progress > 0.0 {
        EmbeddedModelStatus {
            downloaded: false,
            progress: Some(model_progress),
            size_mb: 0,
            status: CheckStatus::Fail {
                reason: "模型未下载完成".to_string(),
            },
        }
    } else {
        EmbeddedModelStatus {
            downloaded: false,
            progress: None,
            size_mb: 0,
            status: CheckStatus::Fail {
                reason: "模型未下载".to_string(),
            },
        }
    };

    // Overall: CPU + GPU must pass. Node.js is non-blocking.
    // Embedded model: Pass if downloaded, Fail otherwise (but not blocking).
    let overall = if cpu.status.is_pass() && gpu.status.is_pass() {
        CheckStatus::Pass
    } else {
        let mut reasons = Vec::new();
        if !cpu.status.is_pass() {
            reasons.push("CPU");
        }
        if !gpu.status.is_pass() {
            reasons.push("GPU");
        }
        CheckStatus::Fail {
            reason: format!("{} 检测失败", reasons.join(" + ")),
        }
    };

    SystemReport {
        cpu,
        gpu,
        nodejs,
        embedded_model,
        overall,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_cpu_returns_pass() {
        let cpu = detect_cpu();
        assert!(cpu.status.is_pass(), "CPU check should pass: {:?}", cpu);
        assert!(cpu.cores > 0, "CPU cores should be > 0");
        assert!(
            !cpu.architecture.is_empty(),
            "CPU architecture should not be empty"
        );
    }

    #[test]
    fn test_embedded_model_dir_contains_if2ai() {
        let dir = crate::modules::system_check::model_download::embedded_model_dir();
        let path = dir.to_string_lossy();
        assert!(
            path.contains(".if2ai"),
            "Model dir should contain .if2ai, got: {}",
            path
        );
        assert!(
            path.contains("models/embedded-rs"),
            "Model dir should end with models/embedded-rs, got: {}",
            path
        );
    }

    #[test]
    fn test_get_download_progress_returns_zero_initially() {
        // Before any download, progress should be 0
        let progress = crate::modules::system_check::model_download::get_download_progress();
        assert!(
            (0.0..=1.0).contains(&progress),
            "Progress should be between 0 and 1, got: {}",
            progress
        );
    }
}
