# Application Layer

<cite>
**Referenced Files in This Document**
- [application/mod.rs](file://src-tauri/src/modules/application/mod.rs)
- [turn_service/mod.rs](file://src-tauri/src/modules/application/turn_service/mod.rs)
- [provider_service.rs](file://src-tauri/src/modules/application/provider_service.rs)
- [permission_service.rs](file://src-tauri/src/modules/application/permission_service.rs)
- [real_api_client.rs](file://src-tauri/src/modules/application/real_api_client.rs)
- [prompt_planner/mod.rs](file://src-tauri/src/modules/application/prompt_planner/mod.rs)
- [prompt_planner/governor.rs](file://src-tauri/src/modules/application/prompt_planner/governor.rs)
- [prompt_planner/preflight.rs](file://src-tauri/src/modules/application/prompt_planner/preflight.rs)
- [prompt_planner/sanitize.rs](file://src-tauri/src/modules/application/prompt_planner/sanitize.rs)
- [memory_coordinator.rs](file://src-tauri/src/modules/application/memory_coordinator.rs)
- [stream_emitter_service.rs](file://src-tauri/src/modules/application/stream_emitter_service.rs)
- [stream_cancel_service.rs](file://src-tauri/src/modules/application/stream_cancel_service.rs)
- [tool_executor.rs](file://src-tauri/src/modules/application/tool_executor.rs)
- [tool_heuristics.rs](file://src-tauri/src/modules/application/tool_heuristics.rs)
- [trajectory_service.rs](file://src-tauri/src/modules/application/trajectory_service.rs)
- [request_intelligence_service.rs](file://src-tauri/src/modules/application/request_intelligence_service.rs)
- [activation_service.rs](file://src-tauri/src/modules/application/activation_service.rs)
- [commands/agent/mod.rs](file://src-tauri/src/commands/agent/mod.rs)
</cite>

## Update Summary
**Changes Made**
- Added documentation for the newly extracted `stream_cancel_service.rs` module and its role in centralized cancellation management
- Updated permission service documentation to reflect the extraction from `commands/agent.rs` into `permission_service.rs`
- Enhanced IPC command adapter documentation to show how thin adapters delegate to application-layer services
- Updated architecture diagrams to reflect the new service boundaries and delegation patterns

## Table of Contents
1. [Introduction](#introduction)
2. [Project Structure](#project-structure)
3. [Core Components](#core-components)
4. [Architecture Overview](#architecture-overview)
5. [Detailed Component Analysis](#detailed-component-analysis)
6. [IPC Command Adapters](#ipc-command-adapters)
7. [Dependency Analysis](#dependency-analysis)
8. [Performance Considerations](#performance-considerations)
9. [Troubleshooting Guide](#troubleshooting-guide)
10. [Conclusion](#conclusion)

## Introduction
This document describes the application layer module architecture that orchestrates a single agent turn. It focuses on the business logic modules responsible for activation, license lifecycle, memory orchestration, tool execution, and trajectory recording. It also documents the prompt planner components (governor, preflight, sanitize), the provider service, permission service, and real API client integration. The document explains application-level orchestration patterns, service composition, inter-module communication, and outlines initialization, dependency injection, and error handling strategies.

**Updated** The application layer now features centralized cancellation management through `stream_cancel_service.rs` and extracted permission handling from IPC adapters into dedicated application services.

## Project Structure
The application layer resides under `src-tauri/src/modules/application` and exposes a curated set of services and utilities. The module acts as a thin orchestration layer between IPC command adapters and lower-level runtime/memory/tools/api modules. It defines typed inputs/outputs and stable contracts to support future M2+ projections and M4+ governance.

```mermaid
graph TB
subgraph "Application Layer"
A["turn_service/mod.rs"]
B["provider_service.rs"]
C["prompt_planner/mod.rs"]
D["memory_coordinator.rs"]
E["stream_emitter_service.rs"]
F["permission_service.rs"]
G["real_api_client.rs"]
H["tool_executor.rs"]
I["trajectory_service.rs"]
J["request_intelligence_service.rs"]
K["activation_service.rs"]
L["stream_cancel_service.rs"]
end
subgraph "IPC Command Adapters"
M["commands/agent/mod.rs"]
end
subgraph "Lower Layers"
R1["runtime/*"]
R2["memory/*"]
R3["tools/*"]
R4["api/*"]
R5["control_plane/*"]
end
A --> B
A --> C
A --> D
A --> J
D --> R2
C --> R1
E --> D
E --> R2
F --> R1
G --> R4
H --> R3
H --> R5
I --> R1
K --> R1
M --> A
M --> F
M --> L
```

**Diagram sources**
- [application/mod.rs:45-124](file://src-tauri/src/modules/application/mod.rs#L45-L124)
- [turn_service/mod.rs:1-347](file://src-tauri/src/modules/application/turn_service/mod.rs#L1-L347)
- [provider_service.rs:22-125](file://src-tauri/src/modules/application/provider_service.rs#L22-L125)
- [prompt_planner/mod.rs:35-373](file://src-tauri/src/modules/application/prompt_planner/mod.rs#L35-L373)
- [memory_coordinator.rs:42-335](file://src-tauri/src/modules/application/memory_coordinator.rs#L42-L335)
- [stream_emitter_service.rs:12-122](file://src-tauri/src/modules/application/stream_emitter_service.rs#L12-L122)
- [permission_service.rs:1-251](file://src-tauri/src/modules/application/permission_service.rs#L1-L251)
- [real_api_client.rs:10-171](file://src-tauri/src/modules/application/real_api_client.rs#L10-L171)
- [tool_executor.rs:8-143](file://src-tauri/src/modules/application/tool_executor.rs#L8-L143)
- [trajectory_service.rs:8-58](file://src-tauri/src/modules/application/trajectory_service.rs#L8-L58)
- [request_intelligence_service.rs:22-61](file://src-tauri/src/modules/application/request_intelligence_service.rs#L22-L61)
- [activation_service.rs:37-254](file://src-tauri/src/modules/application/activation_service.rs#L37-L254)
- [stream_cancel_service.rs:1-51](file://src-tauri/src/modules/application/stream_cancel_service.rs#L1-L51)
- [commands/agent/mod.rs:1-220](file://src-tauri/src/commands/agent/mod.rs#L1-L220)

**Section sources**
- [application/mod.rs:1-124](file://src-tauri/src/modules/application/mod.rs#L1-L124)

## Core Components
- TurnService: Composes provider resolution, request intelligence, memory orchestration, and prompt planning into a single PreparedChatInputs bundle for the runtime.
- ProviderService: Resolves a concrete provider client, model, and timeout from configuration.
- PromptPlanner: Builds a structured PromptPlan from system prompt, web-tools guide, memory injection sections, and optional active strategy overlay.
- MemoryCoordinator: Orchestrates per-turn recall/injection and post-turn write decisions (policy, quality gate, conflict resolution).
- StreamEmitterService: Dispatches a typed after-turn envelope to both frontend and harness transports.
- PermissionService: Bridges runtime PermissionPrompter with Tauri IPC for permission prompts (extracted from IPC adapters).
- RealApiClient: Implements the runtime ApiClient over the outbound ProviderClient.
- ToolExecutor: Bridges async ToolRegistry to the runtime's ToolExecutor trait and loads control plane switches.
- ToolHeuristics: Heuristics for interpreting tool results and shell commands.
- TrajectoryService: Records conversation trajectories for future RL training.
- RequestIntelligenceService: Runs a deterministic classifier to decide execution mode.
- ActivationService: Wraps onboarding-style activation and exposes typed activation snapshots.
- StreamCancelService: Centralized service for managing agent stream cancellations across the application.

**Updated** Added StreamCancelService for centralized cancellation management and updated PermissionService description to reflect extraction from IPC adapters.

**Section sources**
- [turn_service/mod.rs:152-322](file://src-tauri/src/modules/application/turn_service/mod.rs#L152-L322)
- [provider_service.rs:31-125](file://src-tauri/src/modules/application/provider_service.rs#L31-L125)
- [prompt_planner/mod.rs:56-373](file://src-tauri/src/modules/application/prompt_planner/mod.rs#L56-L373)
- [memory_coordinator.rs:57-335](file://src-tauri/src/modules/application/memory_coordinator.rs#L57-L335)
- [stream_emitter_service.rs:21-122](file://src-tauri/src/modules/application/stream_emitter_service.rs#L21-L122)
- [permission_service.rs:15-251](file://src-tauri/src/modules/application/permission_service.rs#L15-L251)
- [real_api_client.rs:18-171](file://src-tauri/src/modules/application/real_api_client.rs#L18-L171)
- [tool_executor.rs:52-143](file://src-tauri/src/modules/application/tool_executor.rs#L52-L143)
- [tool_heuristics.rs:1-104](file://src-tauri/src/modules/application/tool_heuristics.rs#L1-L104)
- [trajectory_service.rs:14-58](file://src-tauri/src/modules/application/trajectory_service.rs#L14-L58)
- [request_intelligence_service.rs:29-61](file://src-tauri/src/modules/application/request_intelligence_service.rs#L29-L61)
- [activation_service.rs:74-254](file://src-tauri/src/modules/application/activation_service.rs#L74-L254)
- [stream_cancel_service.rs:16-51](file://src-tauri/src/modules/application/stream_cancel_service.rs#L16-L51)

## Architecture Overview
The application layer enforces strict separation of concerns:
- Application services depend on runtime, memory, tools, and api modules, but not on commands.
- Orchestration occurs via typed inputs/outputs and stable contracts.
- Cross-cutting concerns (permissions, telemetry, tracing) are centralized.
- IPC command adapters remain thin wrappers that delegate to application services.

```mermaid
sequenceDiagram
participant IPC as "IPC Adapter (commands/agent/mod.rs)"
participant TS as "TurnService"
participant PS as "ProviderService"
participant RI as "RequestIntelligenceService"
participant MC as "MemoryCoordinator"
participant PP as "PromptPlanner"
participant RT as "Runtime"
IPC->>TS : prepare_chat_inputs(request)
TS->>PS : resolve_chat_runtime_provider(workdir)
PS-->>TS : RuntimeProviderResolution
TS->>RI : classify(RequestIntelligenceInput)
RI-->>TS : ExecutionModeDecision
TS->>MC : prepare_context(...)
MC-->>TS : MemoryInjectionArtifacts + memory_items
TS->>PP : build_prompt_plan(BuildPromptPlanRequest)
PP-->>TS : PromptPlanResult
TS-->>IPC : PreparedChatInputs
IPC->>RT : start_agent_stream / run_agent_turn
```

**Updated** Added stream cancellation flow to the architecture diagram.

**Diagram sources**
- [turn_service/mod.rs:242-322](file://src-tauri/src/modules/application/turn_service/mod.rs#L242-L322)
- [provider_service.rs:81-125](file://src-tauri/src/modules/application/provider_service.rs#L81-L125)
- [request_intelligence_service.rs:50-61](file://src-tauri/src/modules/application/request_intelligence_service.rs#L50-L61)
- [memory_coordinator.rs:105-124](file://src-tauri/src/modules/application/memory_coordinator.rs#L105-L124)
- [prompt_planner/mod.rs:210-277](file://src-tauri/src/modules/application/prompt_planner/mod.rs#L210-L277)

## Detailed Component Analysis

### TurnService
TurnService is the first orchestration seam for a single turn. It:
- Resolves the provider client and model.
- Classifies the request to determine execution mode.
- Delegates memory preparation to MemoryCoordinator.
- Builds the prompt plan via PromptPlanner.
- Returns a PreparedChatInputs bundle for the runtime.

```mermaid
classDiagram
class TurnService {
+new(deps : TurnServiceDeps) TurnService
+tool_registry() Arc<ToolRegistry>
+prepare_chat_inputs(req) PreparedChatInputs
}
class TurnServiceDeps {
+tool_registry : Arc<ToolRegistry>
+pinned_store : PinnedStore
+memory_provider : SharedMemoryProvider
+active_retrieval_manager : Option<ActiveRetrievalManager>
+memory_injection_deps() MemoryInjectionDeps
}
class PreparedChatInputs {
+provider : RuntimeProviderResolution
+prompt : PromptPlanResult
+memory_items : Vec<MemoryItemProjection>
+memory_injection : MemoryInjectionArtifacts
+execution_mode_decision : ExecutionModeDecision
}
TurnService --> TurnServiceDeps : "owns"
TurnService --> PreparedChatInputs : "produces"
```

**Diagram sources**
- [turn_service/mod.rs:152-322](file://src-tauri/src/modules/application/turn_service/mod.rs#L152-L322)

**Section sources**
- [turn_service/mod.rs:152-322](file://src-tauri/src/modules/application/turn_service/mod.rs#L152-L322)

### ProviderService
ProviderService resolves a concrete provider client, model id, and request timeout from configuration. It preserves legacy fallback behavior and transport policy semantics.

```mermaid
flowchart TD
Start(["Resolve Provider"]) --> LoadPolicy["Load Provider Transport Policy"]
LoadPolicy --> ResolveModel["Resolve Role Model (chat)"]
ResolveModel --> SwitchProtocol{"Protocol"}
SwitchProtocol --> |"anthropic-messages"| BuildClaw["Build ClawApiClient"]
SwitchProtocol --> |"openai-completions"| BuildOAI["Build OpenAI-Compatible Client"]
SwitchProtocol --> |Other| Error["Return Unsupported Protocol Error"]
BuildClaw --> Output["RuntimeProviderResolution"]
BuildOAI --> Output
Error --> End(["Exit"])
Output --> End
```

**Diagram sources**
- [provider_service.rs:48-125](file://src-tauri/src/modules/application/provider_service.rs#L48-L125)

**Section sources**
- [provider_service.rs:31-125](file://src-tauri/src/modules/application/provider_service.rs#L31-L125)

### PromptPlanner and Prompt Planner Components
PromptPlanner builds a structured PromptPlan with stable block kinds and renders a single text. It delegates sanitization, preflight estimation, and governance to submodules.

```mermaid
classDiagram
class PromptPlanner {
+build_prompt_plan(req) PromptPlanResult
}
class PromptPlan {
+blocks : Vec<PromptBlock>
+join_into_text() String
+block_count() usize
}
class PromptBlock {
+kind : PromptBlockKind
+title : &'static str
+content : String
}
class Sanitizer {
+sanitize_messages_for_provider(msgs) (Vec<InputMessage>, SanitizationStats)
}
class Preflight {
+estimate_messages_char_count(msgs) usize
+estimate_messages_token_count(msgs) usize
+summarize_message_for_budget(msg, max) InputMessage
}
class Governor {
+admit(msgs, limits) (Vec<InputMessage>, RequestPreflightStats)
}
PromptPlanner --> PromptPlan : "produces"
PromptPlanner --> Sanitizer : "uses"
PromptPlanner --> Preflight : "uses"
PromptPlanner --> Governor : "uses"
```

**Diagram sources**
- [prompt_planner/mod.rs:56-373](file://src-tauri/src/modules/application/prompt_planner/mod.rs#L56-L373)
- [prompt_planner/sanitize.rs:49-148](file://src-tauri/src/modules/application/prompt_planner/sanitize.rs#L49-L148)
- [prompt_planner/preflight.rs:10-142](file://src-tauri/src/modules/application/prompt_planner/preflight.rs#L10-L142)
- [prompt_planner/governor.rs:34-172](file://src-tauri/src/modules/application/prompt_planner/governor.rs#L34-L172)

**Section sources**
- [prompt_planner/mod.rs:195-277](file://src-tauri/src/modules/application/prompt_planner/mod.rs#L195-L277)
- [prompt_planner/sanitize.rs:1-172](file://src-tauri/src/modules/application/prompt_planner/sanitize.rs#L1-L172)
- [prompt_planner/preflight.rs:1-142](file://src-tauri/src/modules/application/prompt_planner/preflight.rs#L1-L142)
- [prompt_planner/governor.rs:1-204](file://src-tauri/src/modules/application/prompt_planner/governor.rs#L1-L204)

### MemoryCoordinator
MemoryCoordinator orchestrates per-turn recall and injection and computes post-turn write decisions in a staged pipeline: policy → quality gate → conflict resolution.

```mermaid
flowchart TD
In(["prepare_context"]) --> Assemble["assemble_recall(...)"]
Assemble --> Out1["PrepareContextOutput{artifacts,memory_items,sections,diagnostics}"]
In2(["after_turn"]) --> Policy["Evaluate Write Policy"]
Policy --> Quality["Evaluate Quality Gate"]
Quality --> Conflict["Resolve Conflicts"]
Conflict --> Out2["AfterTurnOutput{decisions,quality,conflicts,notes,policy_version}"]
```

**Diagram sources**
- [memory_coordinator.rs:96-179](file://src-tauri/src/modules/application/memory_coordinator.rs#L96-L179)

**Section sources**
- [memory_coordinator.rs:57-335](file://src-tauri/src/modules/application/memory_coordinator.rs#L57-L335)

### StreamEmitterService
Dispatches a typed after-turn envelope to both frontend and harness transports, carrying decisions, quality, and conflicts.

```mermaid
sequenceDiagram
participant App as "AppHandle"
participant Bus as "EventBus"
participant MC as "MemoryCoordinator"
App->>MC : after_turn(AfterTurnInput)
MC-->>App : AfterTurnOutput
App->>App : Emit MEMORY_AFTER_TURN_EVENT payload
App->>Bus : Emit AgentEvent : MemoryAfterTurn
```

**Diagram sources**
- [stream_emitter_service.rs:52-122](file://src-tauri/src/modules/application/stream_emitter_service.rs#L52-L122)

**Section sources**
- [stream_emitter_service.rs:21-122](file://src-tauri/src/modules/application/stream_emitter_service.rs#L21-L122)

### PermissionService
Bridges the runtime PermissionPrompter trait with Tauri IPC by emitting permission-request events and awaiting user decisions. **Extracted from IPC adapters** into a dedicated application service.

```mermaid
sequenceDiagram
participant Runtime as "Runtime"
participant TP as "TauriPermissionPrompter"
Runtime->>TP : decide(PermissionRequest)
TP->>TP : emit "permission-request" event
TP->>TP : recv_timeout(60s)
TP-->>Runtime : PermissionPromptDecision
```

**Updated** PermissionService is now a dedicated application service that handles permission prompt responses centrally.

**Diagram sources**
- [permission_service.rs:51-94](file://src-tauri/src/modules/application/permission_service.rs#L51-L94)

**Section sources**
- [permission_service.rs:15-251](file://src-tauri/src/modules/application/permission_service.rs#L15-L251)

### StreamCancelService
Centralized service for managing agent stream cancellations across the application. **New component** that handles the lookup-and-fire behavior for stopping in-flight streaming turns.

```mermaid
sequenceDiagram
participant IPC as "IPC Adapter"
participant SCS as "StreamCancelService"
participant Mutex as "Mutex<HashMap>"
participant Sender as "Oneshot Sender"
IPC->>SCS : cancel_stream(stream_id)
SCS->>Mutex : lock()
Mutex-->>SCS : senders
SCS->>Mutex : remove(stream_id)
Mutex-->>SCS : sender
SCS->>Sender : send(())
SCS-->>IPC : Ok(())
```

**New** Added StreamCancelService for centralized cancellation management.

**Diagram sources**
- [stream_cancel_service.rs:30-50](file://src-tauri/src/modules/application/stream_cancel_service.rs#L30-L50)

**Section sources**
- [stream_cancel_service.rs:16-51](file://src-tauri/src/modules/application/stream_cancel_service.rs#L16-L51)

### RealApiClient
Implements the runtime ApiClient over the outbound ProviderClient, converting runtime blocks to provider messages and streaming assistant events.

```mermaid
classDiagram
class RealApiClient {
+new(provider,model,request_timeout,tool_registry) RealApiClient
+stream(request) Vec<AssistantEvent>
}
RealApiClient --> ProviderClient : "uses"
RealApiClient --> ToolRegistry : "uses"
```

**Diagram sources**
- [real_api_client.rs:21-171](file://src-tauri/src/modules/application/real_api_client.rs#L21-L171)

**Section sources**
- [real_api_client.rs:18-171](file://src-tauri/src/modules/application/real_api_client.rs#L18-L171)

### ToolExecutor
Bridges async ToolRegistry to the runtime's ToolExecutor trait and loads control plane switches from configuration.

```mermaid
classDiagram
class ToolRegistryExecutor {
+new_with_context(registry,ctx) ToolRegistryExecutor
+execute_with_trace(name,input,trace_id,request_id) String
}
ToolRegistryExecutor --> ToolRegistry : "uses"
ToolRegistryExecutor --> ToolExecutionBroker : "uses"
```

**Diagram sources**
- [tool_executor.rs:52-143](file://src-tauri/src/modules/application/tool_executor.rs#L52-L143)

**Section sources**
- [tool_executor.rs:14-143](file://src-tauri/src/modules/application/tool_executor.rs#L14-L143)

### ToolHeuristics
Provides heuristics to detect unverified file claims, mutating tool successes, and shell commands likely to mutate files.

```mermaid
flowchart TD
Start(["Analyze Tool Result"]) --> CheckError{"Is error?"}
CheckError --> |Yes| NotMutating["Return false"]
CheckError --> |No| CheckTool{"Is mutating tool?"}
CheckTool --> |Yes| Mutating["Return true"]
CheckTool --> |No| CheckShell{"Is shell-like tool?"}
CheckShell --> |No| NotMutating
CheckShell --> |Yes| InspectCmd["Inspect command heuristically"]
InspectCmd --> Mutating
```

**Diagram sources**
- [tool_heuristics.rs:27-95](file://src-tauri/src/modules/application/tool_heuristics.rs#L27-L95)

**Section sources**
- [tool_heuristics.rs:1-104](file://src-tauri/src/modules/application/tool_heuristics.rs#L1-L104)

### TrajectoryService
Records conversation trajectories for RL training, preferring a shared AppState manager and falling back to a temporary manager.

```mermaid
flowchart TD
Start(["record_trajectory_if_possible"]) --> HasManager{"Shared manager?"}
HasManager --> |Yes| UseManager["manager.record(...)"]
HasManager --> |No| MakeTemp["Create temporary manager"]
MakeTemp --> UseTemp["manager.record(...)"]
UseManager --> End(["Done"])
UseTemp --> End
```

**Diagram sources**
- [trajectory_service.rs:23-57](file://src-tauri/src/modules/application/trajectory_service.rs#L23-L57)

**Section sources**
- [trajectory_service.rs:14-58](file://src-tauri/src/modules/application/trajectory_service.rs#L14-L58)

### RequestIntelligenceService
Runs a deterministic classifier to decide execution mode for a turn.

```mermaid
sequenceDiagram
participant Caller as "Caller"
participant RIS as "RequestIntelligenceService"
Caller->>RIS : classify(RequestIntelligenceInput)
RIS-->>Caller : RequestIntelligenceOutput{decision}
```

**Diagram sources**
- [request_intelligence_service.rs:50-61](file://src-tauri/src/modules/application/request_intelligence_service.rs#L50-L61)

**Section sources**
- [request_intelligence_service.rs:29-61](file://src-tauri/src/modules/application/request_intelligence_service.rs#L29-L61)

### ActivationService
Wraps onboarding-style activation and exposes typed activation snapshots and checks.

```mermaid
classDiagram
class ActivationService {
+new() ActivationService
+license_lifecycle() &LicenseLifecycleService
+current_snapshot() ActivationSnapshot
+validate_preconditions() ActivationChecklist
+run_activation_ceremony() ActivationCeremonyResult
+test_active_provider() TestResult
+complete_activation() ActivationSnapshot
+complete_activation_legacy() ()
}
ActivationService --> LicenseLifecycleService : "uses"
```

**Diagram sources**
- [activation_service.rs:76-254](file://src-tauri/src/modules/application/activation_service.rs#L76-L254)

**Section sources**
- [activation_service.rs:74-254](file://src-tauri/src/modules/application/activation_service.rs#L74-L254)

## IPC Command Adapters
**Updated** IPC command adapters are now thin wrappers that delegate to application-layer services. They handle parameter parsing and dependency injection but contain no business logic.

```mermaid
sequenceDiagram
participant IPC as "commands/agent/mod.rs"
participant TS as "TurnService"
participant PS as "PermissionService"
participant SCS as "StreamCancelService"
IPC->>TS : run_turn / stream_turn
TS-->>IPC : Results
IPC->>PS : respond_permission
PS-->>IPC : Ok(())
IPC->>SCS : stop_agent_stream
SCS-->>IPC : Ok(())
```

**New** Added comprehensive documentation for IPC command adapters showing delegation patterns.

**Diagram sources**
- [commands/agent/mod.rs:90-216](file://src-tauri/src/commands/agent/mod.rs#L90-L216)

**Section sources**
- [commands/agent/mod.rs:1-220](file://src-tauri/src/commands/agent/mod.rs#L1-L220)

## Dependency Analysis
- Coupling: TurnService depends on ProviderService, RequestIntelligenceService, MemoryCoordinator, and PromptPlanner. These dependencies are intentional and stable.
- Cohesion: Each service encapsulates a single responsibility (provider resolution, prompt planning, memory orchestration, etc.).
- External dependencies: Services import runtime, memory, tools, api, and control_plane modules as needed.
- No circular dependencies: The module re-exports and imports are controlled via mod.rs and do not introduce cycles.
- **Updated** PermissionService and StreamCancelService are now central application services that IPC adapters delegate to.

```mermaid
graph LR
TurnService --> ProviderService
TurnService --> RequestIntelligenceService
TurnService --> MemoryCoordinator
TurnService --> PromptPlanner
MemoryCoordinator --> PromptPlanner
StreamEmitterService --> MemoryCoordinator
ToolExecutor --> ControlPlane
RealApiClient --> ProviderClient
PermissionService -.-> IPCAdapters
StreamCancelService -.-> IPCAdapters
```

**Diagram sources**
- [application/mod.rs:45-124](file://src-tauri/src/modules/application/mod.rs#L45-L124)
- [turn_service/mod.rs:35-43](file://src-tauri/src/modules/application/turn_service/mod.rs#L35-L43)
- [memory_coordinator.rs:49-55](file://src-tauri/src/modules/application/memory_coordinator.rs#L49-L55)
- [stream_emitter_service.rs:14-19](file://src-tauri/src/modules/application/stream_emitter_service.rs#L14-L19)
- [tool_executor.rs:10-11](file://src-tauri/src/modules/application/tool_executor.rs#L10-L11)
- [real_api_client.rs:12-16](file://src-tauri/src/modules/application/real_api_client.rs#L12-L16)

**Section sources**
- [application/mod.rs:1-124](file://src-tauri/src/modules/application/mod.rs#L1-L124)

## Performance Considerations
- Blocking in sync contexts: RealApiClient uses block_in_place to call async functions from the sync stream method. Ensure timeouts are configured appropriately to avoid long blocking periods.
- Budgeting and trimming: PromptPlanner's governor trims messages and tokens to fit provider budgets. Keep token/char estimates accurate to minimize retries.
- Memory orchestration: MemoryCoordinator's after_turn pipeline (policy → quality gate → conflict resolution) should be efficient; avoid unnecessary recomputation of classifiers or activation status.
- Permission prompts: PermissionService blocks on an mpsc channel; keep UI responsiveness by minimizing prompt frequency and providing clear user feedback.
- **Updated** Stream cancellation: StreamCancelService uses a mutex-protected HashMap for tracking cancellation senders. Ensure proper cleanup to prevent memory leaks.

## Troubleshooting Guide
- Provider resolution failures: Verify transport policy loading and model resolution. Check for missing API keys or unsupported protocols.
- Prompt planner errors: Validate system prompt loading and ensure memory injection artifacts are present when expected.
- Tool execution errors: Confirm control plane switches and tool registry definitions. Use ToolExecutor's trace id for auditing.
- Permission denials: Review PermissionPrompter decisions and ensure frontend listeners are registered for permission-request events.
- Stream emissions: If MEMORY_AFTER_TURN_EVENT or harness events fail, check logging for non-fatal errors and ensure the bus is subscribed.
- **Updated** Stream cancellation failures: Check for locked mutex errors or missing stream IDs in the StreamCancelService. Verify that the stream_id matches the one registered during turn startup.
- **Updated** Permission response handling: Ensure the respond_permission IPC command is properly registered and that the session_id matches the pending request.

**Section sources**
- [provider_service.rs:48-125](file://src-tauri/src/modules/application/provider_service.rs#L48-L125)
- [prompt_planner/mod.rs:188-194](file://src-tauri/src/modules/application/prompt_planner/mod.rs#L188-L194)
- [tool_executor.rs:118-143](file://src-tauri/src/modules/application/tool_executor.rs#L118-L143)
- [permission_service.rs:43-86](file://src-tauri/src/modules/application/permission_service.rs#L43-86)
- [stream_emitter_service.rs:92-121](file://src-tauri/src/modules/application/stream_emitter_service.rs#L92-L121)
- [stream_cancel_service.rs:24-50](file://src-tauri/src/modules/application/stream_cancel_service.rs#L24-L50)

## Conclusion
The application layer cleanly separates orchestration from IPC and lower-level modules. It introduces typed contracts and stable seams to support future M2+ projections and M4+ governance. The services demonstrate clear responsibilities, predictable dependencies, and robust error handling strategies. Initialization and dependency injection are achieved via constructors and re-exports in mod.rs, enabling modular composition and testability.

**Updated** The recent refactoring successfully extracted permission handling and stream cancellation into dedicated application services, while maintaining thin IPC command adapters. This improves code organization, reduces coupling, and provides better separation of concerns across the application layer.