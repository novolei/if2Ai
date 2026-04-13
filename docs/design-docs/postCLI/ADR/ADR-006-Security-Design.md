# ADR-006: Security Design

**Status**: Accepted
**Date**: 2026-04-13
**Supersedes**: None
**Phase**: P0

---

## Context

Memory systems store sensitive user data and must protect against:

1. **Memory injection**: Malicious content injected into memory stores
2. **Path traversal**: Attackers accessing unintended files
3. **Data exfiltration**: Unauthorized access to memory data
4. **Corruption**: Atomicity violations causing data loss
5. **Malicious skills**: Untrusted tools accessing memory

hermes-agent has extensive threat scanning; if2Ai needs appropriate controls for a desktop app.

---

## Decision

Implement layered security for if2Ai memory system:

### Layer 1: Input Validation

```rust
/// Validate and sanitize memory entry inputs
pub fn validate_memory_entry(key: &str, content: &str) -> Result<(), SecurityError> {
    // Key validation
    if key.is_empty() || key.len() > 256 {
        return Err(SecurityError::InvalidKeyLength);
    }
    if !key.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.') {
        return Err(SecurityError::InvalidKeyFormat);
    }

    // Content validation
    let content_len = content.len();
    if content_len > 1_000_000 {
        return Err(SecurityError::ContentTooLarge(content_len));
    }

    // Check for injection patterns
    if contains_injection_pattern(content) {
        return Err(SecurityError::InjectionDetected);
    }

    Ok(())
}

fn contains_injection_pattern(content: &str) -> bool {
    let suspicious = [
        "<script", "javascript:", "data:text/html",
        "{{.", "{{=", "${", "#{", "\\x00",
    ];
    suspicious.iter().any(|p| content.to_lowercase().contains(p))
}
```

### Layer 2: Path Validation

```rust
/// Validate file paths to prevent traversal
pub fn validate_safe_path(base: &Path, requested: &Path) -> Result<PathBuf, SecurityError> {
    // Canonicalize both paths
    let base_canonical = base.canonicalize()
        .map_err(|_| SecurityError::BasePathInvalid)?;
    let requested_canonical = requested.canonicalize()
        .map_err(|_| SecurityError::PathTraversalAttempt)?;

    // Ensure requested path is under base
    if !requested_canonical.starts_with(&base_canonical) {
        return Err(SecurityError::PathTraversalAttempt);
    }

    Ok(requested_canonical)
}

/// Get safe memory database path
pub fn get_memory_db_path() -> Result<PathBuf, SecurityError> {
    let base = dirs::data_local_dir()
        .ok_or(SecurityError::NoDataDirectory)?;
    let memory_dir = base.join(".if2ai").join("memory");

    // Validate directory is under expected base
    let safe_path = validate_safe_path(&base, &memory_dir)?;

    // Create directory if needed
    std::fs::create_dir_all(&safe_path)
        .map_err(|_| SecurityError::DirectoryCreationFailed)?;

    Ok(safe_path.join("memory.db"))
}
```

### Layer 3: Atomic Writes

```rust
/// Atomic file write with rename
pub async fn atomic_write<P: AsRef<Path>, C: AsRef<[u8]>>(
    path: P,
    contents: C,
) -> Result<(), SessionError> {
    let path = path.as_ref();
    let temp_path = path.with_extension("tmp");

    // Write to temp file
    {
        let mut file = fs::File::create(&temp_path)
            .await
            .map_err(|e| SessionError::WriteError(e.to_string()))?;

        file.write_all(contents.as_ref())
            .await
            .map_err(|e| SessionError::WriteError(e.to_string()))?;

        // Sync to disk
        file.sync_all()
            .await
            .map_err(|e| SessionError::WriteError(e.to_string()))?;
    }

    // Atomic rename
    fs::rename(&temp_path, path)
        .await
        .map_err(|e| SessionError::WriteError(e.to_string()))?;

    Ok(())
}

/// Atomic JSON write for sessions
pub async fn atomic_json_write<P: AsRef<Path>, T: Serialize>(
    path: P,
    data: &T,
) -> Result<(), SessionError> {
    let contents = serde_json::to_string_pretty(data)
        .map_err(|e| SessionError::WriteError(e.to_string()))?;
    atomic_write(path, contents.as_bytes()).await
}
```

### Layer 4: Memory Access Control

```rust
/// Memory access context with permission checks
pub struct MemoryAccessContext {
    pub session_id: Option<String>,
    pub project_id: Option<String>,
    pub read_categories: Vec<MemoryCategory>,
    pub write_categories: Vec<MemoryCategory>,
}

impl MemoryAccessContext {
    pub fn for_session(session_id: &str) -> Self {
        Self {
            session_id: Some(session_id.to_string()),
            project_id: None,
            read_categories: vec![
                MemoryCategory::Conversation,
                MemoryCategory::Daily,
            ],
            write_categories: vec![
                MemoryCategory::Conversation,
            ],
        }
    }

    pub fn for_project(project_id: &str) -> Self {
        Self {
            session_id: None,
            project_id: Some(project_id.to_string()),
            read_categories: vec![
                MemoryCategory::Core,
                MemoryCategory::Daily,
                MemoryCategory::Conversation,
                MemoryCategory::Custom("project".to_string()),
            ],
            write_categories: vec![
                MemoryCategory::Core,
                MemoryCategory::Daily,
                MemoryCategory::Conversation,
                MemoryCategory::Custom("project".to_string()),
            ],
        }
    }

    pub fn can_read(&self, category: &MemoryCategory) -> bool {
        self.read_categories.iter().any(|c| c == category)
    }

    pub fn can_write(&self, category: &MemoryCategory) -> bool {
        self.write_categories.iter().any(|c| c == category)
    }
}
```

