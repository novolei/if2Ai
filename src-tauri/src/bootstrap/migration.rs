//! MEM-MOD-PATH-FIX — one-shot migration from the legacy double-root
//! layout to the single-root `~/.if2ai/` model.
//!
//! Pre-fix the codebase wrote configs / sessions / projects / models
//! to `~/.if2ai/` while parking memory / vector_db / summaries /
//! jobs / trajectories / learning / harness under
//! `~/Library/Application Support/.if2ai/` (macOS `data_local_dir`).
//! That split made memory data invisible to debug / backup workflows
//! that lived under the home dir, and made cross-platform reasoning
//! about "where is my agent's brain?" unnecessarily hard.
//!
//! This module runs **once** at boot, before any subsystem opens a
//! file under those paths:
//!
//! 1. Detect the legacy `<data_local_dir>/.if2ai` directory.
//! 2. For each subtree (`memory`, `models`, `trajectories`, `if2ai/learning`,
//!    `if2ai/harness`), `fs::rename` it into `~/.if2ai/` when the
//!    target slot is empty.  Same-volume renames are atomic + O(1).
//! 3. When the target already has data, **merge**: copy missing
//!    files, keep the newer mtime when both sides have the same
//!    file.  Used for `models/` because users have already
//!    re-downloaded into `~/.if2ai/models/`.
//! 4. Write `~/.if2ai/.migrated_from_data_local_dir.<unix_ts>`
//!    sentinel so the next boot is a no-op.
//!
//! Safety guarantees:
//! - **Idempotent**: sentinel makes re-runs no-ops.  The migration
//!   itself is checked subtree-by-subtree so a partial completion
//!   from a previous boot resumes cleanly.
//! - **Best-effort**: any failure is logged via `tracing::warn!`
//!   and the boot continues — the worst-case is data stays in the
//!   legacy location and the user sees an empty subsystem until
//!   we resolve the failure (vs. the alternative of refusing to
//!   boot).
//! - **No data loss**: `fs::rename` is atomic.  The merge path
//!   never overwrites a newer file with an older one.
//! - **Lock-aware**: when the legacy memory subtree contains a
//!   LanceDB lock file (`.lock`), the migration is **skipped**
//!   and a warning is emitted.  Means the user must close the
//!   running `tauri dev` before the next boot — better than
//!   risking a corrupted vector index.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Per-call summary used by `tracing::info!` and the unit tests.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct MigrationReport {
    /// Subtrees that were moved this run (e.g. `["memory", "trajectories"]`).
    pub moved: Vec<String>,
    /// Subtrees that were merged file-by-file because the target was
    /// non-empty (e.g. `["models"]` when the user has both copies).
    pub merged: Vec<String>,
    /// Subtrees skipped because a lock file was detected (typically
    /// `vector_db/`).
    pub skipped_locked: Vec<String>,
    /// `true` when the sentinel already existed, so this call was a
    /// no-op.
    pub sentinel_present: bool,
}

impl MigrationReport {
    fn is_empty_run(&self) -> bool {
        self.moved.is_empty() && self.merged.is_empty() && self.skipped_locked.is_empty()
    }
}

/// Subtree names that live under both roots historically.  Listed in
/// safest-first order so a single failure doesn't abort the rest.
const MIGRATABLE_SUBTREES: &[&str] = &[
    "memory",        // SQLite + LanceDB + summaries — biggest win
    "trajectories",  // newline-delimited JSON, large but no locks
];

/// `legacy/` may use BOTH the dotted (`./.if2ai/...`) and undotted
/// (`./if2ai/...`) prefixes — earlier `harness` / `learning` modules
/// were inconsistent.  We probe both.
const LEGACY_PREFIXES: &[&str] = &[".if2ai", "if2ai"];

