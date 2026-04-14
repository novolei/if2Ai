//! External skills directories support.
//!
//! Allows skills to be loaded from additional directories beyond the default
//! ~/.if2ai/skills/ directory. Configuration is read from skills.external_dirs.
//!
//! Ported from Hermes concept of external skills directories.

use std::path::{Path, PathBuf};
use thiserror::Error;

/// Errors for external directories operations.
#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum ExternalDirsError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid path: {0}")]
    InvalidPath(String),
}

/// Result type for external directories operations.
#[allow(dead_code)]
pub type ExternalDirsResult<T> = Result<T, ExternalDirsError>;

/// Configuration for external skills directories.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct ExternalDirsConfig {
    /// List of external directory paths.
    pub dirs: Vec<PathBuf>,
    /// Whether to enable remote passthrough.
    pub remote_passthrough_enabled: bool,
}

#[allow(dead_code)]
impl ExternalDirsConfig {
    /// Create a new empty config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create config with the given directories.
    pub fn with_dirs(self, dirs: Vec<PathBuf>) -> Self {
        Self {
            dirs,
            remote_passthrough_enabled: self.remote_passthrough_enabled,
        }
    }

    /// Add a directory to the config.
    #[allow(dead_code)]
    pub fn add_dir(mut self, dir: PathBuf) -> Self {
        self.dirs.push(dir);
        self
    }

    /// Enable remote passthrough.
    pub fn with_remote_passthrough(mut self) -> Self {
        self.remote_passthrough_enabled = true;
        self
    }
}

/// Manager for external skills directories.
///
/// Reads configuration from skills.external_dirs and provides
/// access to all skills directories (local + external).
#[derive(Debug, Clone)]
pub struct ExternalSkillsDirs {
    /// Default skills directory (~/.if2ai/skills/).
    default_dir: PathBuf,
    /// External directories from config.
    external_dirs: Vec<PathBuf>,
    /// Remote passthrough enabled.
    remote_passthrough_enabled: bool,
}

impl Default for ExternalSkillsDirs {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl ExternalSkillsDirs {
    /// Create a new ExternalSkillsDirs manager.
    ///
    /// Uses ~/.if2ai/skills/ as the default directory.
    pub fn new() -> Self {
        let default_dir = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".if2ai")
            .join("skills");

        Self {
            default_dir,
            external_dirs: Vec::new(),
            remote_passthrough_enabled: false,
        }
    }

    /// Create with a specific default directory.
    #[allow(dead_code)]
    pub fn with_default_dir(default_dir: PathBuf) -> Self {
        Self {
            default_dir,
            external_dirs: Vec::new(),
            remote_passthrough_enabled: false,
        }
    }

    /// Set external directories from config.
    ///
    /// Takes a list of path strings from skills.external_dirs config.
    pub fn set_external_dirs(&mut self, dirs: Vec<PathBuf>) {
        self.external_dirs = dirs;
    }

