//! Atomic writes — crash-safe file operations
//!
//! Writes data to a temporary file and atomically renames it,
//! ensuring that readers never see partial writes.

use serde::Serialize;
use std::path::Path;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;

/// Atomically write contents to a file
///
/// Writes to a `.tmp` file first, syncs to disk, then atomically renames.
/// This ensures that if the process is killed mid-write, either the old
/// file or the new file exists (never a corrupt partial state).
pub async fn atomic_write<P: AsRef<Path>, C: AsRef<[u8]>>(
    path: P,
    contents: C,
) -> Result<(), WriteError> {
    let path = path.as_ref();
    let temp_path = path.with_extension("tmp");

    // Write to temp file
    {
        let mut file = File::create(&temp_path)
            .await
            .map_err(|e| WriteError::Io(e.to_string()))?;

        file.write_all(contents.as_ref())
            .await
            .map_err(|e| WriteError::Io(e.to_string()))?;

        // Sync to disk before rename
        file.sync_all()
            .await
            .map_err(|e| WriteError::Io(e.to_string()))?;
    }

    // Atomic rename
    fs::rename(&temp_path, path)
        .await
        .map_err(|e| WriteError::Io(e.to_string()))?;

    Ok(())
}

/// Atomically write serializable data as pretty-printed JSON
pub async fn atomic_json_write<P: AsRef<Path>, T: Serialize>(
    path: P,
    data: &T,
) -> Result<(), WriteError> {
    let contents =
        serde_json::to_string_pretty(data).map_err(|e| WriteError::Serialization(e.to_string()))?;
    atomic_write(path, contents.as_bytes()).await
}

/// Error types for atomic writes
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("I/O error: {0}")]
    Io(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn atomic_write_and_read() {
        let temp_dir = std::env::temp_dir().join("if2ai_atomic_test");
        let _ = fs::remove_dir_all(&temp_dir).await;
        fs::create_dir_all(&temp_dir).await.unwrap();

        let path = temp_dir.join("test.txt");
        atomic_write(&path, b"Hello, atomic!").await.unwrap();

        let content = fs::read_to_string(&path).await.unwrap();
        assert_eq!(content, "Hello, atomic!");

        let _ = fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn atomic_json_write_roundtrip() {
        let temp_dir = std::env::temp_dir().join("if2ai_json_atomic_test");
        let _ = fs::remove_dir_all(&temp_dir).await;
        fs::create_dir_all(&temp_dir).await.unwrap();

        let path = temp_dir.join("data.json");
        let mut data = HashMap::new();
        data.insert("key".to_string(), "value".to_string());

        atomic_json_write(&path, &data).await.unwrap();

        let content = fs::read_to_string(&path).await.unwrap();
        let parsed: HashMap<String, String> = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["key"], "value");

        let _ = fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn overwrites_existing_file() {
        let temp_dir = std::env::temp_dir().join("if2ai_overwrite_test");
        let _ = fs::remove_dir_all(&temp_dir).await;
        fs::create_dir_all(&temp_dir).await.unwrap();

        let path = temp_dir.join("test.txt");
        atomic_write(&path, b"First content").await.unwrap();
        atomic_write(&path, b"Second content").await.unwrap();

        let content = fs::read_to_string(&path).await.unwrap();
        assert_eq!(content, "Second content");

        let _ = fs::remove_dir_all(&temp_dir).await;
    }
}