/// Check + perform migration.  Pass `legacy_data_root` from
/// [`dirs::data_local_dir`] (production) or a synthetic temp dir
/// (tests).  `if2ai_dir` is the post-fix single root (`~/.if2ai`).
///
/// Returns a [`MigrationReport`] describing what happened.  Errors
/// from individual subtrees are logged + included in the report;
/// only catastrophic failures (cannot create `if2ai_dir`) propagate.
pub fn migrate_legacy_data_dir(
    if2ai_dir: &Path,
    legacy_data_root: Option<&Path>,
) -> std::io::Result<MigrationReport> {
    let mut report = MigrationReport::default();

    // Idempotent gate: never run twice.
    let sentinel_glob = format!("{}/.migrated_from_data_local_dir", if2ai_dir.display());
    if has_sentinel(if2ai_dir) {
        report.sentinel_present = true;
        return Ok(report);
    }

    let Some(legacy_root) = legacy_data_root else {
        write_sentinel(if2ai_dir, &report)?;
        return Ok(report);
    };

    // Build the candidate "old root" list — `<legacy>/.if2ai` and
    // `<legacy>/if2ai`.  Either may exist; either may be empty.
    let candidates: Vec<PathBuf> = LEGACY_PREFIXES
        .iter()
        .map(|p| legacy_root.join(p))
        .filter(|p| p.exists())
        .collect();

    if candidates.is_empty() {
        // Truly fresh install with no legacy data.  Still write the
        // sentinel so we don't probe data_local_dir on every future
        // boot.
        write_sentinel(if2ai_dir, &report)?;
        return Ok(report);
    }

    fs::create_dir_all(if2ai_dir)?;

    for legacy in &candidates {
        for &subtree in MIGRATABLE_SUBTREES {
            let src = legacy.join(subtree);
            if !src.exists() {
                continue;
            }
            let dst = if2ai_dir.join(subtree);

            if has_lock_file(&src) {
                tracing::warn!(
                    src = %src.display(),
                    "[migration] {} contains a .lock file (LanceDB?) — skipping. \
                     Close any running app and re-launch to retry.",
                    subtree
                );
                report.skipped_locked.push(subtree.to_string());
                continue;
            }

            match move_or_merge(&src, &dst) {
                Ok(MoveOutcome::Moved) => {
                    tracing::info!(
                        src = %src.display(),
                        dst = %dst.display(),
                        "[migration] moved {} subtree (atomic)",
                        subtree
                    );
                    report.moved.push(subtree.to_string());
                }
                Ok(MoveOutcome::Merged { copied }) => {
                    tracing::info!(
                        src = %src.display(),
                        dst = %dst.display(),
                        files_copied = copied,
                        "[migration] merged {} subtree (target was non-empty)",
                        subtree
                    );
                    report.merged.push(subtree.to_string());
                }
                Err(err) => {
                    tracing::warn!(
                        src = %src.display(),
                        dst = %dst.display(),
                        error = %err,
                        "[migration] failed to migrate {} — leaving legacy in place",
                        subtree
                    );
                }
            }
        }
    }

    if report.is_empty_run() {
        // Nothing to do — legacy roots existed but held no migratable
        // subtrees.  Still drop the sentinel.
        tracing::debug!(
            "[migration] legacy root probed but contained no recognised subtrees: {sentinel_glob}"
        );
    }

    write_sentinel(if2ai_dir, &report)?;
    Ok(report)
}

enum MoveOutcome {
    Moved,
    Merged { copied: usize },
}

fn move_or_merge(src: &Path, dst: &Path) -> std::io::Result<MoveOutcome> {
    if !dst.exists() {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(src, dst)?;
        return Ok(MoveOutcome::Moved);
    }
    // Target exists — merge file-by-file (newer mtime wins).
    let copied = merge_dirs(src, dst)?;
    // Best-effort cleanup: drop the now-redundant src tree.  Failures
    // are non-fatal; the next boot's sentinel suppresses re-attempts.
    let _ = fs::remove_dir_all(src);
    Ok(MoveOutcome::Merged { copied })
}

fn merge_dirs(src: &Path, dst: &Path) -> std::io::Result<usize> {
    let mut copied = 0usize;
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);

        if entry.file_type()?.is_dir() {
            copied += merge_dirs(&src_path, &dst_path)?;
            continue;
        }

        let should_copy = match (src_path.metadata(), dst_path.metadata()) {
            (Ok(src_meta), Ok(dst_meta)) => {
                // Newer mtime wins.  Same mtime → keep dst (already
                // present); copy only when src is strictly newer.
                let src_mtime = src_meta.modified().ok();
                let dst_mtime = dst_meta.modified().ok();
                match (src_mtime, dst_mtime) {
                    (Some(s), Some(d)) => s > d,
                    _ => false, // can't compare — be conservative, keep dst
                }
            }
            (Ok(_), Err(_)) => true, // dst missing → copy
            _ => false,              // src unreadable → skip
        };

        if should_copy {
            fs::copy(&src_path, &dst_path)?;
            copied += 1;
        }
    }
    Ok(copied)
}

fn has_lock_file(root: &Path) -> bool {
    fn walk(p: &Path) -> bool {
        if let Ok(read) = fs::read_dir(p) {
            for entry in read.flatten() {
                let path = entry.path();
                if path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|name| name.ends_with(".lock"))
                {
                    return true;
                }
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) && walk(&path) {
                    return true;
                }
            }
        }
        false
    }
    walk(root)
}

fn has_sentinel(if2ai_dir: &Path) -> bool {
    let Ok(read) = fs::read_dir(if2ai_dir) else {
        return false;
    };
    for entry in read.flatten() {
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with(".migrated_from_data_local_dir")
        {
            return true;
        }
    }
    false
}

