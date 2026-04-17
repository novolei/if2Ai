//! Chrome/Chromium binary discovery.
//!
//! Searches well-known installation paths on macOS, Linux, and Windows
//! and returns the first valid executable found.

use std::path::PathBuf;

/// Candidate paths to probe per platform.
#[cfg(target_os = "macos")]
fn candidates() -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
        PathBuf::from("/Applications/Chromium.app/Contents/MacOS/Chromium"),
        PathBuf::from("/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary"),
        PathBuf::from("/Applications/Brave Browser.app/Contents/MacOS/Brave Browser"),
    ];
    // User-level install: only add when home dir is available, avoiding a
    // misleading empty PathBuf in the candidates list.
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join("Applications/Google Chrome.app/Contents/MacOS/Google Chrome"));
    }
    paths
}

#[cfg(target_os = "linux")]
fn candidates() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/usr/bin/google-chrome"),
        PathBuf::from("/usr/bin/google-chrome-stable"),
        PathBuf::from("/usr/bin/chromium"),
        PathBuf::from("/usr/bin/chromium-browser"),
        PathBuf::from("/usr/local/bin/chromium"),
        PathBuf::from("/snap/bin/chromium"),
    ]
}

#[cfg(target_os = "windows")]
fn candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // LOCALAPPDATA
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        paths.push(PathBuf::from(&local).join("Google/Chrome/Application/chrome.exe"));
    }
    // PROGRAMFILES
    if let Ok(pf) = std::env::var("PROGRAMFILES") {
        paths.push(PathBuf::from(&pf).join("Google/Chrome/Application/chrome.exe"));
        paths.push(PathBuf::from(&pf).join("Chromium/Application/chrome.exe"));
    }
    // PROGRAMFILES(X86)
    if let Ok(pf86) = std::env::var("PROGRAMFILES(X86)") {
        paths.push(PathBuf::from(&pf86).join("Google/Chrome/Application/chrome.exe"));
    }
    paths
}

/// Result of a Chrome binary discovery attempt.
#[derive(Debug, Clone)]
pub struct ChromeStatus {
    /// Whether a Chrome binary was found.
    pub found: bool,
    /// Absolute path to the binary, if found.
    pub path: Option<PathBuf>,
}

/// Search well-known locations for a Chrome or Chromium binary.
///
/// Returns the first candidate path that exists as a file.
/// Returns [`ChromeStatus::found`] `false` when no binary is detected.
#[must_use]
pub fn find_chrome_binary() -> ChromeStatus {
    for candidate in candidates() {
        if candidate.is_file() {
            return ChromeStatus {
                found: true,
                path: Some(candidate),
            };
        }
    }
    ChromeStatus {
        found: false,
        path: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_status_has_correct_shape_when_not_found() {
        // We cannot guarantee Chrome is installed in CI, so just verify the
        // function returns a coherent ChromeStatus without panicking.
        let status = find_chrome_binary();
        if status.found {
            assert!(status.path.is_some());
        } else {
            assert!(status.path.is_none());
        }
    }
}
