# ADR-010: Skills Hub Not Introduced

**Status**: Accepted
**Date**: 2026-04-13
**Supersedes**: None
**Phase**: N/A (Not Adopted)

---

## Context

hermes-agent includes an 8-source Skills Hub for discovering and installing tools from external repositories:

```python
# hermes-agent/tools/skills_hub.py
class SkillsHub:
    SOURCES = [
        "claw_plugins",   # Official claw plugins
        "claw_hub",       # ClawHub marketplace
        "pypi",           # PyPI packages
        "github",         # GitHub repositories
        "custom1",        # Custom source 1
        "custom2",        # Custom source 2
        "builtin",        # Built-in skills
        "enterprise",     # Enterprise skill server
    ]

    TRUST_LEVELS = ["untrusted", "community", "trusted", "builtin"]

    async def discover(self, query: str) -> List[Skill]:
        # Search all sources
        ...

    async def install(self, skill: Skill) -> bool:
        # Install to local environment
        ...
```

---

## Decision

if2Ai does NOT introduce the Skills Hub. Local tool installation is handled by the existing **ToolRegistry**.

### Why NOT Adopted

#### 1. Security Risk (Primary Reason)

The 2026-02 ClawHub incident demonstrated critical risks in external skill marketplaces:

| Incident | Impact |
|----------|--------|
| 341 malicious skills discovered | Skills with data exfiltration, backdoors |
| 12 zero-day vulnerabilities in skill runtime | Remote code execution |
| No supply chain signing verification | Unverified code execution |

**if2Ai's position**: Desktop single-user app with local data — security attack surface must be minimized.

#### 2. Single-User Desktop Model

hermes-agent's Skills Hub is designed for:
- Multi-agent deployments
- Server-side infrastructure
- Team collaboration
- Centralized skill management

if2Ai is:
- Single-user desktop application
- Local tool installation
- No marketplace needed
- ToolRegistry sufficient

#### 3. Trust Model Mismatch

hermes-agent's trust levels assume:
- Multiple agents sharing skills
- Network-accessible skill servers
- Community curation

if2Ai's model:
- User controls their own tools
- Local-only installation
- Direct file system access

### Alternative: Enhanced ToolRegistry

if2Ai's ToolRegistry provides sufficient extensibility:

```rust
// src-tauri/src/modules/tools/registry.rs

/// Tool registry for if2Ai built-in and user-installed tools
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, ToolDef>>,
    builtin_path: PathBuf,
    user_tools_path: PathBuf,
}

impl ToolRegistry {
    /// Load all tools from standard locations
    pub async fn load(&self) -> Result<(), RegistryError> {
        // Load built-in tools
        self.load_directory(&self.builtin_path, ToolSource::Builtin)?;

        // Load user-installed tools
        self.load_directory(&self.user_tools_path, ToolSource::UserInstalled)?;

        Ok(())
    }

    /// Install a tool from a local path (future: signed package)
    pub async fn install(&self, path: PathBuf) -> Result<String, RegistryError> {
        let tool_def = self.load_tool_def(&path)?;
        let id = tool_def.id.clone();

        let dest = self.user_tools_path.join(&id);
        std::fs::create_dir_all(&dest)?;
        copy_dir(&path, &dest)?;

        self.tools.write().await.insert(id.clone(), tool_def);
        Ok(id)
    }

    /// Uninstall a user tool
    pub async fn uninstall(&self, id: &str) -> Result<(), RegistryError> {
        let mut tools = self.tools.write().await;
        let tool = tools.get(id).ok_or(RegistryError::NotFound)?;

        if tool.source == ToolSource::Builtin {
            return Err(RegistryError::CannotUninstallBuiltin);
        }

        tools.remove(id);

        // Remove files
        let path = self.user_tools_path.join(id);
        if path.exists() {
            std::fs::remove_dir_all(&path)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolSource {
    Builtin,
    UserInstalled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: ToolSource,
    pub version: String,
    pub schema: Value,  // JSON Schema for tool input
    pub permissions: Vec<Permission>,
}
```

### Future: Signed Tool Packages (P4+)

If tool installation from external sources is needed in the future:

```rust
/// Future: Signed tool package for secure installation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedToolPackage {
    pub tool_def: ToolDef,
    pub signature: String,        // Ed25519 signature
    pub signer_public_key: String,
    pub checksum: String,         // SHA-256 of package
}

impl SignedToolPackage {
    /// Verify package signature
    pub fn verify(&self, trusted_keys: &[String]) -> Result<(), PackageError> {
        // Check signer is trusted
        if !trusted_keys.contains(&self.signer_public_key) {
            return Err(PackageError::UntrustedSigner);
        }

        // Verify signature
        verify_ed25519(&self.signature, &self.tool_def)?;

        // Verify checksum
        let computed = sha256_file(&self.package_path)?;
        if computed != self.checksum {
            return Err(PackageError::ChecksumMismatch);
        }

        Ok(())
    }
}
```

---

## Comparison

| Aspect | hermes-agent Skills Hub | if2Ai ToolRegistry |
|--------|------------------------|-------------------|
| Sources | 8 (including external) | 2 (builtin + local) |
| Trust model | 4 levels (untrusted→builtin) | 2 levels (builtin/user) |
| Discovery | Search + browse marketplace | File system browsing |
| Installation | Remote + local | Local only |
| Security | Marketplace risk | Local only |
| Signing | Planned | Future (P4+) |
| Multi-agent | Yes | No (single user) |

---

## What We DO Implement

### 1. Local Tool Discovery

```rust
/// Discover tools in local directories
pub async fn discover_local_tools(&self, base_path: &Path) -> Result<Vec<ToolDef>> {
    let mut tools = Vec::new();

    if !base_path.exists() {
        return Ok(tools);
    }

    let mut entries = fs::read_dir(base_path).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_dir() && path.join("tool.yaml").exists() {
            match self.load_tool_def(&path) {
                Ok(tool) => tools.push(tool),
                Err(e) => log::warn!("Failed to load tool at {:?}: {}", path, e),
            }
        }
    }

    Ok(tools)
}
```

### 2. Tool Permission System

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Permission {
    FileRead(PathPattern),
    FileWrite(PathPattern),
    Network(NetworkScope),
    ExecuteCommand(Vec<String>),
}

impl Permission {
    /// Check if this permission is granted
    pub fn check(&self, requested: &Permission) -> bool {
        match (self, requested) {
            (Permission::FileRead(a), Permission::FileRead(b)) => a.matches(b),
            (Permission::ExecuteCommand(a), Permission::ExecuteCommand(b)) => {
                b.iter().all(|cmd| a.contains(cmd))
            }
            _ => false,
        }
    }
}
```

### 3. Built-in Tool Set

if2Ai's built-in tools are sufficient for most use cases:

| Tool | Category | Purpose |
|------|----------|---------|
| bash | System | Shell command execution |
| powershell | System | Windows shell |
| file_read | IO | Read file contents |
| file_write | IO | Write file contents |
| glob_search | Search | Find files by pattern |
| grep_search | Search | Search file contents |
| content_search | Search | Semantic content search |
| memory.recall | Memory | Retrieve memories |
| memory.store | Memory | Store memories |
| session.list | Session | List sessions |
| session.load | Session | Load session |

---

## Consequences

### Positive
- No external marketplace risk
- Simple security model
- User has full control over installed tools
- ToolRegistry already implemented

### Negative
- No automatic skill discovery
- No community skill sharing
- Users must manually install tools

### Neutral
- hermes-agent multi-agent features not relevant
- if2Ai can add signing in P4+ if needed

---

## ADR Index

| ADR | Title | Status |
|-----|-------|--------|
| ADR-001 | SQLite P0 Persistence | Accepted |
| ADR-002 | Active Retrieval vs Passive Invocation | Accepted |
| ADR-003 | FastEmbed + LanceDB Selection | Accepted |
| ADR-004 | Token Budget Allocation | Accepted |
| ADR-005 | Relationship with Upstream claw-cli | Accepted |
| ADR-006 | Security Design | Accepted |
| ADR-007 | HRR Introduction Timing (P2a) | Accepted |
| ADR-008 | Self-Learning Modules Independence | Accepted |
| ADR-009 | Trajectory Learning Timing (P3) | Accepted |
| ADR-010 | Skills Hub Not Introduced | Accepted |

---

## References

- [hermes-agent SkillsHub](https://github.com/1tius/hermes-agent/blob/main/tools/skills_hub.py)
- [2026-02 ClawHub Incident](https://github.com/1Audit/incident-reports/2026/02/clawhub-malicious-skills.md) — documented incident
- [if2Ai ToolRegistry](../src-tauri/src/modules/tools/registry.rs)
- [Tauri Security Model](https://tauri.app/security/)
