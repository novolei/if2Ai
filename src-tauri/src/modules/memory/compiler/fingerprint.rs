//! Fingerprint sidecar for compile-cache (Phase 8B.2 / T-C2).
//!
//! Each `compile_*` writes `<output>.md` plus a sibling
//! `<output>.md.fingerprint` containing the MD5 of the input keys
//! (typically `session_id:updated_at` joined by newline).  On the
//! next compile cycle, [`is_unchanged`] returns `true` when the
//! computed fingerprint equals the on-disk one AND the output file
//! exists, letting the compiler short-circuit to
//! [`crate::modules::memory::compiler::CompileResult::Skipped`]
//! without burning LLM budget.
//!
//! Mirrors openhanako `lib/memory/compile.js::computeFingerprint`.

#![allow(dead_code)] // first production consumers land in 8B.3 (compile_today/week/longterm) + 8B.4 (compile_facts/assemble)

use std::path::{Path, PathBuf};

use crate::modules::memory::MemoryError;

/// Sentinel returned by [`compute_fingerprint`] when no input keys are
/// supplied — distinguishable from a real digest so the empty-input
/// case still cleanly UPSERTs an empty `.md`.
pub const EMPTY_FINGERPRINT: &str = "empty";

/// Compute the MD5 hex digest of `keys` joined by `\n`.  Returns
/// [`EMPTY_FINGERPRINT`] on empty input.
#[must_use]
pub fn compute_fingerprint(keys: &[String]) -> String {
    if keys.is_empty() {
        return EMPTY_FINGERPRINT.to_string();
    }
    let digest = md5::compute(keys.join("\n").as_bytes());
    format!("{digest:x}")
}

/// Resolve the sidecar path: `<output_path>.md.fingerprint` when
/// `output_path` ends with `.md`, otherwise `<output_path>.fingerprint`.
#[must_use]
pub fn fingerprint_path(output_path: &Path) -> PathBuf {
    let mut p = output_path.to_path_buf().into_os_string();
    p.push(".fingerprint");
    PathBuf::from(p)
}

/// Read the fingerprint sidecar.  Returns `None` when the file does
/// not exist or is unreadable — both are normal "first run" states
/// and must NOT be reported as errors.
#[must_use]
pub fn read_fingerprint(output_path: &Path) -> Option<String> {
    let fp_path = fingerprint_path(output_path);
    std::fs::read_to_string(fp_path)
        .ok()
        .map(|s| s.trim().to_string())
}

/// Atomically write the fingerprint to `<output_path>.fingerprint`
/// (tmp + rename) so a crash mid-write cannot corrupt the cache.
pub fn write_fingerprint(output_path: &Path, fp: &str) -> Result<(), MemoryError> {
    let fp_path = fingerprint_path(output_path);
    if let Some(parent) = fp_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            MemoryError::Generic(format!(
                "fingerprint write: failed to create parent dir {parent:?}: {e}"
            ))
        })?;
    }
    let mut tmp_path = fp_path.clone().into_os_string();
    tmp_path.push(".tmp");
    let tmp_path = PathBuf::from(tmp_path);
    std::fs::write(&tmp_path, fp).map_err(|e| {
        MemoryError::Generic(format!("fingerprint write: tmp write to {tmp_path:?}: {e}"))
    })?;
    std::fs::rename(&tmp_path, &fp_path).map_err(|e| {
        MemoryError::Generic(format!(
            "fingerprint write: rename {tmp_path:?} -> {fp_path:?}: {e}"
        ))
    })?;
    Ok(())
}

/// `true` iff `output_path` exists AND its fingerprint sidecar matches
/// `current_fp`.  Either condition false → recompile required.
#[must_use]
pub fn is_unchanged(output_path: &Path, current_fp: &str) -> bool {
    if !output_path.exists() {
        return false;
    }
    matches!(read_fingerprint(output_path).as_deref(), Some(stored) if stored == current_fp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn empty_keys_returns_sentinel() {
        assert_eq!(compute_fingerprint(&[]), EMPTY_FINGERPRINT);
    }

    #[test]
    fn same_keys_same_fp() {
        let keys = vec![
            "sess-1:2024-01-01T00:00:00Z".to_string(),
            "sess-2:2024-01-01T01:00:00Z".to_string(),
        ];
        assert_eq!(compute_fingerprint(&keys), compute_fingerprint(&keys));
    }

    #[test]
    fn changed_key_changed_fp() {
        let keys_a = vec!["sess-1:t1".to_string()];
        let keys_b = vec!["sess-1:t2".to_string()];
        assert_ne!(compute_fingerprint(&keys_a), compute_fingerprint(&keys_b));
    }

    #[test]
    fn fingerprint_path_appends_dot_fingerprint() {
        let p = Path::new("/tmp/today.md");
        assert_eq!(
            fingerprint_path(p),
            PathBuf::from("/tmp/today.md.fingerprint")
        );
    }

    #[test]
    fn read_returns_none_when_missing() {
        let dir = tempdir().expect("tempdir");
        let out = dir.path().join("missing.md");
        assert_eq!(read_fingerprint(&out), None);
    }

    #[test]
    fn write_then_read_roundtrip() {
        let dir = tempdir().expect("tempdir");
        let out = dir.path().join("today.md");
        write_fingerprint(&out, "abc123").expect("write");
        assert_eq!(read_fingerprint(&out), Some("abc123".to_string()));
    }

    #[test]
    fn is_unchanged_false_when_output_missing() {
        let dir = tempdir().expect("tempdir");
        let out = dir.path().join("missing.md");
        write_fingerprint(&out, "fp").expect("write");
        // sidecar exists but output doesn't
        assert!(!is_unchanged(&out, "fp"));
    }

    #[test]
    fn is_unchanged_true_when_output_and_fp_match() {
        let dir = tempdir().expect("tempdir");
        let out = dir.path().join("today.md");
        std::fs::write(&out, "compiled body").expect("write output");
        write_fingerprint(&out, "fp123").expect("write fp");
        assert!(is_unchanged(&out, "fp123"));
    }

    #[test]
    fn is_unchanged_false_when_fp_differs() {
        let dir = tempdir().expect("tempdir");
        let out = dir.path().join("today.md");
        std::fs::write(&out, "body").expect("write output");
        write_fingerprint(&out, "fp_old").expect("write fp");
        assert!(!is_unchanged(&out, "fp_new"));
    }

    #[test]
    fn write_atomic_does_not_leave_tmp() {
        let dir = tempdir().expect("tempdir");
        let out = dir.path().join("today.md");
        write_fingerprint(&out, "abc").expect("write");
        let tmp_path = {
            let mut p = fingerprint_path(&out).into_os_string();
            p.push(".tmp");
            PathBuf::from(p)
        };
        assert!(!tmp_path.exists(), "tmp file must not linger after success");
    }
}
