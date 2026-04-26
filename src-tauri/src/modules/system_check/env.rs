//! System environment detection functions.
//!
//! Implements hardware and runtime detection for the onboarding system check:
//! - `detect_cpu()`: CPU architecture and core count via sysinfo crate
//! - `detect_gpu()`: GPU availability and name via platform-specific commands
//! - `detect_memory()`: System RAM info via sysinfo crate
//! - `run_full_check()`: Orchestrates all checks into a `SystemReport`

use std::process::Stdio;
use tokio::process::Command;

use super::model_download::{
    embedded_model_exists, get_download_progress, is_download_in_progress, MODEL_SIZE_MB,
};
use super::types::{CheckStatus, CpuInfo, EmbeddedModelStatus, GpuInfo, MemoryInfo, SystemReport};

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

// ── Memory Detection ────────────────────────────────────────────────────────

/// Detect system RAM (total and available).
///
/// Uses `sysinfo::System` for cross-platform detection.
/// Memory info is always Pass (info only, not blocking).
pub fn detect_memory() -> MemoryInfo {
    let mut sys = sysinfo::System::new_all();
    sys.refresh_memory();

    let total_mb = sys.total_memory() / (1024 * 1024);
    let available_mb = sys.available_memory() / (1024 * 1024);

    MemoryInfo {
        total_mb,
        available_mb,
        status: CheckStatus::Pass,
    }
}

// ── Full Check ──────────────────────────────────────────────────────────────

/// Run all system checks and produce a full report.
///
/// Executes CPU, GPU, and Memory checks synchronously (fast via sysinfo).
/// The embedded model check is synchronous (file existence only).
pub async fn run_full_check() -> SystemReport {
    // All hardware checks are synchronous (fast via sysinfo)
    let cpu = detect_cpu();
    let gpu = detect_gpu().await;
    let memory = detect_memory();

    // Embedded model check is synchronous
    let model_exists = embedded_model_exists();
    let model_progress = get_download_progress();
    let model_downloading = is_download_in_progress();

    let embedded_model = if model_exists {
        EmbeddedModelStatus {
            downloaded: true,
            progress: Some(1.0),
            size_mb: MODEL_SIZE_MB,
            status: CheckStatus::Pass,
        }
    } else if model_downloading {
        EmbeddedModelStatus {
            downloaded: false,
            progress: Some(model_progress),
            size_mb: MODEL_SIZE_MB,
            status: CheckStatus::Running,
        }
    } else {
        EmbeddedModelStatus {
            downloaded: false,
            progress: None,
            size_mb: MODEL_SIZE_MB,
            status: CheckStatus::Pending,
        }
    };

    // CPU and GPU are always Pass. Memory is always Pass (info only).
    // Overall reflects the actual state for display purposes.
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
        memory,
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
    fn test_get_download_progress_returns_zero_initially() {
        let progress = crate::modules::system_check::model_download::get_download_progress();
        assert!(
            (0.0..=1.0).contains(&progress),
            "Progress should be between 0 and 1, got: {}",
            progress
        );
    }
}
