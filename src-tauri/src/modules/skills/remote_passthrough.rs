//! Remote backend environment passthrough support.
//!
//! Provides environment variable passthrough for remote execution backends
//! such as Docker, Singularity, Modal, SSH, and Daytona.
//!
//! Ported from Hermes concept of remote backend env passthrough.

use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

/// Errors for remote passthrough operations.
#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum RemotePassthroughError {
    #[error("invalid backend: {0}")]
    InvalidBackend(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("configuration error: {0}")]
    Config(String),
}

/// Result type for remote passthrough operations.
pub type RemotePassthroughResult<T> = Result<T, RemotePassthroughError>;

/// Supported remote execution backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteBackend {
    /// Docker container runtime.
    Docker,
    /// Singularity container runtime.
    Singularity,
    /// Modal serverless platform.
    Modal,
    /// SSH remote execution.
    Ssh,
    /// Daytona remote development.
    Daytona,
}

#[allow(dead_code)]
impl RemoteBackend {
    /// Parse a backend from string.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "docker" | "docker-runtime" => Some(Self::Docker),
            "singularity" | "singularity-runtime" => Some(Self::Singularity),
            "modal" | "modal-runtime" => Some(Self::Modal),
            "ssh" | "ssh-runtime" => Some(Self::Ssh),
            "daytona" | "daytona-runtime" => Some(Self::Daytona),
            _ => None,
        }
    }

    /// Get the backend name as string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Docker => "docker",
            Self::Singularity => "singularity",
            Self::Modal => "modal",
            Self::Ssh => "ssh",
            Self::Daytona => "daytona",
        }
    }

    /// Get the required credential environment variables for this backend.
    pub fn required_credentials(&self) -> Vec<&'static str> {
        match self {
            Self::Docker => vec![],
            Self::Singularity => vec![],
            Self::Modal => vec!["MODAL_API_TOKEN", "MODAL_ENDPOINT"],
            Self::Ssh => vec!["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"],
            Self::Daytona => vec!["DAYTONA_API_KEY", "DAYTONA_ENDPOINT"],
        }
    }

    /// Get the mount point environment variables.
    #[allow(dead_code)]
    pub fn mount_variables(&self) -> Vec<&'static str> {
        match self {
            Self::Docker => vec!["DOCKER_MOUNT_VOLUME"],
            Self::Singularity => vec!["SINGULARITY_MOUNT"],
            Self::Modal => vec!["MODAL_VOLUME"],
            Self::Ssh => vec!["SSHFS_MOUNT"],
            Self::Daytona => vec!["DAYTONA_VOLUME"],
        }
    }
}

impl std::fmt::Display for RemoteBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Environment variable passthrough configuration for a remote backend.
#[derive(Debug, Clone)]
pub struct PassthroughConfig {
    /// The backend type.
    pub backend: RemoteBackend,
    /// Environment variables to pass through.
    pub env_vars: Vec<String>,
    /// Required credential files to mount.
    pub credential_files: Vec<PathBuf>,
    /// Additional mount paths.
    pub mount_paths: Vec<PathBuf>,
    /// Working directory inside the remote.
    pub working_dir: Option<PathBuf>,
}

#[allow(dead_code)]
impl PassthroughConfig {
    /// Create a new passthrough config for a backend.
    pub fn new(backend: RemoteBackend) -> Self {
        Self {
            backend,
            env_vars: Vec::new(),
            credential_files: Vec::new(),
            mount_paths: Vec::new(),
            working_dir: None,
        }
    }

    /// Add an environment variable to pass through.
    pub fn add_env_var(mut self, var: impl Into<String>) -> Self {
        self.env_vars.push(var.into());
        self
    }

    /// Add multiple environment variables.
    #[allow(dead_code)]
    pub fn add_env_vars(mut self, vars: impl IntoIterator<Item = String>) -> Self {
        self.env_vars.extend(vars);
        self
    }

    /// Add a credential file to mount.
    pub fn add_credential_file(mut self, path: PathBuf) -> Self {
        self.credential_files.push(path);
        self
    }

    /// Add a mount path.
    pub fn add_mount_path(mut self, path: PathBuf) -> Self {
        self.mount_paths.push(path);
        self
    }

    /// Set the working directory inside the remote.
    pub fn set_working_dir(mut self, path: PathBuf) -> Self {
        self.working_dir = Some(path);
        self
    }

