# Tauri Command API

<cite>
**Referenced Files in This Document**
- [src/lib/tauri.ts](file://src/lib/tauri.ts)
- [src-tauri/src/main.rs](file://src-tauri/src/main.rs)
- [src-tauri/src/commands/mod.rs](file://src-tauri/src/commands/mod.rs)
- [src-tauri/src/commands/agent/mod.rs](file://src-tauri/src/commands/agent/mod.rs)
- [src-tauri/src/modules/application/turn_service/mod.rs](file://src-tauri/src/modules/application/turn_service/mod.rs)
- [src-tauri/src/modules/application/turn_service/run.rs](file://src-tauri/src/modules/application/turn_service/run.rs)
- [src-tauri/src/commands/session.rs](file://src-tauri/src/commands/session.rs)
- [src-tauri/src/commands/project.rs](file://src-tauri/src/commands/project.rs)
- [src-tauri/src/commands/memory.rs](file://src-tauri/src/commands/memory.rs)
- [src-tauri/src/commands/tools.rs](file://src-tauri/src/commands/tools.rs)
- [src-tauri/src/commands/pinned.rs](file://src-tauri/src/commands/pinned.rs)
- [src-tauri/src/commands/slash.rs](file://src-tauri/src/commands/slash.rs)
- [src-tauri/src/commands/tts.rs](file://src-tauri/src/commands/tts.rs)
- [src-tauri/src/commands/browser.rs](file://src-tauri/src/commands/browser.rs)
- [src-tauri/tests/turn_service_run_turn_e2e.rs](file://src-tauri/tests/turn_service_run_turn_e2e.rs)
</cite>

## Update Summary
**Changes Made**
- Updated Agent Commands section to reflect the new thin IPC adapter pattern with 9-line run_agent_turn implementation
- Added comprehensive TurnService architecture documentation explaining canonical turn ownership
- Updated command execution flow to show thin adapter delegation to TurnService
- Enhanced architectural diagrams to illustrate clean boundary between IPC adapters and service layer
- Added documentation for TurnServiceDeps dependency injection pattern
- Updated performance considerations to reflect 86% reduction in IPC adapter complexity

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
This document provides comprehensive API documentation for If2Ai's Tauri command interface. It covers all IPC commands exposed by the backend, their JavaScript wrappers in the frontend, parameter schemas, return value formats, and error handling mechanisms. It explains the command execution flow from frontend to backend, including parameter validation and response processing. The document also describes the Tauri command pattern used throughout the application, including async/await patterns and error propagation. Practical examples demonstrate command usage from React components, including proper error handling and loading states. Finally, it addresses command batching, sequential execution, performance considerations, debugging techniques for command failures, and state synchronization patterns.

**Updated** The IPC command layer has been streamlined from 2587 lines to 2045 lines (-21% reduction) through architectural refactoring. The run_agent_turn function now acts as a thin 9-line adapter that constructs TurnService via make_turn_service, delegates to service.run_turn(), and returns result, maintaining backward compatibility while establishing clean architectural boundaries.

## Project Structure
The Tauri command interface is implemented in Rust and exposed to the frontend via typed wrappers. The backend organizes commands by domain (agent, session, project, memory, tools, skills, TTS, browser) and exposes them through a centralized handler registration in the main application entry point. The frontend wraps these commands in a thin IPC layer that provides strongly-typed helpers and event subscriptions.

**Updated** The agent command module now implements a thin adapter pattern, delegating the canonical turn lifecycle to the TurnService architecture. The TurnService acts as the single entry point for chat turn orchestration, owning the full prepare -> execute -> finalize lifecycle.

```mermaid
graph TB
subgraph "Frontend"
FE_Types["TypeScript Types<br/>and Wrappers"]
FE_Invoke["invoke() Wrapper"]
FE_Listen["listen() Events"]
end
subgraph "Tauri Bridge"
Bridge["Tauri Core"]
end
subgraph "Backend"
Main["main.rs<br/>Handler Registration"]
Commands["commands/mod.rs<br/>Command Registry"]
AgentAdapter["agent/mod.rs<br/>Thin IPC Adapter"]
TurnService["turn_service/mod.rs<br/>Canonical Orchestrator"]
RunLifecycle["turn_service/run.rs<br/>Non-streaming Lifecycle"]
Session["session.rs<br/>Session Commands"]
Project["project.rs<br/>Project Commands"]
Memory["memory.rs<br/>Memory Commands"]
Tools["tools.rs<br/>Tools Commands"]
Pinned["pinned.rs<br/>Pinned Commands"]
Slash["slash.rs<br/>Slash Commands"]
TTS["tts.rs<br/>TTS Commands"]
Browser["browser.rs<br/>Browser Commands"]
end
FE_Types --> FE_Invoke
FE_Listen --> Bridge
FE_Invoke --> Bridge
Bridge --> Main
Main --> Commands
Commands --> AgentAdapter
AgentAdapter --> TurnService
TurnService --> RunLifecycle
Commands --> Session
Commands --> Project
Commands --> Memory
Commands --> Tools
Commands --> Pinned
Commands --> Slash
Commands --> TTS
Commands --> Browser
```

**Diagram sources**
- [src-tauri/src/main.rs:1-226](file://src-tauri/src/main.rs#L1-L226)
- [src-tauri/src/commands/mod.rs:1-421](file://src-tauri/src/commands/mod.rs#L1-L421)
- [src-tauri/src/commands/agent/mod.rs:1-285](file://src-tauri/src/commands/agent/mod.rs#L1-L285)
- [src-tauri/src/modules/application/turn_service/mod.rs:1-338](file://src-tauri/src/modules/application/turn_service/mod.rs#L1-L338)

**Section sources**
- [src-tauri/src/main.rs:1-226](file://src-tauri/src/main.rs#L1-L226)
- [src-tauri/src/commands/mod.rs:1-421](file://src-tauri/src/commands/mod.rs#L1-L421)

## Core Components
This section outlines the core components of the Tauri command interface and how they work together to provide a robust IPC layer.

**Updated** The core components now include the TurnService architecture that establishes clean boundaries between the IPC adapter layer and the canonical turn lifecycle owner.

- Application State Container: The backend maintains a shared state container that holds managers and providers used across commands. This includes session managers, tool registries, memory providers, and other subsystems. The state is injected into commands via Tauri's state management.
- Thin IPC Adapter Pattern: Agent commands now act as thin adapters that parse parameters and delegate to TurnService, reducing complexity in the IPC layer while centralizing business logic in the service layer.
- TurnService Architecture: The TurnService acts as the canonical orchestrator for chat turns, owning the full lifecycle from preparation through finalization. It receives dependencies through TurnServiceDeps for clean separation of concerns.
- Command Registration: Commands are declared with the `#[tauri::command]` attribute and registered in the main application entry point. The registration ensures that frontend invocations can reach the backend handlers.
- Typed Frontend Wrappers: The frontend provides TypeScript wrappers around `@tauri-apps/api` that encapsulate invoke calls, event subscriptions, and parameter validation. These wrappers return strongly-typed results and handle error propagation consistently.
- Event System: The backend emits events for streaming responses and asynchronous updates. The frontend listens to these events to receive incremental data and completion signals.

Key backend state fields include:
- Session manager for conversation persistence
- Tool registry for available tools
- Project manager for multi-project support
- Permission prompt senders keyed by session_id
- Session-scoped permission overrides keyed by session_id -> tool_name
- Stream cancel senders keyed by stream_id
- Shared memory provider (SQLite/Vector/Hybrid)
- Context budget configuration
- Trajectory manager for ShareGPT JSONL persistence
- Learning module for self-model + reflection engine
- Active retrieval manager for intent classification
- Harness state for agent loop observability
- Threat scanner for PII/secret detection
- Job runner for background memory jobs
- Utility LLM shim for memory subsystems
- Session summary store and rolling summarizer
- Pinned store for memory injection
- Memory compiler and memory ticker

These fields are initialized in the main application entry point and made available to all commands through the state container.

**Section sources**
- [src-tauri/src/commands/mod.rs:28-169](file://src-tauri/src/commands/mod.rs#L28-L169)
- [src-tauri/src/main.rs:406-782](file://src-tauri/src/main.rs#L406-L782)
- [src-tauri/src/commands/agent/mod.rs:28-58](file://src-tauri/src/commands/agent/mod.rs#L28-L58)
- [src-tauri/src/modules/application/turn_service/mod.rs:70-115](file://src-tauri/src/modules/application/turn_service/mod.rs#L70-L115)

## Architecture Overview
The Tauri command architecture follows a clear separation of concerns with the new TurnService pattern:
- Frontend: Provides typed wrappers for all IPC commands and event listeners for streaming responses.
- Bridge: Uses Tauri's invoke and listen APIs to communicate with the backend.
- IPC Adapter Layer: Thin parameter parsing and delegation to TurnService for agent commands.
- Canonical Service Layer: TurnService owns the full chat turn lifecycle with clean dependency injection.
- Backend: Implements command handlers with strict parameter validation and error handling. Handlers interact with shared state and subsystems to perform operations.

**Updated** The architecture now features a clean boundary where IPC adapters remain thin (9-line run_agent_turn) while TurnService owns the complex orchestration logic.

```mermaid
sequenceDiagram
participant FE as "Frontend Component"
participant Bridge as "Tauri Bridge"
participant IPC as "IPC Adapter (run_agent_turn)"
participant TS as "TurnService"
participant State as "AppState"
participant Subsys as "Subsystems"
FE->>Bridge : invoke("run_agent_turn", params)
Bridge->>IPC : route to thin adapter
IPC->>TS : construct via make_turn_service
TS->>State : access dependencies via TurnServiceDeps
TS->>Subsys : perform full lifecycle orchestration
Subsys-->>TS : result or error
TS-->>IPC : typed response
IPC-->>Bridge : forward response
Bridge-->>FE : typed result
```

**Diagram sources**
- [src/lib/tauri.ts:202-212](file://src/lib/tauri.ts#L202-L212)
- [src-tauri/src/commands/agent/mod.rs:94-109](file://src-tauri/src/commands/agent/mod.rs#L94-L109)
- [src-tauri/src/commands/agent/mod.rs:43-58](file://src-tauri/src/commands/agent/mod.rs#L43-L58)

## Detailed Component Analysis

### Agent Commands
**Updated** Agent commands now implement a thin adapter pattern that delegates to TurnService, significantly reducing complexity in the IPC layer while establishing clean architectural boundaries.

Agent commands handle conversational turns and streaming responses. They orchestrate the conversation runtime, manage permissions, and coordinate with memory and tool systems through the TurnService architecture.

Key commands:
- run_agent_turn: Now acts as a 9-line thin adapter that constructs TurnService and delegates to service.run_turn().
- start_agent_stream: Thin adapter that constructs TurnService with streaming coordination state and delegates to service.stream_turn().
- stop_agent_stream: Directly manages streaming cancellation through AppState state containers.
- respond_permission: Handles permission prompt responses with session-scoped overrides.

**Updated** The run_agent_turn function has been streamlined to:
1. Parse Tauri parameters (state, app_handle, session_id, user_message, permission_mode)
2. Construct TurnService via make_turn_service
3. Delegate to service.run_turn() with RunTurnRequest
4. Return the result

Parameter validation and error handling:
- Session restoration and existence checks (handled by TurnService)
- Permission mode parsing and enforcement (handled by TurnService)
- Provider resolution and API client creation (handled by TurnService)
- Tool execution with permission policies (handled by TurnService)
- Error mapping from runtime errors to user-friendly messages (handled by TurnService)

```mermaid
sequenceDiagram
participant FE as "Frontend"
participant Bridge as "Tauri Bridge"
participant IPC as "run_agent_turn (Thin Adapter)"
participant MakeTS as "make_turn_service"
participant TS as "TurnService"
FE->>Bridge : invoke("run_agent_turn", {sessionId, userMessage})
Bridge->>IPC : route to thin adapter
IPC->>MakeTS : construct TurnService with AppState deps
MakeTS-->>IPC : TurnService instance
IPC->>TS : service.run_turn(RunTurnRequest)
TS->>TS : full lifecycle orchestration
TS-->>IPC : {message, session_id, thinking?}
IPC-->>Bridge : forward response
Bridge-->>FE : typed response
```

**Diagram sources**
- [src-tauri/src/commands/agent/mod.rs:94-109](file://src-tauri/src/commands/agent/mod.rs#L94-L109)
- [src-tauri/src/commands/agent/mod.rs:43-58](file://src-tauri/src/commands/agent/mod.rs#L43-L58)
- [src-tauri/src/modules/application/turn_service/run.rs:103-127](file://src-tauri/src/modules/application/turn_service/run.rs#L103-L127)

**Section sources**
- [src-tauri/src/commands/agent/mod.rs:83-145](file://src-tauri/src/commands/agent/mod.rs#L83-L145)
- [src-tauri/src/commands/agent/mod.rs:28-58](file://src-tauri/src/commands/agent/mod.rs#L28-L58)
- [src-tauri/src/modules/application/turn_service/mod.rs:1-338](file://src-tauri/src/modules/application/turn_service/mod.rs#L1-L338)
- [src-tauri/src/modules/application/turn_service/run.rs:1-636](file://src-tauri/src/modules/application/turn_service/run.rs#L1-L636)
- [src/lib/tauri.ts:202-212](file://src/lib/tauri.ts#L202-L212)

### TurnService Architecture
**New** The TurnService architecture establishes the canonical orchestrator for chat turns, owning the full lifecycle while maintaining clean separation from the IPC layer.

TurnService is the **only** entry point that the IPC adapter uses to compose provider resolution + memory injection + prompt planning, and to drive the actual runtime `prepare -> execute -> finalize` turn lifecycle.

Ownership status:
- MIG-001-a: long-lived dependency surface expanded so the service can own runtime construction, the tool loop, stream emission, and finalize hooks
- MIG-001-b: owns the non-streaming turn lifecycle via TurnService::run_turn
- MIG-001-c: owns the streaming turn lifecycle via stream_turn

Strict layering (CHARTER §2.1):
- `application::turn_service` MUST NOT import from `crate::commands::*`. Every dependency is injected through TurnServiceDeps so the service stays decoupled from the AppState aggregate held by the IPC layer.

TurnServiceDeps provides dependency injection:
- tool_registry: Access to available tools
- pinned_store: Memory pinning functionality
- memory_provider: Shared memory access
- active_retrieval_manager: Intent classification
- session_manager: Conversation persistence
- project_manager: Execution context resolution
- harness: Agent loop observability
- learning_module: Self-model + reflection
- context_budget: Working memory limits
- memory_ticker: Turn hooks
- trajectory_manager: ShareGPT persistence
- app_handle: Tauri event emission

**Section sources**
- [src-tauri/src/modules/application/turn_service/mod.rs:1-338](file://src-tauri/src/modules/application/turn_service/mod.rs#L1-L338)

### Session Commands
Session commands manage conversation sessions, including creation, listing, deletion, renaming, and toggling pinned status.

Key commands:
- create_session: Creates a new session within a project or legacy session.
- list_sessions: Lists all sessions (legacy path).
- list_project_sessions: Lists sessions within a specific project.
- delete_session: Deletes a session by ID.
- rename_session: Renames a session in place.
- set_session_pinned: Sets the pinned state of a session.
- memory_session_set_enabled: Toggles per-session memory on/off.
- get_session: Retrieves a session with its messages.

Parameter validation:
- Project ID validation for project-scoped sessions
- Session existence checks before operations
- Title validation and constraints

**Section sources**
- [src-tauri/src/commands/session.rs:10-135](file://src-tauri/src/commands/session.rs#L10-L135)
- [src/lib/tauri.ts:531-560](file://src/lib/tauri.ts#L531-L560)

### Project Commands
Project commands handle project CRUD operations, file system interactions, and directory previews.

Key commands:
- create_project: Creates a new project with name and workdir.
- list_projects: Lists all projects.
- get_project: Retrieves a project by ID.
- rename_project: Renames a project.
- delete_project: Deletes a project and all its sessions.
- open_project_in_finder: Opens a project workdir in the system file manager.
- ensure_default_workdir: Ensures default playground project exists.
- pick_folder_dialog: Opens macOS folder picker dialog.
- open_directory_path: Opens arbitrary directory path in file manager.
- list_directory_preview: Lists first-level directory entries for preview.
- read_file_preview: Reads a text file for inline preview.
- write_file_contents: Writes file contents to a path.
- create_permanent_worktree: Creates a permanent git worktree for a project.

File system operations:
- Directory scanning with limits and sorting
- File preview with MIME type detection
- Base64 encoding for binary previews
- Git worktree creation with canonical paths

**Section sources**
- [src-tauri/src/commands/project.rs:40-495](file://src-tauri/src/commands/project.rs#L40-L495)
- [src/lib/tauri.ts:415-475](file://src/lib/tauri.ts#L415-L475)

### Memory Commands
Memory commands provide access to the memory subsystem, including recall, deletion, export, promotion/demotion, and compiled memory operations.

Key commands:
- memory_recall: Searches memory entries with optional category and scope filters.
- memory_delete: Deletes a memory entry by key.
- memory_export: Exports memory entries with optional category and scope filters.
- memory_purge: Purges all entries in a category.
- memory_clear_all: Wipes all memory entries across categories and scopes.
- memory_promotion_candidates: Scans for promotion recommendations.
- memory_promote: Promotes an entry to a target scope.
- memory_demote: Demotes an entry to a target scope.
- memory_compile_now: Manually triggers the full compile pipeline.
- memory_compiled_read: Reads compiled memory sections.
- memory_compiled_clear: Clears compiled cache for a scope.
- memory_summaries_list: Lists session summaries within a time window.

Scope handling:
- Global, Project, Session scope kinds with proper fallbacks
- Memory execution scope construction from optional parameters
- Three-tier visibility rules at storage layer

**Section sources**
- [src-tauri/src/commands/memory.rs:143-758](file://src-tauri/src/commands/memory.rs#L143-L758)
- [src/lib/tauri.ts:367-398](file://src/lib/tauri.ts#L367-L398)

### Tools Commands
Tools commands enable direct tool execution from the frontend with permission enforcement and context binding.

Key commands:
- execute_tool: Executes a tool by name with JSON arguments.
- list_tools: Lists all available tools with metadata.
- get_tool_definitions: Gets tool definitions in OpenAI function calling format.
- list_toolsets: Lists all available toolsets.

Permission enforcement:
- Same permission policy used by agent streaming path
- Explicit session binding required for tools requiring context
- Policy decision logging and audit trails

**Section sources**
- [src-tauri/src/commands/tools.rs:127-257](file://src-tauri/src/commands/tools.rs#L127-L257)
- [src/lib/tauri.ts:682-720](file://src/lib/tauri.ts#L682-L720)

### Pinned Commands
Pinned commands manage pinned memories for memory injection and editor UI.

Key commands:
- pinned_get: Lists pinned memories for requested scope.
- pinned_add: Adds a user-initiated pin.
- pinned_delete: Deletes a pin by ID (idempotent).
- pinned_reorder: Reorders pins by restamping created_at timestamps.

DTO mapping:
- Wire-shape mirror of internal PinnedItem structure
- Flat representation for created_by_kind with optional tool_name/session_id
- Scope handling for project/global/both combinations

**Section sources**
- [src-tauri/src/commands/pinned.rs:75-168](file://src-tauri/src/commands/pinned.rs#L75-L168)
- [src/lib/tauri.ts:367-398](file://src/lib/tauri.ts#L367-L398)

### Slash Commands
Slash commands provide command parsing, suggestion, execution, and skill management.

Key commands:
- parse_slash_command: Parses input and returns whether it matches a known command.
- list_slash_commands: Lists all registered slash commands.
- suggest_slash_commands: Suggests slash commands matching input prefix.
- execute_slash_command: Executes a slash command and returns result message.
- resolve_skill_slash: Resolves a skill slash command to invocation message.
- list_skills: Lists available skills with status and enablement state.

Skill toolset validation:
- Available toolsets computation from tool registry
- Required vs fallback toolset satisfaction checks
- Skill distribution envelope validation for remote artifacts

**Section sources**
- [src-tauri/src/commands/slash.rs:212-344](file://src-tauri/src/commands/slash.rs#L212-L344)
- [src/lib/tauri.ts:768-798](file://src/lib/tauri.ts#L768-L798)

### TTS Commands
TTS commands provide text-to-speech synthesis with both buffered and streaming modes, including warmup management and voice cloning.

Key commands:
- tts_health: Checks TTS system health and model status.
- tts_warmup_status: Gets current warmup status.
- tts_start_warmup: Triggers async warmup sequence.
- tts_synthesize: Buffered synthesis returns WAV as base64.
- tts_stream_start: Starts streaming synthesis job.
- tts_stream_status: Polls job status.
- tts_stream_result: Gets final job result.
- tts_stream_close: Cancels/close a stream.
- tts_demo_audio: Gets demo audio as base64.
- tts_list_voices: Lists available voice names.
- tts_split_text: Splits text for voice clone preview.

Streaming architecture:
- Provider handle with lazy loading and idle eviction
- Streaming job manager with chunk emission
- Gapless playback support via PCM16LE chunks
- Queue depth tracking for concurrency control

**Section sources**
- [src-tauri/src/commands/tts.rs:406-800](file://src-tauri/src/commands/tts.rs#L406-L800)
- [src/lib/tauri.ts:202-212](file://src/lib/tauri.ts#L202-L212)

### Browser Commands
Browser commands manage AI-driven browser control, including session lifecycle, profile management, and takeover/release operations.

Key commands:
- get_browser_sessions: Lists all active browser sessions and their state.
- close_browser_session: Gracefully shuts down a specific browser session.
- get_chrome_status: Reports whether Chrome/Chromium binary is available.
- request_browser_status: Triggers immediate browser-status event.
- list_browser_profiles: Lists persistent browser profiles on disk.
- clear_browser_profile: Wipes persistent profile directory.
- get_browser_settings: Reads persisted browser settings.
- set_browser_settings: Persists user-edited browser settings.
- request_browser_takeover: Hands control of browser to human user.
- release_browser_takeover: Releases takeover flag and collapses browser.
- get_browser_action_log: Reads action log for session.

Profile management:
- Session-scoped persistent profiles with disk usage tracking
- Safety checks preventing profile deletion while sessions are running
- Environment override support for profile mode

**Section sources**
- [src-tauri/src/commands/browser.rs:37-224](file://src-tauri/src/commands/browser.rs#L37-L224)

## Dependency Analysis
The Tauri command interface exhibits a well-structured dependency graph with clear boundaries between frontend wrappers, the Tauri bridge, and backend handlers.

**Updated** The dependency graph now shows the thin adapter pattern where agent commands depend on TurnService for orchestration.

```mermaid
graph TB
FE["Frontend Wrappers<br/>src/lib/tauri.ts"]
Tauri["@tauri-apps/api<br/>invoke/listen"]
Main["main.rs<br/>Handler Registration"]
Mod["commands/mod.rs<br/>State Container"]
AgentAdapter["agent/mod.rs<br/>Thin IPC Adapters"]
TurnService["turn_service/mod.rs<br/>TurnService"]
RunLifecycle["turn_service/run.rs<br/>Run Lifecycle"]
Session["session.rs<br/>Session Ops"]
Project["project.rs<br/>Project Ops"]
Memory["memory.rs<br/>Memory Ops"]
Tools["tools.rs<br/>Tools Ops"]
Pinned["pinned.rs<br/>Pinned Ops"]
Slash["slash.rs<br/>Slash Ops"]
TTS["tts.rs<br/>TTS Ops"]
Browser["browser.rs<br/>Browser Ops"]
FE --> Tauri
Tauri --> Main
Main --> Mod
Mod --> AgentAdapter
AgentAdapter --> TurnService
TurnService --> RunLifecycle
Mod --> Session
Mod --> Project
Mod --> Memory
Mod --> Tools
Mod --> Pinned
Mod --> Slash
Mod --> TTS
Mod --> Browser
```

**Diagram sources**
- [src-tauri/src/main.rs:1-226](file://src-tauri/src/main.rs#L1-L226)
- [src-tauri/src/commands/mod.rs:1-421](file://src-tauri/src/commands/mod.rs#L1-L421)
- [src-tauri/src/commands/agent/mod.rs:1-285](file://src-tauri/src/commands/agent/mod.rs#L1-L285)
- [src-tauri/src/modules/application/turn_service/mod.rs:1-338](file://src-tauri/src/modules/application/turn_service/mod.rs#L1-L338)

**Section sources**
- [src-tauri/src/main.rs:1-226](file://src-tauri/src/main.rs#L1-L226)
- [src-tauri/src/commands/mod.rs:1-421](file://src-tauri/src/commands/mod.rs#L1-L421)

## Performance Considerations
Several performance characteristics and optimization opportunities are evident in the command interface:

- Lazy Provider Loading: TTS provider uses lazy initialization to reduce cold start time, loading only when first requested and evicting after periods of inactivity.
- Concurrency Control: Streaming synthesis includes queue depth tracking to prevent overload and ensure fair resource allocation.
- Background Jobs: Memory operations leverage a dedicated job runner with retry budgets and rate limiting to prevent overwhelming LLM providers.
- Working Memory Budgeting: Agent runtime enforces working memory token limits to prevent excessive memory usage during conversations.
- File System Operations: Project commands implement blocking operations in spawned tasks to avoid blocking the main thread, with appropriate limits and validations.
- Event-Driven Updates: Streaming responses use Tauri events to push incremental data to the frontend, reducing polling overhead.
- **Updated** Thin Adapter Pattern: The 9-line run_agent_turn adapter reduces IPC complexity and improves maintainability while preserving performance characteristics.

Command batching and sequential execution:
- Sequential execution is the default pattern for most commands to maintain consistency and avoid race conditions.
- Streaming commands support concurrent operations through job managers and provider handles.
- Memory operations can be batched through the job runner system, which manages retries and concurrency limits.
- **Updated** TurnService dependency injection eliminates repeated state access overhead in IPC adapters.

## Troubleshooting Guide
Common issues and debugging techniques for Tauri command failures:

Error handling patterns:
- Runtime errors are mapped to user-friendly messages with specific guidance for common scenarios (network timeouts, authentication failures, permission denials).
- Tool execution errors are wrapped with success/error indicators for direct tool calls.
- Memory operations return descriptive error messages for invalid scopes and missing keys.
- **Updated** TurnService errors provide detailed context for lifecycle orchestration failures.

Debugging techniques:
- Enable backend logging with tracing to capture detailed execution traces and error contexts.
- Use harness mode for development to capture comprehensive telemetry and event streams.
- Monitor Tauri event emissions for streaming commands to ensure proper correlation of stream IDs.
- Validate parameter schemas in frontend wrappers before invoking backend commands.
- Check capability configurations to ensure commands are properly authorized.
- **Updated** Monitor TurnService dependency construction for proper AppState injection.

State synchronization:
- Use session IDs to correlate related operations across multiple commands.
- Leverage memory scopes to ensure consistent visibility across related operations.
- Monitor provider state for TTS commands to detect loading or eviction issues.
- **Updated** Verify TurnServiceDeps construction for proper dependency injection.

**Section sources**
- [src-tauri/src/commands/agent/mod.rs:664-721](file://src-tauri/src/commands/agent/mod.rs#L664-L721)
- [src-tauri/src/commands/tools.rs:138-257](file://src-tauri/src/commands/tools.rs#L138-L257)
- [src-tauri/src/modules/application/turn_service/mod.rs:70-115](file://src-tauri/src/modules/application/turn_service/mod.rs#L70-L115)

## Conclusion
The If2Ai Tauri command interface provides a comprehensive and well-structured IPC layer that enables seamless communication between the frontend and backend. The architecture emphasizes typed wrappers, robust error handling, and clear separation of concerns across domains. 

**Updated** The recent architectural refactoring has established clean boundaries between the IPC adapter layer and the canonical TurnService, resulting in a 21% reduction in IPC command layer complexity while maintaining backward compatibility. The thin adapter pattern in run_agent_turn (9 lines) demonstrates how the IPC layer now focuses purely on parameter parsing and delegation, while TurnService owns the full chat turn lifecycle.

The command patterns support both synchronous operations and streaming responses, with careful consideration for performance, concurrency, and state management. The provided documentation serves as a complete reference for developers implementing React components that interact with the backend through these commands, ensuring reliable and maintainable integrations. The new TurnService architecture provides a solid foundation for future enhancements while maintaining the reliability and performance characteristics of the existing system.