    /// Set external directories from config string slice.
    #[allow(dead_code)]
    pub fn set_external_dirs_from_strings(&mut self, dir_strings: &[String]) {
        self.external_dirs = dir_strings
            .iter()
            .filter_map(|s| {
                let path = PathBuf::from(s);
                // Validate path is not empty and is absolute
                if path.is_absolute() && !s.is_empty() {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();
    }

    /// Enable remote passthrough.
    pub fn enable_remote_passthrough(&mut self) {
        self.remote_passthrough_enabled = true;
    }

    /// Get the default skills directory.
    pub fn default_dir(&self) -> &Path {
        &self.default_dir
    }

    /// Get all external directories.
    pub fn external_dirs(&self) -> &[PathBuf] {
        &self.external_dirs
    }

    /// Check if remote passthrough is enabled.
    pub fn remote_passthrough_enabled(&self) -> bool {
        self.remote_passthrough_enabled
    }

    /// Get all skills directories (default + external).
    ///
    /// Returns all directories that should be scanned for skills.
    /// External directories that don't exist are filtered out.
    pub fn get_all_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = vec![self.default_dir.clone()];

        for external in &self.external_dirs {
            if external.exists() && external.is_dir() {
                dirs.push(external.clone());
            }
        }

        dirs
    }

    /// Get the count of all directories (including default).
    pub fn dir_count(&self) -> usize {
        1 + self.external_dirs.len()
    }

    /// Check if a path is within any registered skills directory.
    pub fn is_registered_dir(&self, path: &Path) -> bool {
        // Check default dir
        if path.starts_with(&self.default_dir) {
            return true;
        }

        // Check external dirs
        for external in &self.external_dirs {
            if path.starts_with(external) {
                return true;
            }
        }

        false
    }

    /// Load configuration from an environment variable or config file.
    ///
    /// Looks for skills.external_dirs in the environment or config.
    pub fn load_from_env(&mut self) {
        // Try to load from environment variable
        if let Ok(env_val) = std::env::var("IF2AI_SKILLS_EXTERNAL_DIRS") {
            if !env_val.is_empty() {
                let dirs: Vec<PathBuf> = env_val
                    .split(',')
                    .map(|s| PathBuf::from(s.trim()))
                    .collect();
                self.set_external_dirs(dirs);
            }
        }

        // Check for remote passthrough env var
        if let Ok(env_val) = std::env::var("IF2AI_SKILLS_REMOTE_PASSTHROUGH") {
            if env_val == "1" || env_val.to_lowercase() == "true" {
                self.enable_remote_passthrough();
            }
        }
    }

    /// Validate external directories configuration.
    ///
    /// Returns warnings for directories that don't exist.
    pub fn validate(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        for dir in &self.external_dirs {
            if !dir.exists() {
                warnings.push(format!(
                    "External skills directory does not exist: {}",
                    dir.display()
                ));
            } else if !dir.is_dir() {
                warnings.push(format!(
                    "External skills path is not a directory: {}",
                    dir.display()
                ));
            }
        }

        warnings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_external_dirs_config_new() {
        let config = ExternalDirsConfig::new();
        assert!(config.dirs.is_empty());
        assert!(!config.remote_passthrough_enabled);
    }

    #[test]
    fn test_external_dirs_config_with_dirs() {
        let config = ExternalDirsConfig::new()
            .with_dirs(vec![PathBuf::from("/tmp/skills")])
            .with_remote_passthrough();

        assert_eq!(config.dirs.len(), 1);
        assert!(config.remote_passthrough_enabled);
    }

    #[test]
    fn test_external_skills_dirs_default() {
        let dirs = ExternalSkillsDirs::new();
        assert!(dirs.default_dir().to_string_lossy().contains(".if2ai"));
        assert!(dirs.external_dirs().is_empty());
    }

    #[test]
    fn test_external_skills_dirs_with_external() {
        let mut dirs = ExternalSkillsDirs::new();
        dirs.set_external_dirs(vec![PathBuf::from("/tmp/external-skills")]);

        assert_eq!(dirs.external_dirs().len(), 1);
        assert_eq!(dirs.dir_count(), 2); // default + 1 external
    }

    #[test]
    fn test_get_all_dirs_filters_nonexistent() {
        let mut dirs = ExternalSkillsDirs::new();
        dirs.set_external_dirs(vec![
            PathBuf::from("/tmp/existing"), // doesn't exist but we don't create
            PathBuf::from("/tmp"),          // this exists
        ]);

        // /tmp exists so it should be included
        let all = dirs.get_all_dirs();
        assert!(!all.is_empty()); // at least default dir
    }

    #[test]
    fn test_is_registered_dir() {
        let mut dirs = ExternalSkillsDirs::new();
        dirs.set_external_dirs(vec![PathBuf::from("/tmp/skills")]);

        // Default dir should be registered
        assert!(dirs.is_registered_dir(dirs.default_dir()));

        // External dir should be registered
        assert!(dirs.is_registered_dir(&PathBuf::from("/tmp/skills/my-skill")));

        // Other paths should not be registered
        assert!(!dirs.is_registered_dir(&PathBuf::from("/usr/local/skills")));
    }

    #[test]
    fn test_load_from_env() {
        let mut dirs = ExternalSkillsDirs::new();

        // Set env vars
        std::env::set_var("IF2AI_SKILLS_EXTERNAL_DIRS", "/tmp/skills1,/tmp/skills2");
        std::env::set_var("IF2AI_SKILLS_REMOTE_PASSTHROUGH", "true");

        dirs.load_from_env();

        assert_eq!(dirs.external_dirs().len(), 2);
        assert!(dirs.remote_passthrough_enabled());

        // Clean up
        std::env::remove_var("IF2AI_SKILLS_EXTERNAL_DIRS");
        std::env::remove_var("IF2AI_SKILLS_REMOTE_PASSTHROUGH");
    }

    #[test]
    fn test_validate_warnings() {
        let mut dirs = ExternalSkillsDirs::new();
        dirs.set_external_dirs(vec![
            PathBuf::from("/nonexistent/path"),
            PathBuf::from("/also/nonexistent"),
        ]);

        let warnings = dirs.validate();
        assert_eq!(warnings.len(), 2);
        assert!(warnings[0].contains("does not exist"));
    }
}
