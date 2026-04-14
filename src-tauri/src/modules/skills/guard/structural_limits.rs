#![allow(unused)]

//! Structural limits — file count, size, and binary file checks.
//!
//! Ported from Hermes `tools/skills_guard.py` lines 487-489 and structural checks.

use std::fs;
use std::path::Path;

/// Maximum number of files allowed in a skill directory.
pub const MAX_FILE_COUNT: usize = 50;

/// Maximum total size in KB for a skill directory.
pub const MAX_TOTAL_SIZE_KB: usize = 1024;

/// Maximum size in KB for a single file.
pub const MAX_SINGLE_FILE_KB: usize = 256;

/// Known binary/executable file extensions that should not be in a skill.
/// Extensions with and without leading dot are included since
/// Path::extension() returns extensions without a leading dot.
pub const SUSPICIOUS_BINARY_EXTENSIONS: &[&str] = &[
    ".exe", "exe", ".dll", "dll", ".so", "so", ".dylib", "dylib", ".bin", ".dat", ".com", ".msi",
    ".dmg", ".app", ".deb", ".rpm",
];

/// File extensions that are scanned (text files).
pub const SCANNABLE_EXTENSIONS: &[&str] = &[
    ".md", ".txt", ".py", ".sh", ".bash", ".js", ".ts", ".rb", ".yaml", ".yml", ".json", ".toml",
    ".cfg", ".ini", ".conf", ".html", ".css", ".xml", ".tex", ".r", ".jl", ".pl", ".php",
];

/// Represents a structural limit finding.
#[derive(Debug, Clone)]
pub struct StructuralFinding {
    /// Pattern ID for this finding
    pub pattern_id: &'static str,
    /// Severity of the finding
    pub severity: &'static str,
    /// Category of the finding
    pub category: &'static str,
    /// File path (relative) or "(directory)" for directory-level findings
    pub file: String,
    /// Line number (0 for file/directory-level findings)
    pub line: u32,
    /// Match text for display
    pub match_text: String,
    /// Human-readable description
    pub description: String,
}

/// Check if a file extension is considered suspicious/binary.
pub fn is_suspicious_binary_extension(ext: &str) -> bool {
    SUSPICIOUS_BINARY_EXTENSIONS
        .iter()
        .any(|e| ext.eq_ignore_ascii_case(e))
}

/// Check if a file extension is scannable (text file).
pub fn is_scannable_extension(ext: &str) -> bool {
    SCANNABLE_EXTENSIONS
        .iter()
        .any(|e| e.eq_ignore_ascii_case(ext))
}

/// Check the structure of a skill directory.
/// Returns findings for: file count, total size, binary files, oversized files,
/// unexpected executables, and symlinks pointing outside the directory.
pub fn check_structure(skill_dir: &Path) -> Vec<StructuralFinding> {
    let mut findings = Vec::new();
    let mut file_count: usize = 0;
    let mut total_size: u64 = 0;

    let dir_resolved = match skill_dir.canonicalize() {
        Ok(p) => p,
        Err(_) => return findings,
    };

    for entry in walkdir(skill_dir) {
        let path = entry;
        let rel_path = match path.strip_prefix(skill_dir) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => continue,
        };

        // Check for symlinks
        if path.is_symlink() {
            match path.canonicalize() {
                Ok(resolved) => {
                    // Check if resolved path is within the skill directory
                    let is_inside = resolved.strip_prefix(&dir_resolved).is_ok();
                    if !is_inside {
                        // Symlink points outside the skill directory
                        findings.push(StructuralFinding {
                            pattern_id: "symlink_escape",
                            severity: "critical",
                            category: "traversal",
                            file: rel_path.clone(),
                            line: 0,
                            match_text: format!("symlink -> {}", resolved.display()),
                            description: "symlink points outside the skill directory".to_string(),
                        });
                    }
                }
                Err(_) => {
                    // Broken or circular symlink
                    findings.push(StructuralFinding {
                        pattern_id: "broken_symlink",
                        severity: "medium",
                        category: "traversal",
                        file: rel_path.clone(),
                        line: 0,
                        match_text: "broken symlink".to_string(),
                        description: "broken or circular symlink".to_string(),
                    });
                }
            }
            continue;
        }

        if !path.is_file() {
            continue;
        }

        file_count += 1;

        // Get file size
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        let size = metadata.len();
        total_size += size;

        // Check individual file size
        let size_kb = size / 1024;
        if size > MAX_SINGLE_FILE_KB as u64 * 1024 {
            findings.push(StructuralFinding {
                pattern_id: "oversized_file",
                severity: "medium",
                category: "structural",
                file: rel_path.clone(),
                line: 0,
                match_text: format!("{}KB", size_kb),
                description: format!("file is {}KB (limit: {}KB)", size_kb, MAX_SINGLE_FILE_KB),
            });
        }

        // Check for binary/executable files
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        if is_suspicious_binary_extension(&ext) {
            findings.push(StructuralFinding {
                pattern_id: "binary_file",
                severity: "critical",
                category: "structural",
                file: rel_path.clone(),
                line: 0,
                match_text: format!("binary: .{}", ext),
                description: format!("binary/executable file (.{}) should not be in a skill", ext),
            });
        }

        // Check for unexpected executable permissions on non-script files
        let script_exts = [".sh", ".bash", ".py", ".rb", ".pl"];
        #[cfg(unix)]
        let is_executable = {
            use std::os::unix::fs::MetadataExt;
            (metadata.mode() & 0o111) != 0
        };
        #[cfg(not(unix))]
        let is_executable = false;

        if is_executable && !script_exts.iter().any(|e| ext == *e) {
            findings.push(StructuralFinding {
                pattern_id: "unexpected_executable",
                severity: "medium",
                category: "structural",
                file: rel_path.clone(),
                line: 0,
                match_text: "executable bit set".to_string(),
                description: "file has executable permission but is not a recognized script type"
                    .to_string(),
            });
        }
    }

    // Check file count limit
    if file_count > MAX_FILE_COUNT {
        findings.push(StructuralFinding {
            pattern_id: "too_many_files",
            severity: "medium",
            category: "structural",
            file: "(directory)".to_string(),
            line: 0,
            match_text: format!("{} files", file_count),
            description: format!("skill has {} files (limit: {})", file_count, MAX_FILE_COUNT),
        });
    }

    // Check total size limit
    let total_size_kb = total_size / 1024;
    if total_size_kb > MAX_TOTAL_SIZE_KB as u64 {
        findings.push(StructuralFinding {
            pattern_id: "oversized_skill",
            severity: "high",
            category: "structural",
            file: "(directory)".to_string(),
            line: 0,
            match_text: format!("{}KB total", total_size_kb),
            description: format!(
                "skill directory is {}KB (limit: {}KB)",
                total_size_kb, MAX_TOTAL_SIZE_KB
            ),
        });
    }

    findings
}