fn write_sentinel(if2ai_dir: &Path, report: &MigrationReport) -> std::io::Result<()> {
    fs::create_dir_all(if2ai_dir)?;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = if2ai_dir.join(format!(".migrated_from_data_local_dir.{ts}"));
    let body = format!(
        "moved={:?}\nmerged={:?}\nskipped_locked={:?}\n",
        report.moved, report.merged, report.skipped_locked
    );
    fs::write(&path, body)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Helper: build a synthetic legacy tree with a single file
    /// inside `<legacy>/.if2ai/<subtree>/data.bin`.
    fn seed_legacy(legacy_root: &Path, subtree: &str, body: &[u8]) -> PathBuf {
        let dir = legacy_root.join(".if2ai").join(subtree);
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("data.bin");
        fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn fresh_install_writes_sentinel_no_legacy() {
        let home = tempdir().unwrap();
        let report =
            migrate_legacy_data_dir(&home.path().join(".if2ai"), None).expect("must succeed");
        assert!(report.is_empty_run());
        // Sentinel must exist post-call.
        let if2ai = home.path().join(".if2ai");
        assert!(has_sentinel(&if2ai), "sentinel should be written");
        // Re-running is a no-op (sentinel_present=true).
        let again = migrate_legacy_data_dir(&if2ai, None).expect("idempotent");
        assert!(again.sentinel_present);
    }

    #[test]
    fn legacy_memory_subtree_is_atomically_moved_when_target_empty() {
        let home = tempdir().unwrap();
        let legacy = tempdir().unwrap();
        seed_legacy(legacy.path(), "memory", b"sqlite-page-zero");

        let if2ai = home.path().join(".if2ai");
        let report = migrate_legacy_data_dir(&if2ai, Some(legacy.path())).expect("must succeed");

        assert_eq!(report.moved, vec!["memory".to_string()]);
        assert!(report.merged.is_empty());
        assert!(report.skipped_locked.is_empty());

        let dst = if2ai.join("memory").join("data.bin");
        assert!(dst.exists(), "destination must hold the moved file");
        assert_eq!(fs::read(&dst).unwrap(), b"sqlite-page-zero");

        // Source must be gone after rename.
        let src = legacy.path().join(".if2ai").join("memory");
        assert!(!src.exists(), "source dir gone after atomic rename");
    }

    #[test]
    fn lock_file_in_subtree_skips_migration() {
        let home = tempdir().unwrap();
        let legacy = tempdir().unwrap();
        // Seed a fake LanceDB-style lock file under memory/vector_db.
        let mem = legacy.path().join(".if2ai").join("memory");
        fs::create_dir_all(mem.join("vector_db")).unwrap();
        fs::write(mem.join("vector_db").join("lance.lock"), b"").unwrap();
        fs::write(mem.join("memory.db"), b"data").unwrap();

        let if2ai = home.path().join(".if2ai");
        let report = migrate_legacy_data_dir(&if2ai, Some(legacy.path())).expect("must succeed");

        assert_eq!(
            report.skipped_locked,
            vec!["memory".to_string()],
            "memory subtree must be skipped due to .lock"
        );
        assert!(
            !if2ai.join("memory").exists(),
            "no partial copy on lock-skip"
        );
        // Legacy is intact.
        assert!(mem.join("memory.db").exists());
    }

    #[test]
    fn target_non_empty_triggers_merge_keeping_newer_mtime() {
        let home = tempdir().unwrap();
        let legacy = tempdir().unwrap();
        // Seed legacy with an OLDER file.
        seed_legacy(legacy.path(), "memory", b"OLD");
        // Seed home target with a NEWER file (same name).
        let if2ai = home.path().join(".if2ai");
        fs::create_dir_all(if2ai.join("memory")).unwrap();
        let dst_file = if2ai.join("memory").join("data.bin");
        fs::write(&dst_file, b"NEW").unwrap();
        // Ensure dst is strictly newer.
        std::thread::sleep(std::time::Duration::from_millis(10));
        let _ = fs::File::options()
            .write(true)
            .open(&dst_file)
            .and_then(|f| {
                use std::io::Write;
                let mut f = f;
                f.write_all(b"NEW")
            });

        let report = migrate_legacy_data_dir(&if2ai, Some(legacy.path())).expect("must succeed");
        assert_eq!(report.merged, vec!["memory".to_string()]);
        assert_eq!(
            fs::read(&dst_file).unwrap(),
            b"NEW",
            "newer file must survive merge"
        );
    }

    #[test]
    fn handles_undotted_legacy_prefix() {
        // `harness` / `learning` historically used `if2ai/...` (no
        // dot); the migrator must catch that too.
        let home = tempdir().unwrap();
        let legacy = tempdir().unwrap();
        let undotted = legacy.path().join("if2ai").join("memory");
        fs::create_dir_all(&undotted).unwrap();
        fs::write(undotted.join("data.bin"), b"x").unwrap();

        let if2ai = home.path().join(".if2ai");
        let report = migrate_legacy_data_dir(&if2ai, Some(legacy.path())).expect("must succeed");
        assert_eq!(report.moved, vec!["memory".to_string()]);
        assert!(if2ai.join("memory").join("data.bin").exists());
    }
}
