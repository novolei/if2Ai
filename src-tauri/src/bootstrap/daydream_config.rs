//! Read/write `DayDreamConfig` at `<if2ai_dir>/daydream.json`.

use std::fs;
use std::path::Path;

use crate::modules::memory::daydream::DayDreamConfig;

const FILENAME: &str = "daydream.json";

/// Read the persisted config. Returns `DayDreamConfig::default()` (`enabled: false`)
/// if the file is missing or invalid — matches the opt-in safety contract.
pub fn load(if2ai_dir: &Path) -> DayDreamConfig {
    let path = if2ai_dir.join(FILENAME);
    let Ok(raw) = fs::read_to_string(&path) else {
        return DayDreamConfig::default();
    };
    serde_json::from_str(&raw).unwrap_or_else(|err| {
        tracing::warn!(
            path = %path.display(),
            error = %err,
            "[init] daydream.json invalid; falling back to defaults"
        );
        DayDreamConfig::default()
    })
}

/// Persist the config. Best-effort: returns `Err(...)` so the command
/// layer can surface a toast, but the in-memory engine still updates.
pub fn save(if2ai_dir: &Path, cfg: &DayDreamConfig) -> std::io::Result<()> {
    let path = if2ai_dir.join(FILENAME);
    let raw = serde_json::to_string_pretty(cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(path, raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn load_returns_default_when_file_missing() {
        let dir = tempdir().unwrap();
        let cfg = load(dir.path());
        assert_eq!(cfg, DayDreamConfig::default());
    }

    #[test]
    fn round_trip_preserves_fields() {
        let dir = tempdir().unwrap();
        let cfg = DayDreamConfig {
            enabled: true,
            idle_trigger_minutes: 45,
            max_entries_per_cycle: 50,
            strategy: crate::modules::memory::daydream::ConsolidationStrategy::Aggressive,
        };
        save(dir.path(), &cfg).unwrap();
        let read = load(dir.path());
        assert_eq!(read, cfg);
    }
}