### Layer 5: Threat Scanning (Simplified from hermes-agent)

```rust
/// Simplified threat scanner for memory content
/// Note: Full path scanning (hermes-agent approach) requires more infrastructure
pub struct ThreatScanner {
    blocked_patterns: Vec<Regex>,
    max_content_size: usize,
}

impl Default for ThreatScanner {
    fn default() -> Self {
        Self {
            blocked_patterns: vec![
                // XSS patterns
                Regex::new(r"(?i)<script[^>]*>").unwrap(),
                Regex::new(r"(?i)javascript:").unwrap(),
                Regex::new(r"(?i)on\w+\s*=").unwrap(),
                // Path traversal
                Regex::new(r"\.\./").unwrap(),
                Regex::new(r"(?i)C:\\").unwrap(),
                Regex::new(r"(?i)/etc/passwd").unwrap(),
                // Shell injection
                Regex::new(r"[;&|`$]").unwrap(),
            ],
            max_content_size: 1_000_000,
        }
    }
}

impl ThreatScanner {
    pub fn scan(&self, content: &str) -> Result<(), ThreatDetected> {
        if content.len() > self.max_content_size {
            return Err(ThreatDetected::ContentTooLarge);
        }

        for pattern in &self.blocked_patterns {
            if pattern.is_match(content) {
                return Err(ThreatDetected::MaliciousPattern(
                    pattern.to_string()
                ));
            }
        }

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ThreatDetected {
    #[error("content exceeds maximum size")]
    ContentTooLarge,
    #[error("malicious pattern detected: {0}")]
    MaliciousPattern(String),
}
```

---

## Rationale

### Why Layered Security?

1. **Defense in depth**: Multiple layers catch different attack vectors
2. **Fail-safe defaults**: Layers can be independently disabled if needed
3. **Performance**: Heavy scanning only on write, not on every read

### Why NOT Full hermes-agent Threat Scanning?

hermes-agent's full threat scanning includes:
- Full file path scanning
- YARA rule matching
- Sandboxed execution

These require significant infrastructure. if2Ai's simplified approach:
- Regex pattern matching (sufficient for common attacks)
- File path validation (prevents traversal)
- Atomic writes (prevents corruption)

Full YARA scanning can be added in P4+ if needed.

### Why Atomic Writes?

Without atomic writes:
1. Process killed mid-write → corrupt session file
2. Read during write → partial/corrupt data
3. Crash recovery → inconsistent state

Atomic rename guarantees:
1. Either old file exists, or new file exists
2. Never partial/corrupt

---

## Consequences

### Positive
- Defense in depth against injection attacks
- Atomic writes prevent corruption
- Path validation prevents traversal
- Access control limits blast radius

### Negative
- Some performance overhead on writes (mitigated by caching)
- Regex patterns may need updates for new threats

### Neutral
- Security can be relaxed in development mode
- ToolRegistry has separate security model

---

## Implementation Notes

### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    #[error("invalid key length: {0}")]
    InvalidKeyLength(usize),

    #[error("invalid key format")]
    InvalidKeyFormat,

    #[error("content too large: {0} bytes")]
    ContentTooLarge(usize),

    #[error("injection pattern detected")]
    InjectionDetected,

    #[error("path traversal attempt")]
    PathTraversalAttempt,

    #[error("base path invalid")]
    BasePathInvalid,

    #[error("no data directory available")]
    NoDataDirectory,

    #[error("directory creation failed")]
    DirectoryCreationFailed,
}
```

### Integration with MemoryProvider

```rust
impl SqliteMemoryProvider {
    pub async fn store_secure(
        &self,
        ctx: &MemoryAccessContext,
        key: &str,
        content: &str,
        category: MemoryCategory,
    ) -> Result<(), MemoryError> {
        // Check write permission
        if !ctx.can_write(&category) {
            return Err(MemoryError::Generic("permission denied".to_string()));
        }

        // Validate input
        validate_memory_entry(key, content)?;

        // Scan for threats
        THREAT_SCANNER.scan(content)?;

        // Store
        self.store(key, content, category).await
    }
}
```

### Frozen Snapshot Security

```rust
/// Verify frozen snapshot integrity to detect prompt injection
pub fn verify_frozen_snapshot(snapshot: &FrozenSnapshot, current_prompt: &str) -> bool {
    let current_hash = compute_hash(current_prompt);
    if current_hash != snapshot.prompt_hash {
        // Prompt was modified — possible injection
        log::warn!(
            "Frozen snapshot mismatch: expected {}, got {}",
            snapshot.prompt_hash,
            current_hash
        );
        return false;
    }
    true
}
```

---

## Review Checklist

- [ ] Input validation rejects empty/too-long keys
- [ ] Input validation rejects suspicious patterns
- [ ] Path validation prevents traversal
- [ ] Atomic writes use temp + rename
- [ ] `sync_all()` called before rename
- [ ] MemoryAccessContext enforces read/write permissions
- [ ] ThreatScanner detects XSS patterns
- [ ] ThreatScanner detects path traversal patterns
- [ ] ThreatScanner detects shell injection
- [ ] Frozen snapshot verified on load
- [ ] Security errors return user-friendly messages

---

## References

- [hermes-agent Threat Scanner](https://github.com/1tius/hermes-agent/blob/main/tools/memory_tool.py)
- [OWASP XSS Prevention](https://cheatsheetseries.owasp.org/cheatsheets/Cross_Site_Scripting_Prevention_Cheat_Sheet.html)
- [Path Traversal Prevention](https://owasp.org/www-community/attacks/Path_Traversal)
- [Atomic File Writes](https://doc.rust-lang.org/std/fs/fn.rename.html)