/// Simple directory walker that collects all paths recursively.
fn walkdir(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut results = Vec::new();
    walkdir_recursive(dir, &mut results);
    results
}

fn walkdir_recursive(dir: &Path, results: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walkdir_recursive(&path, results);
        } else {
            results.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_suspicious_binary_extensions() {
        assert!(is_suspicious_binary_extension(".exe"));
        assert!(is_suspicious_binary_extension(".dll"));
        assert!(!is_suspicious_binary_extension(".md"));
        assert!(!is_suspicious_binary_extension(".py"));
    }

    #[test]
    fn test_scannable_extensions() {
        assert!(is_scannable_extension(".md"));
        assert!(is_scannable_extension(".py"));
        assert!(is_scannable_extension(".js"));
        assert!(is_scannable_extension(".yaml"));
        assert!(!is_scannable_extension(".exe"));
        assert!(!is_scannable_extension(".dll"));
    }

    #[test]
    fn test_check_structure_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let findings = check_structure(tmp.path());
        assert!(findings.is_empty());
    }

    #[test]
    fn test_check_structure_normal_files() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().to_path_buf();

        // Create a normal file
        let mut file = fs::File::create(skill_dir.join("SKILL.md")).unwrap();
        writeln!(file, "# Test Skill").unwrap();

        let findings = check_structure(&skill_dir);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_check_structure_too_many_files() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().to_path_buf();

        // Create MAX_FILE_COUNT + 1 files
        for i in 0..=MAX_FILE_COUNT {
            let file_path = skill_dir.join(format!("file_{}.txt", i));
            let mut file = fs::File::create(file_path).unwrap();
            writeln!(file, "content {}", i).unwrap();
        }

        let findings = check_structure(&skill_dir);
        let too_many = findings.iter().find(|f| f.pattern_id == "too_many_files");
        assert!(too_many.is_some());
    }

    #[test]
    fn test_check_structure_oversized_file() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().to_path_buf();

        // Create a file larger than MAX_SINGLE_FILE_KB
        let file_path = skill_dir.join("large.txt");
        let mut file = fs::File::create(&file_path).unwrap();
        // Write more than 256KB
        let chunk = vec![b'x'; 1024];
        for _ in 0..(MAX_SINGLE_FILE_KB + 1) {
            file.write_all(&chunk).unwrap();
        }
        drop(file);

        let findings = check_structure(&skill_dir);
        let oversized = findings.iter().find(|f| f.pattern_id == "oversized_file");
        assert!(oversized.is_some());
    }

    #[test]
    fn test_check_structure_binary_file() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().to_path_buf();

        // Create a binary file
        let file_path = skill_dir.join("malicious.dll");
        fs::write(&file_path, b"binary content").unwrap();

        // Debug: list all files found by walkdir
        let files = walkdir(&skill_dir);
        eprintln!("DEBUG: walkdir found {} files:", files.len());
        for f in &files {
            eprintln!("  DEBUG: {:?}", f);
            if let Some(ext) = f.extension().and_then(|e| e.to_str()) {
                eprintln!("    ext: {}", ext);
                eprintln!("    is_suspicious: {}", is_suspicious_binary_extension(ext));
            }
        }

        let findings = check_structure(&skill_dir);
        eprintln!("DEBUG: findings count: {}", findings.len());
        for f in &findings {
            eprintln!("  DEBUG finding: {} - {}", f.pattern_id, f.description);
        }
        let binary = findings.iter().find(|f| f.pattern_id == "binary_file");
        assert!(binary.is_some());
    }
}
