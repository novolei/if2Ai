# Security & Permissions

<cite>
**Referenced Files in This Document**
- [permissions.rs](file://rust/crates/runtime/src/permissions.rs)
- [oauth.rs](file://rust/crates/runtime/src/oauth.rs)
- [mod.rs](file://src-tauri/src/modules/security/mod.rs)
- [access.rs](file://src-tauri/src/modules/security/access.rs)
- [atomic_write.rs](file://src-tauri/src/modules/security/atomic_write.rs)
- [path.rs](file://src-tauri/src/modules/security/path.rs)
- [validation.rs](file://src-tauri/src/modules/security/validation.rs)
- [permission_service.rs](file://src-tauri/src/modules/application/permission_service.rs)
- [permissions.rs](file://src-tauri/src/modules/runtime/permissions.rs)
- [oauth.rs](file://src-tauri/src/modules/runtime/oauth.rs)
</cite>

## Table of Contents
1. [Introduction](#introduction)
2. [Project Structure](#project-structure)
3. [Core Components](#core-components)
4. [Architecture Overview](#architecture-overview)
5. [Detailed Component Analysis](#detailed-component-analysis)
6. [Dependency Analysis](#dependency-analysis)
7. [Performance Considerations](#performance-considerations)
8. [Troubleshooting Guide](#troubleshooting-guide)
9. [Conclusion](#conclusion)

## Introduction
This document explains the security and permissions system across the Rust runtime and Tauri application layers. It covers:
- Permission checking mechanisms and escalation flows
- OAuth integration with PKCE and credential storage
- Access control patterns for memory categories
- Atomic write operations for crash-safe persistence
- Path validation and input validation rules
- Security policies, enforcement logic, and validation procedures

## Project Structure
Security and permissions are implemented across two primary areas:
- Rust runtime crate: permission model, OAuth primitives, and shared types
- Tauri application: security modules (path, validation, access, atomic write), permission service bridge, and OAuth utilities

```mermaid
graph TB
subgraph "Rust Runtime"
RT_PERM["permissions.rs"]
RT_OAUTH["oauth.rs"]
end
subgraph "Tauri Application"
SEC_MOD["security/mod.rs"]
SEC_ACCESS["security/access.rs"]
SEC_ATOMIC["security/atomic_write.rs"]
SEC_PATH["security/path.rs"]
SEC_VAL["security/validation.rs"]
APP_PERM["application/permission_service.rs"]
RUNTIME_PERM["runtime/permissions.rs"]
RUNTIME_OAUTH["runtime/oauth.rs"]
end
RT_PERM --> RUNTIME_PERM
RT_OAUTH --> RUNTIME_OAUTH
SEC_MOD --> SEC_ACCESS
SEC_MOD --> SEC_ATOMIC
SEC_MOD --> SEC_PATH
SEC_MOD --> SEC_VAL
APP_PERM --> RUNTIME_PERM
```

**Diagram sources**
- [permissions.rs:1-233](file://rust/crates/runtime/src/permissions.rs#L1-L233)
- [oauth.rs:1-590](file://rust/crates/runtime/src/oauth.rs#L1-L590)
- [mod.rs:1-26](file://src-tauri/src/modules/security/mod.rs#L1-L26)
- [access.rs:1-104](file://src-tauri/src/modules/security/access.rs#L1-L104)
- [atomic_write.rs:1-122](file://src-tauri/src/modules/security/atomic_write.rs#L1-L122)
- [path.rs:1-145](file://src-tauri/src/modules/security/path.rs#L1-L145)
- [validation.rs:1-142](file://src-tauri/src/modules/security/validation.rs#L1-L142)
- [permission_service.rs:1-158](file://src-tauri/src/modules/application/permission_service.rs#L1-L158)
- [permissions.rs:1-248](file://src-tauri/src/modules/runtime/permissions.rs#L1-L248)
- [oauth.rs:1-595](file://src-tauri/src/modules/runtime/oauth.rs#L1-L595)

**Section sources**
- [mod.rs:1-26](file://src-tauri/src/modules/security/mod.rs#L1-L26)
- [permissions.rs:1-233](file://rust/crates/runtime/src/permissions.rs#L1-L233)
- [oauth.rs:1-590](file://rust/crates/runtime/src/oauth.rs#L1-L590)

## Core Components
- Permission model and policy engine
- OAuth 2.0 with PKCE and credential storage
- Memory access control by category
- Atomic write operations for crash-safe persistence
- Path validation against traversal attempts
- Input validation for memory entries

**Section sources**
- [permissions.rs:1-233](file://rust/crates/runtime/src/permissions.rs#L1-L233)
- [oauth.rs:1-590](file://rust/crates/runtime/src/oauth.rs#L1-L590)
- [access.rs:1-104](file://src-tauri/src/modules/security/access.rs#L1-L104)
- [atomic_write.rs:1-122](file://src-tauri/src/modules/security/atomic_write.rs#L1-L122)
- [path.rs:1-145](file://src-tauri/src/modules/security/path.rs#L1-L145)
- [validation.rs:1-142](file://src-tauri/src/modules/security/validation.rs#L1-L142)

## Architecture Overview
The system enforces layered security:
- Policy evaluation determines whether a tool invocation is permitted
- For escalated actions, a prompter requests explicit user consent
- Memory operations are validated, scoped, and written atomically
- Paths are strictly validated to prevent traversal
- Inputs are sanitized and checked for injection patterns

```mermaid
sequenceDiagram
participant Caller as "Caller"
participant Policy as "PermissionPolicy"
participant Prompter as "PermissionPrompter"
participant Access as "MemoryAccessContext"
participant Path as "validate_safe_path"
participant Val as "validate_memory_entry"
participant Atomic as "atomic_write"
Caller->>Policy : authorize(tool, input, prompter?)
alt meets requirement
Policy-->>Caller : Allow
else requires escalation
Policy->>Prompter : decide(request)
Prompter-->>Policy : Allow/Deny
Policy-->>Caller : Allow/Deny
end
Caller->>Access : can_read/write(category)
Access-->>Caller : true/false
Caller->>Path : validate_safe_path(base, requested)
Path-->>Caller : Ok/Err
Caller->>Val : validate_memory_entry(key, content)
Val-->>Caller : Ok/Err
Caller->>Atomic : atomic_write(path, contents)
Atomic-->>Caller : Ok/Err
```

**Diagram sources**
- [permissions.rs:88-134](file://rust/crates/runtime/src/permissions.rs#L88-L134)
- [permission_service.rs:43-86](file://src-tauri/src/modules/application/permission_service.rs#L43-L86)
- [access.rs:49-57](file://src-tauri/src/modules/security/access.rs#L49-L57)
- [path.rs:10-78](file://src-tauri/src/modules/security/path.rs#L10-L78)
- [validation.rs:6-34](file://src-tauri/src/modules/security/validation.rs#L6-L34)
- [atomic_write.rs:16-45](file://src-tauri/src/modules/security/atomic_write.rs#L16-L45)

## Detailed Component Analysis

### Permission Model and Policy Engine
- PermissionMode defines five levels: ReadOnly, WorkspaceWrite, DangerFullAccess, Prompt, Allow
- PermissionPolicy binds an active mode to per-tool requirements
- Authorization logic:
  - If active mode equals Allow or is greater than or equal to required, allow
  - Otherwise, if current mode is Prompt or WorkspaceWrite escalating to DangerFullAccess, consult prompter
  - Deny otherwise with a reason

```mermaid
classDiagram
class PermissionMode {
+as_str() str
}
class PermissionRequest {
+tool_name : string
+input : string
+current_mode : PermissionMode
+required_mode : PermissionMode
}
class PermissionPromptDecision {
+Allow
+Deny(reason : string)
}
class PermissionPrompter {
+decide(request) PermissionPromptDecision
}
class PermissionOutcome {
+Allow
+Deny(reason : string)
}
class PermissionPolicy {
+active_mode : PermissionMode
+tool_requirements : map<string, PermissionMode>
+new(mode) PermissionPolicy
+with_tool_requirement(name, required) PermissionPolicy
+required_mode_for(name) PermissionMode
+authorize(tool, input, prompter?) PermissionOutcome
}
PermissionPolicy --> PermissionMode : "uses"
PermissionPolicy --> PermissionRequest : "constructs"
PermissionPolicy --> PermissionPrompter : "invokes"
PermissionPrompter --> PermissionPromptDecision : "returns"
PermissionOutcome <-- PermissionPolicy : "returns"
```

**Diagram sources**
- [permissions.rs:3-135](file://rust/crates/runtime/src/permissions.rs#L3-L135)
- [permissions.rs:5-150](file://src-tauri/src/modules/runtime/permissions.rs#L5-L150)

**Section sources**
- [permissions.rs:1-233](file://rust/crates/runtime/src/permissions.rs#L1-L233)
- [permissions.rs:1-248](file://src-tauri/src/modules/runtime/permissions.rs#L1-L248)

### Permission Prompt Bridge (Tauri)
- Bridges synchronous PermissionPrompter with asynchronous Tauri IPC
- Emits a permission-request event and waits up to 60 seconds for a response
- Parses permission_mode strings into PermissionMode

```mermaid
sequenceDiagram
participant Policy as "PermissionPolicy"
participant Prompter as "TauriPermissionPrompter"
participant Frontend as "Frontend Window"
Policy->>Prompter : decide(request)
Prompter->>Frontend : emit "permission-request" {session_id, tool_name, permission_mode, message}
Frontend-->>Prompter : respond_permission(decision)
Prompter-->>Policy : PermissionPromptDecision
```

**Diagram sources**
- [permission_service.rs:43-86](file://src-tauri/src/modules/application/permission_service.rs#L43-L86)

**Section sources**
- [permission_service.rs:1-158](file://src-tauri/src/modules/application/permission_service.rs#L1-L158)

### Memory Access Control (Category-Based)
- MemoryAccessContext defines read/write categories per session or project
- Session contexts: read Conversation and Daily; write Conversation
- Project contexts: read/write Core, Daily, Conversation, and Custom("project")

```mermaid
classDiagram
class MemoryAccessContext {
+session_id : Option<string>
+project_id : Option<string>
+read_categories : Vec<MemoryCategory>
+write_categories : Vec<MemoryCategory>
+for_session(session_id) MemoryAccessContext
+for_project(project_id) MemoryAccessContext
+can_read(category) bool
+can_write(category) bool
}
```

**Diagram sources**
- [access.rs:8-58](file://src-tauri/src/modules/security/access.rs#L8-L58)

**Section sources**
- [access.rs:1-104](file://src-tauri/src/modules/security/access.rs#L1-L104)

### Atomic Write Operations
- Writes to a temporary file, flushes to disk, then atomically renames to the target
- Provides typed helpers for binary and pretty-printed JSON
- Errors encapsulate I/O and serialization failures

```mermaid
flowchart TD
Start(["Entry: atomic_write(path, contents)"]) --> Temp["Create temp file with .tmp extension"]
Temp --> Write["Write contents to temp file"]
Write --> Sync["Flush to disk (sync_all)"]
Sync --> Rename["Atomically rename temp → target"]
Rename --> Done(["Success"])
Write --> |Error| Cleanup["Cleanup temp file"]
Sync --> |Error| Cleanup
Rename --> |Error| Cleanup
Cleanup --> Fail(["Fail with WriteError"])
```

**Diagram sources**
- [atomic_write.rs:16-45](file://src-tauri/src/modules/security/atomic_write.rs#L16-L45)

**Section sources**
- [atomic_write.rs:1-122](file://src-tauri/src/modules/security/atomic_write.rs#L1-L122)

### Path Validation (Traversal Prevention)
- Canonicalizes base and requested paths
- Rejects attempts to escape the base directory via parent navigation or symlinks
- Supports both existing and non-existing paths

```mermaid
flowchart TD
A["validate_safe_path(base, requested)"] --> B{"base exists?"}
B --> |No| C["create_dir_all(base)"]
B --> |Yes| D["canonicalize(base)"]
C --> D
D --> E{"requested absolute?"}
E --> |Yes| F["use requested as-is"]
E --> |No| G["join(base, requested)"]
F --> H["full_path"]
G --> H
H --> I{"full_path exists?"}
I --> |Yes| J["canonicalize(full_path)"]
J --> K{"starts_with(base_canonical)?"}
K --> |No| Err1["PathTraversalAttempt"]
K --> |Yes| Ok1["return canonical"]
I --> |No| L["walk ancestors to nearest existing"]
L --> M["canonicalize(ancestor)"]
M --> N["strip_prefix(ancestor) to get remaining"]
N --> O["walk components: Normal join, ParentDir pop + check bounds"]
O --> P{"still under base?"}
P --> |No| Err2["PathTraversalAttempt"]
P --> |Yes| Ok2["return canonicalized path"]
```

**Diagram sources**
- [path.rs:10-78](file://src-tauri/src/modules/security/path.rs#L10-L78)

**Section sources**
- [path.rs:1-145](file://src-tauri/src/modules/security/path.rs#L1-L145)

### Input Validation (Memory Entries)
- Enforces key constraints: non-empty, length limit, allowed characters
- Enforces content constraints: size cap and injection pattern detection
- Returns structured errors for diagnostics

```mermaid
flowchart TD
VStart["validate_memory_entry(key, content)"] --> KeyCheck["Key: empty? length > 256? chars allowed?"]
KeyCheck --> |Fail| ErrKey["ValidationError"]
KeyCheck --> |Pass| SizeCheck["Content length ≤ 1MB?"]
SizeCheck --> |Fail| ErrSize["ValidationError"]
SizeCheck --> |Pass| InjectCheck["Contains XSS/template injections?"]
InjectCheck --> |Yes| ErrInject["ValidationError"]
InjectCheck --> |No| VOk["Ok"]
```

**Diagram sources**
- [validation.rs:6-34](file://src-tauri/src/modules/security/validation.rs#L6-L34)

**Section sources**
- [validation.rs:1-142](file://src-tauri/src/modules/security/validation.rs#L1-L142)

### OAuth Integration (PKCE, Credentials Storage)
- Generates PKCE code pairs and state
- Builds authorization URLs and token exchange/refresh forms
- Persists credentials to a JSON file with atomic write semantics for the credentials file
- Parses OAuth callbacks and validates the callback path

```mermaid
sequenceDiagram
participant Client as "OAuth Client"
participant Config as "OAuthConfig"
participant PKCE as "generate_pkce_pair()"
participant Auth as "Authorization Request"
participant Token as "Token Exchange/Refresh"
participant Store as "Credentials Storage"
Client->>PKCE : generate_pkce_pair()
PKCE-->>Client : {verifier, challenge}
Client->>Auth : build_url(authorize_url, client_id, redirect_uri, scopes, state, challenge)
Auth-->>Client : authorize_url
Client->>Token : exchange(code, redirect_uri, verifier)
Token-->>Client : access_token (+refresh_token?)
Client->>Store : save_oauth_credentials(token_set)
Store-->>Client : Ok
```

**Diagram sources**
- [oauth.rs:234-325](file://rust/crates/runtime/src/oauth.rs#L234-L325)
- [oauth.rs:113-232](file://rust/crates/runtime/src/oauth.rs#L113-L232)
- [oauth.rs:276-292](file://rust/crates/runtime/src/oauth.rs#L276-L292)

**Section sources**
- [oauth.rs:1-590](file://rust/crates/runtime/src/oauth.rs#L1-L590)
- [oauth.rs:1-595](file://src-tauri/src/modules/runtime/oauth.rs#L1-L595)

## Dependency Analysis
- Runtime permission types and policy are re-exported into the Tauri runtime module for application use
- Application permission service depends on runtime permission types and emits IPC events
- Security modules are consumed by memory, skills, voice, and browser modules
- OAuth utilities are shared between runtime and application layers

```mermaid
graph LR
RT_PERM["runtime/permissions.rs"] --> APP_PERM["application/permission_service.rs"]
RT_OAUTH["runtime/oauth.rs"] --> APP_OAUTH["runtime/oauth.rs (app)"]
SEC_MOD["security/mod.rs"] --> SEC_ACCESS["security/access.rs"]
SEC_MOD --> SEC_ATOMIC["security/atomic_write.rs"]
SEC_MOD --> SEC_PATH["security/path.rs"]
SEC_MOD --> SEC_VAL["security/validation.rs"]
APP_MEM["memory module"] --> SEC_PATH
APP_MEM --> SEC_VAL
APP_MEM --> SEC_ACCESS
APP_MEM --> SEC_ATOMIC
APP_SKL["skills module"] --> SEC_PATH
APP_SKL --> SEC_ATOMIC
```

**Diagram sources**
- [permissions.rs:1-248](file://src-tauri/src/modules/runtime/permissions.rs#L1-L248)
- [permission_service.rs:1-158](file://src-tauri/src/modules/application/permission_service.rs#L1-L158)
- [oauth.rs:1-595](file://src-tauri/src/modules/runtime/oauth.rs#L1-L595)
- [mod.rs:1-26](file://src-tauri/src/modules/security/mod.rs#L1-L26)

**Section sources**
- [mod.rs:1-26](file://src-tauri/src/modules/security/mod.rs#L1-L26)
- [permission_service.rs:1-158](file://src-tauri/src/modules/application/permission_service.rs#L1-L158)
- [permissions.rs:1-248](file://src-tauri/src/modules/runtime/permissions.rs#L1-L248)
- [oauth.rs:1-595](file://src-tauri/src/modules/runtime/oauth.rs#L1-L595)

## Performance Considerations
- Atomic writes incur a small overhead due to temporary file creation and fsync; appropriate for safety, minimal impact on typical memory operations
- Path canonicalization and traversal checks add negligible cost compared to filesystem operations
- Input validation is linear in key and content sizes; limits guard against excessive work
- Permission checks are constant-time lookups with a small map of tool requirements

## Troubleshooting Guide
Common issues and resolutions:
- Permission denied during escalation
  - Cause: Active mode insufficient for tool requirement
  - Resolution: Switch to a higher permission mode or approve escalation prompt
  - Evidence: Denial reason indicates required vs current mode
  - Section sources
    - [permissions.rs:127-134](file://rust/crates/runtime/src/permissions.rs#L127-L134)
    - [permissions.rs:142-149](file://src-tauri/src/modules/runtime/permissions.rs#L142-L149)

- Prompt timeout or missing frontend handler
  - Cause: Prompter waiting for response timed out or no handler registered
  - Resolution: Ensure the frontend listens for permission-request and responds with respond_permission
  - Section sources
    - [permission_service.rs:69-85](file://src-tauri/src/modules/application/permission_service.rs#L69-L85)

- Path traversal attempt
  - Cause: Requested path escapes base directory or uses unsafe components
  - Resolution: Use only subpaths under allowed base; avoid parent directory navigation
  - Section sources
    - [path.rs:37-77](file://src-tauri/src/modules/security/path.rs#L37-L77)

- Injection or oversized content rejected
  - Cause: Content contains suspicious patterns or exceeds size limits
  - Resolution: Sanitize content and reduce size; adhere to allowed key formats
  - Section sources
    - [validation.rs:22-34](file://src-tauri/src/modules/security/validation.rs#L22-L34)

- Atomic write failure
  - Cause: Disk I/O or rename failure; temp file cleanup issues
  - Resolution: Verify disk space and permissions; retry; inspect temp file artifacts
  - Section sources
    - [atomic_write.rs:58-65](file://src-tauri/src/modules/security/atomic_write.rs#L58-L65)

## Conclusion
The system combines a robust permission policy, secure OAuth flows, strict path and input validation, and crash-safe atomic writes to deliver a layered security model. Applications enforce access control at the memory category level, gate dangerous operations behind escalation prompts, and persist data safely using atomic file operations. Adhering to the documented policies and validation rules ensures secure operation across modules.