    /// Build the environment for the remote backend.
    ///
    /// Returns a HashMap of environment variables that should be set
    /// in the remote execution context.
    pub fn build_env(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();

        // Add passthrough variables
        for var in &self.env_vars {
            if let Ok(value) = std::env::var(var) {
                env.insert(var.clone(), value);
            }
        }

        // Add credential file paths
        for (idx, cred_file) in self.credential_files.iter().enumerate() {
            env.insert(
                format!("{}_CRED_{}", self.backend.as_str().to_uppercase(), idx),
                cred_file.to_string_lossy().to_string(),
            );
        }

        // Add mount paths
        for (idx, mount) in self.mount_paths.iter().enumerate() {
            env.insert(
                format!("{}_MOUNT_{}", self.backend.as_str().to_uppercase(), idx),
                mount.to_string_lossy().to_string(),
            );
        }

        // Add working directory
        if let Some(ref workdir) = self.working_dir {
            env.insert(
                format!("{}_WORKDIR", self.backend.as_str().to_uppercase()),
                workdir.to_string_lossy().to_string(),
            );
        }

        env
    }
}

/// Remote passthrough manager.
///
/// Manages environment variable passthrough for remote execution backends.
#[derive(Debug, Clone)]
pub struct RemotePassthroughManager {
    /// Configured backends.
    backends: Vec<PassthroughConfig>,
    /// Global passthrough environment variables.
    global_env_vars: Vec<String>,
}

impl Default for RemotePassthroughManager {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl RemotePassthroughManager {
    /// Create a new RemotePassthroughManager.
    pub fn new() -> Self {
        Self {
            backends: Vec::new(),
            global_env_vars: Vec::new(),
        }
    }

    /// Add a backend configuration.
    pub fn add_backend(&mut self, config: PassthroughConfig) {
        self.backends.push(config);
    }

    /// Add a backend from a string representation.
    pub fn add_backend_from_str(&mut self, backend_str: &str) -> RemotePassthroughResult<()> {
        let backend = RemoteBackend::from_str(backend_str)
            .ok_or_else(|| RemotePassthroughError::InvalidBackend(backend_str.to_string()))?;

        self.backends.push(PassthroughConfig::new(backend));
        Ok(())
    }

    /// Add a global environment variable to pass through to all backends.
    pub fn add_global_env_var(&mut self, var: impl Into<String>) {
        self.global_env_vars.push(var.into());
    }

    /// Set global environment variables from a list.
    pub fn set_global_env_vars(&mut self, vars: Vec<String>) {
        self.global_env_vars = vars;
    }

    /// Get all configured backends.
    pub fn backends(&self) -> &[PassthroughConfig] {
        &self.backends
    }

    /// Get global environment variables.
    pub fn global_env_vars(&self) -> &[String] {
        &self.global_env_vars
    }

    /// Get passthrough config for a specific backend.
    #[allow(dead_code)]
    pub fn get_backend_config(&self, backend: RemoteBackend) -> Option<&PassthroughConfig> {
        self.backends.iter().find(|c| c.backend == backend)
    }

    /// Check if any backends are configured.
    pub fn is_enabled(&self) -> bool {
        !self.backends.is_empty()
    }

    /// Build combined environment for all configured backends.
    ///
    /// Returns environment variables that should be passed to the remote context.
    pub fn build_combined_env(&self) -> HashMap<String, String> {
        let mut combined = HashMap::new();

        // Add global env vars
        for var in &self.global_env_vars {
            if let Ok(value) = std::env::var(var) {
                combined.insert(var.clone(), value);
            }
        }

        // Add backend-specific env vars
        for backend in &self.backends {
            for (key, value) in backend.build_env() {
                combined.insert(key, value);
            }
        }

        combined
    }

    /// Load configuration from environment variables.
    ///
    /// Looks for:
    /// - IF2AI_REMOTE_BACKENDS: comma-separated list of backends
    /// - IF2AI_REMOTE_PASSTHROUGH_ENVS: comma-separated env vars to passthrough
    pub fn load_from_env(&mut self) {
        // Load backends
        if let Ok(backends_str) = std::env::var("IF2AI_REMOTE_BACKENDS") {
            for backend_str in backends_str.split(',') {
                let trimmed = backend_str.trim();
                if !trimmed.is_empty() {
                    if let Err(e) = self.add_backend_from_str(trimmed) {
                        eprintln!("Invalid backend in IF2AI_REMOTE_BACKENDS: {}", e);
                    }
                }
            }
        }

        // Load global passthrough env vars
        if let Ok(envs_str) = std::env::var("IF2AI_REMOTE_PASSTHROUGH_ENVS") {
            let vars: Vec<String> = envs_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            self.set_global_env_vars(vars);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_backend_from_str() {
        assert_eq!(
            RemoteBackend::from_str("docker"),
            Some(RemoteBackend::Docker)
        );
        assert_eq!(
            RemoteBackend::from_str("DOCKER"),
            Some(RemoteBackend::Docker)
        );
        assert_eq!(
            RemoteBackend::from_str("singularity"),
            Some(RemoteBackend::Singularity)
        );
        assert_eq!(RemoteBackend::from_str("modal"), Some(RemoteBackend::Modal));
        assert_eq!(RemoteBackend::from_str("ssh"), Some(RemoteBackend::Ssh));
        assert_eq!(
            RemoteBackend::from_str("daytona"),
            Some(RemoteBackend::Daytona)
        );
        assert_eq!(RemoteBackend::from_str("unknown"), None);
    }

    #[test]
    fn test_remote_backend_required_credentials() {
        assert!(RemoteBackend::Docker.required_credentials().is_empty());
        assert!(!RemoteBackend::Modal.required_credentials().is_empty());
        assert!(!RemoteBackend::Ssh.required_credentials().is_empty());
    }

    #[test]
    fn test_passthrough_config_build_env() {
        let config = PassthroughConfig::new(RemoteBackend::Docker)
            .add_env_var("PATH")
            .add_env_var("HOME")
            .add_credential_file(PathBuf::from("/tmp/cred"))
            .add_mount_path(PathBuf::from("/mnt/data"))
            .set_working_dir(PathBuf::from("/workspace"));

        let env = config.build_env();
        assert!(env.contains_key("PATH"));
        assert!(env.contains_key("HOME"));
        assert!(env.contains_key("DOCKER_CRED_0"));
        assert!(env.contains_key("DOCKER_MOUNT_0"));
        assert!(env.contains_key("DOCKER_WORKDIR"));
    }

    #[test]
    fn test_remote_passthrough_manager_new() {
        let manager = RemotePassthroughManager::new();
        assert!(!manager.is_enabled());
        assert!(manager.backends().is_empty());
    }

    #[test]
    fn test_remote_passthrough_manager_add_backend() {
        let mut manager = RemotePassthroughManager::new();
        manager.add_backend(PassthroughConfig::new(RemoteBackend::Docker));

        assert!(manager.is_enabled());
        assert_eq!(manager.backends().len(), 1);
    }

    #[test]
    fn test_remote_passthrough_manager_global_env_vars() {
        let mut manager = RemotePassthroughManager::new();
        manager.add_global_env_var("PATH");
        manager.add_global_env_var("HOME");

        assert_eq!(manager.global_env_vars().len(), 2);
    }

    #[test]
    fn test_load_from_env() {
        let mut manager = RemotePassthroughManager::new();

        std::env::set_var("IF2AI_REMOTE_BACKENDS", "docker,modal");
        std::env::set_var("IF2AI_REMOTE_PASSTHROUGH_ENVS", "PATH,HOME,API_KEY");

        manager.load_from_env();

        assert_eq!(manager.backends().len(), 2);
        assert_eq!(manager.global_env_vars().len(), 3);

        std::env::remove_var("IF2AI_REMOTE_BACKENDS");
        std::env::remove_var("IF2AI_REMOTE_PASSTHROUGH_ENVS");
    }

    #[test]
    fn test_build_combined_env() {
        let mut manager = RemotePassthroughManager::new();
        manager.add_global_env_var("GLOBAL_VAR");
        manager.add_backend(PassthroughConfig::new(RemoteBackend::Modal).add_env_var("MODAL_VAR"));

        std::env::set_var("GLOBAL_VAR", "global_value");
        std::env::set_var("MODAL_VAR", "modal_value");

        let combined = manager.build_combined_env();
        assert_eq!(
            combined.get("GLOBAL_VAR"),
            Some(&"global_value".to_string())
        );
        assert_eq!(combined.get("MODAL_VAR"), Some(&"modal_value".to_string()));

        std::env::remove_var("GLOBAL_VAR");
        std::env::remove_var("MODAL_VAR");
    }
}
