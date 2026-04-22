# Agent Orchestration

<cite>
**Referenced Files in This Document**
- [DESIGN.md](file://DESIGN.md)
- [agent-loop.md](file://docs/design-docs/agent-loop.md)
- [agent-orchestrator.md](file://docs/design-docs/agent-orchestrator.md)
- [context-compression.md](file://docs/design-docs/context-compression.md)
- [memory-system.md](file://docs/design-docs/memory-system.md)
- [tool-activation.md](file://docs/design-docs/tool-activation.md)
- [tool-system.md](file://docs/design-docs/tool-system.md)
- [llm-routing.md](file://docs/design-docs/llm-routing.md)
- [conversation.rs](file://src-tauri/src/modules/runtime/conversation.rs)
- [session.rs](file://src-tauri/src/modules/runtime/session.rs)
- [registry.rs](file://src-tauri/src/modules/tools/registry.rs)
- [agent.rs](file://src-tauri/src/commands/agent.rs)
- [prompt.rs](file://src-tauri/src/modules/runtime/prompt.rs)
- [mod.rs](file://src-tauri/src/modules/memory/mod.rs)
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
10. [Appendices](#appendices)

## Introduction
This document explains If2Ai’s intelligent agent orchestration framework with a focus on the agent loop, conversation management, context compression, execution mode routing, request intelligence processing, trajectory planning, and self-learning. It synthesizes design documents and code to provide a practical guide for building, operating, and extending autonomous agent behavior across conversational, tool-execution, and memory-driven workflows.

## Project Structure
The agent orchestration spans multiple layers:
- Runtime: agent loop, conversation state, tool execution, and streaming events
- Application: Tauri commands, provider resolution, memory integration, and learning
- Design: agent loop, orchestrator, context compression, memory, tool system, and LLM routing

```mermaid
graph TB
subgraph "Application Layer"
CMD["Tauri Commands<br/>agent.rs"]
PROMPT["System Prompt Builder<br/>prompt.rs"]
MEM["Memory Integration<br/>memory/mod.rs"]
end
subgraph "Runtime Layer"
CONV["Conversation Runtime<br/>conversation.rs"]
SESS["Session Model<br/>session.rs"]
TOOLS["Tool Registry<br/>registry.rs"]
end
subgraph "Design Specs"
LOOP["Agent Loop Spec<br/>agent-loop.md"]
ORCH["Agent Orchestrator Spec<br/>agent-orchestrator.md"]
COMP["Context Compression Spec<br/>context-compression.md"]
TOOL["Tool System Spec<br/>tool-system.md"]
ROUTE["LLM Routing Spec<br/>llm-routing.md"]
MEMSPEC["Memory System Spec<br/>memory-system.md"]
end
CMD --> CONV
CMD --> PROMPT
CMD --> MEM
CONV --> SESS
CONV --> TOOLS
PROMPT --> CONV
MEM --> CONV
LOOP --> CONV
ORCH --> CONV
COMP --> CONV
TOOL --> TOOLS
ROUTE --> CMD
MEMSPEC --> MEM
```

**Diagram sources**
- [agent.rs:160-722](file://src-tauri/src/commands/agent.rs#L160-L722)
- [conversation.rs:182-547](file://src-tauri/src/modules/runtime/conversation.rs#L182-L547)
- [prompt.rs:355-570](file://src-tauri/src/modules/runtime/prompt.rs#L355-L570)
- [registry.rs:354-584](file://src-tauri/src/modules/tools/registry.rs#L354-L584)
- [session.rs:62-161](file://src-tauri/src/modules/runtime/session.rs#L62-L161)
- [agent-loop.md:1-445](file://docs/design-docs/agent-loop.md#L1-L445)
- [agent-orchestrator.md:1-391](file://docs/design-docs/agent-orchestrator.md#L1-L391)
- [context-compression.md:1-449](file://docs/design-docs/context-compression.md#L1-L449)
- [tool-system.md:1-444](file://docs/design-docs/tool-system.md#L1-L444)
- [llm-routing.md:1-462](file://docs/design-docs/llm-routing.md#L1-L462)
- [memory-system.md:1-945](file://docs/design-docs/memory-system.md#L1-L945)

**Section sources**
- [DESIGN.md:1-406](file://DESIGN.md#L1-L406)
- [agent-loop.md:1-445](file://docs/design-docs/agent-loop.md#L1-L445)
- [agent-orchestrator.md:1-391](file://docs/design-docs/agent-orchestrator.md#L1-L391)
- [context-compression.md:1-449](file://docs/design-docs/context-compression.md#L1-L449)
- [memory-system.md:1-945](file://docs/design-docs/memory-system.md#L1-L945)
- [tool-activation.md:1-800](file://docs/design-docs/tool-activation.md#L1-L800)
- [tool-system.md:1-444](file://docs/design-docs/tool-system.md#L1-L444)
- [llm-routing.md:1-462](file://docs/design-docs/llm-routing.md#L1-L462)
- [conversation.rs:1-800](file://src-tauri/src/modules/runtime/conversation.rs#L1-L800)
- [session.rs:1-560](file://src-tauri/src/modules/runtime/session.rs#L1-L560)
- [registry.rs:1-851](file://src-tauri/src/modules/tools/registry.rs#L1-L851)
- [agent.rs:1-800](file://src-tauri/src/commands/agent.rs#L1-L800)
- [prompt.rs:1-800](file://src-tauri/src/modules/runtime/prompt.rs#L1-L800)
- [mod.rs:1-517](file://src-tauri/src/modules/memory/mod.rs#L1-L517)

## Core Components
- Conversation Runtime: orchestrates a single turn, manages messages, streams LLM events, executes tools, and tracks usage
- Session Model: typed message and content-block structures with JSON serialization
- Tool Registry: dynamic registration, validation, and dispatch with timeouts and size caps
- System Prompt Builder: constructs a structured, per-turn system prompt with environment, project, memory, and skills context
- Memory Integration: scoped persistence, recall, and promotion across session/project/global scopes
- LLM Routing: provider detection, selection, fallback, and rate-limiting
- Context Compression: pruning, protection, and summarization to maintain bounded context

**Section sources**
- [conversation.rs:182-547](file://src-tauri/src/modules/runtime/conversation.rs#L182-L547)
- [session.rs:42-370](file://src-tauri/src/modules/runtime/session.rs#L42-L370)
- [registry.rs:354-584](file://src-tauri/src/modules/tools/registry.rs#L354-L584)
- [prompt.rs:355-570](file://src-tauri/src/modules/runtime/prompt.rs#L355-L570)
- [mod.rs:96-380](file://src-tauri/src/modules/memory/mod.rs#L96-L380)
- [llm-routing.md:34-202](file://docs/design-docs/llm-routing.md#L34-L202)
- [context-compression.md:60-158](file://docs/design-docs/context-compression.md#L60-L158)

## Architecture Overview
The runtime integrates tightly with application commands and memory systems. The agent loop is initiated by a Tauri command, which resolves provider and permission context, prepares the prompt, and invokes the ConversationRuntime. The runtime streams assistant events, parses tool calls, executes tools, and updates the session. Memory and learning modules participate asynchronously around the loop.

```mermaid
sequenceDiagram
participant UI as "Frontend"
participant CMD as "Tauri Command<br/>agent.rs"
participant RT as "ConversationRuntime<br/>conversation.rs"
participant API as "LLM Provider"
participant TOOLS as "Tool Registry<br/>registry.rs"
participant MEM as "Memory Provider<br/>memory/mod.rs"
UI->>CMD : "run_agent_turn(session_id, message, permission_mode)"
CMD->>CMD : "resolve session execution context"
CMD->>CMD : "prepare chat inputs (prompt, provider)"
CMD->>RT : "create runtime with working memory"
RT->>API : "stream request (system prompt, messages, tools)"
API-->>RT : "events : text deltas, tool_use, usage, stop"
RT->>TOOLS : "execute tool calls (permission checks)"
TOOLS-->>RT : "tool results"
RT->>MEM : "background hooks (summary, compile, pin)"
RT-->>CMD : "turn summary (assistant messages, tool results, usage)"
CMD->>CMD : "compact session if needed"
CMD-->>UI : "final response (message, thinking)"
```

**Diagram sources**
- [agent.rs:160-722](file://src-tauri/src/commands/agent.rs#L160-L722)
- [conversation.rs:358-515](file://src-tauri/src/modules/runtime/conversation.rs#L358-L515)
- [registry.rs:486-540](file://src-tauri/src/modules/tools/registry.rs#L486-L540)
- [mod.rs:204-380](file://src-tauri/src/modules/memory/mod.rs#L204-L380)

**Section sources**
- [agent.rs:160-722](file://src-tauri/src/commands/agent.rs#L160-L722)
- [conversation.rs:358-515](file://src-tauri/src/modules/runtime/conversation.rs#L358-L515)
- [registry.rs:486-540](file://src-tauri/src/modules/tools/registry.rs#L486-L540)
- [mod.rs:204-380](file://src-tauri/src/modules/memory/mod.rs#L204-L380)

## Detailed Component Analysis

### Agent Loop Implementation
The agent loop encapsulates a single turn of conversation:
- Adds user message to session
- Builds system prompt and message list
- Streams LLM events and builds assistant message
- Parses tool-use blocks and executes tools with permission checks
- Updates session and triggers memory hooks
- Returns summary with assistant messages, tool results, iteration count, and usage

```mermaid
flowchart TD
Start(["Start run_turn"]) --> AddUser["Add user message to session"]
AddUser --> BuildPrompt["Build system prompt and messages"]
BuildPrompt --> StreamLLM["Stream LLM events"]
StreamLLM --> ParseBlocks["Parse tool_use blocks"]
ParseBlocks --> HasTools{"Any tool calls?"}
HasTools -- "No" --> SaveSession["Save session"]
SaveSession --> Return(["Return summary"])
HasTools -- "Yes" --> ExecTools["Execute tools with permissions"]
ExecTools --> UpdateSession["Append tool results"]
UpdateSession --> StreamLLM
```

**Diagram sources**
- [conversation.rs:358-515](file://src-tauri/src/modules/runtime/conversation.rs#L358-L515)

**Section sources**
- [conversation.rs:358-515](file://src-tauri/src/modules/runtime/conversation.rs#L358-L515)
- [agent-loop.md:138-221](file://docs/design-docs/agent-loop.md#L138-L221)

### Conversation Management
The session model defines roles, content blocks, and JSON serialization. The runtime maintains a typed conversation history with usage tracking and optional thinking content.

```mermaid
classDiagram
class ConversationMessage {
+MessageRole role
+Vec~ContentBlock~ blocks
+Option~TokenUsage~ usage
+Option~String~ thinking
+Option~String~ task_outcome
+Option~String~ degraded_reason
+Option~String~ resume_available
+Option~String~ resume_cursor
+Option~String~ request_id
}
class ContentBlock {
<<enumeration>>
Text(text)
ToolUse(id, name, input)
ToolResult(tool_use_id, tool_name, output, is_error)
}
class Session {
+u32 version
+Vec~ConversationMessage~ messages
+save_to_path(path)
+load_from_path(path)
+to_json()
+from_json(value)
}
ConversationMessage --> ContentBlock : "contains"
Session --> ConversationMessage : "stores"
```

**Diagram sources**
- [session.rs:42-370](file://src-tauri/src/modules/runtime/session.rs#L42-L370)

**Section sources**
- [session.rs:42-370](file://src-tauri/src/modules/runtime/session.rs#L42-L370)

### Context Compression Strategies
Context compression trims long histories to maintain model window budgets:
- Prune low-importance messages
- Protect head and tail for grounding
- Summarize middle region with an LLM
- Track compression statistics and effectiveness

```mermaid
flowchart TD
Enter(["Compression Triggered"]) --> Prune["Prune low-importance messages"]
Prune --> Protect["Identify head/tail protection ranges"]
Protect --> Summarize["Summarize middle region"]
Summarize --> Rebuild["Rebuild message list with summary"]
Rebuild --> Exit(["Resume conversation"])
```

**Diagram sources**
- [context-compression.md:111-156](file://docs/design-docs/context-compression.md#L111-L156)

**Section sources**
- [context-compression.md:60-158](file://docs/design-docs/context-compression.md#L60-L158)

### Execution Mode Routing and Request Intelligence
The application layer resolves provider and execution mode decisions per turn:
- Resolves session execution context (workdir, project, permission mode)
- Prepares chat inputs (prompt plan, provider, request timeout)
- Emits advisory execution mode decision for observability
- Applies working memory budget and logs integrity checks

```mermaid
sequenceDiagram
participant CMD as "run_agent_turn"
participant RES as "Execution Context Resolver"
participant PREP as "Prepare Chat Inputs"
participant MODE as "Execution Mode Decision"
CMD->>RES : "resolve_session_execution_context"
RES-->>CMD : "session execution context"
CMD->>PREP : "prepare_chat_inputs"
PREP-->>CMD : "provider, prompt, timeout"
CMD->>MODE : "log execution mode decision"
CMD-->>CMD : "apply working memory budget"
```

**Diagram sources**
- [agent.rs:120-273](file://src-tauri/src/commands/agent.rs#L120-L273)

**Section sources**
- [agent.rs:120-273](file://src-tauri/src/commands/agent.rs#L120-L273)

### Tool Execution Coordination
The tool system supports dynamic registration, validation, timeouts, and multimodal outputs:
- ToolEntry with input schema, timeouts, and handler variants
- Dispatch with shared or session-scoped contexts
- Size caps enforced per modality
- Tool definitions exported for LLM consumption

```mermaid
classDiagram
class ToolEntry {
+String name
+String toolset
+String description
+JsonValue input_schema
+Option~usize~ max_text_bytes
+Option~usize~ max_image_bytes
+Option~u32~ timeout_secs
+bool disabled
+ToolHandler handler
+Option~ToolHandlerMultimodal~ multimodal_handler
}
class ToolRegistry {
+register(entry)
+get(name)
+has(name)
+names_in_toolset(toolset)
+tool_names()
+get_definitions(allowed)
+get_definitions_by_toolsets(toolsets, registry)
+dispatch(name, args)
+dispatch_with_context(name, args, context)
+dispatch_with_context_legacy(name, args, context)
+validate(name, args)
}
ToolRegistry --> ToolEntry : "manages"
```

**Diagram sources**
- [registry.rs:203-351](file://src-tauri/src/modules/tools/registry.rs#L203-L351)

**Section sources**
- [registry.rs:354-584](file://src-tauri/src/modules/tools/registry.rs#L354-L584)
- [tool-system.md:185-235](file://docs/design-docs/tool-system.md#L185-L235)
- [tool-activation.md:175-274](file://docs/design-docs/tool-activation.md#L175-L274)

### Memory Integration and Context Preservation
Memory operates with three-tier scoping (session/project/global) and supports:
- Scoped storage/recall with visibility rules
- Export and promotion/demotion across scopes
- Importance decay and pinned items
- Memory injection into system prompts

```mermaid
classDiagram
class MemoryProvider {
+store(key, content, category)
+recall(query, category, limit)
+delete(key)
+purge_category(category)
+clear_all()
+export(category)
+store_scoped(key, content, category, scope)
+recall_scoped(query, category, limit, scope)
+export_scoped(category, scope)
+promote_scope(key, target_scope)
+demote_scope(key, target_scope)
+apply_importance_decay(lambda, k)
}
class MemoryEntry {
+String key
+String content
+MemoryCategory category
+DateTime~Utc~ created_at
+DateTime~Utc~ updated_at
+f64 importance
+u64 access_count
+f64 trust_score
+Option~String~ session_id
+Option~String~ project_id
}
MemoryProvider --> MemoryEntry : "persists"
```

**Diagram sources**
- [mod.rs:204-380](file://src-tauri/src/modules/memory/mod.rs#L204-L380)

**Section sources**
- [mod.rs:96-380](file://src-tauri/src/modules/memory/mod.rs#L96-L380)
- [memory-system.md:107-183](file://docs/design-docs/memory-system.md#L107-L183)

### Self-Learning, Failure Analysis, and Continuous Improvement
Learning and reflection integrate with the agent loop:
- Records turn outcomes and periodically analyzes sessions
- Updates self-model with learned patterns
- Triggers reflection at intervals and logs insights
- Surfaces memory promotion candidates and applies importance decay

```mermaid
flowchart TD
LoopStart(["After turn"]) --> Record["Record turn outcome"]
Record --> ReflectCheck{"Turn count % interval == 0?"}
ReflectCheck -- "Yes" --> Analyze["Analyze session for insights"]
Analyze --> Update["Update self-model from reflections"]
ReflectCheck -- "No" --> Continue
Update --> Continue
Continue --> Promote["Evaluate memory promotion candidates"]
Promote --> Decay["Apply importance decay"]
Decay --> LoopEnd(["Next turn"])
```

**Diagram sources**
- [agent.rs:491-531](file://src-tauri/src/commands/agent.rs#L491-L531)

**Section sources**
- [agent.rs:491-531](file://src-tauri/src/commands/agent.rs#L491-L531)

### Practical Examples and Decision Patterns
- Conversation pattern: user message → assistant response with tool-use → tool execution → tool result → next assistant turn
- Tool selection: system prompt includes tool definitions; LLM chooses tools based on schema and availability
- Memory context: memory injection sections appended dynamically per turn
- Provider fallback: routing selects primary and falls back on recoverable errors

**Section sources**
- [agent-loop.md:46-61](file://docs/design-docs/agent-loop.md#L46-L61)
- [prompt.rs:496-538](file://src-tauri/src/modules/runtime/prompt.rs#L496-L538)
- [llm-routing.md:135-202](file://docs/design-docs/llm-routing.md#L135-L202)

## Dependency Analysis
The runtime depends on the session model, tool registry, and memory provider. Application commands coordinate provider resolution, prompt preparation, and learning modules.

```mermaid
graph LR
AgentCmd["agent.rs"] --> Conv["conversation.rs"]
AgentCmd --> Prompt["prompt.rs"]
AgentCmd --> Memory["memory/mod.rs"]
Conv --> Session["session.rs"]
Conv --> Tools["registry.rs"]
Prompt --> Conv
Memory --> Conv
```

**Diagram sources**
- [agent.rs:160-722](file://src-tauri/src/commands/agent.rs#L160-L722)
- [conversation.rs:182-547](file://src-tauri/src/modules/runtime/conversation.rs#L182-L547)
- [prompt.rs:355-570](file://src-tauri/src/modules/runtime/prompt.rs#L355-L570)
- [registry.rs:354-584](file://src-tauri/src/modules/tools/registry.rs#L354-L584)
- [session.rs:62-161](file://src-tauri/src/modules/runtime/session.rs#L62-L161)
- [mod.rs:204-380](file://src-tauri/src/modules/memory/mod.rs#L204-L380)

**Section sources**
- [agent.rs:160-722](file://src-tauri/src/commands/agent.rs#L160-L722)
- [conversation.rs:182-547](file://src-tauri/src/modules/runtime/conversation.rs#L182-L547)
- [prompt.rs:355-570](file://src-tauri/src/modules/runtime/prompt.rs#L355-L570)
- [registry.rs:354-584](file://src-tauri/src/modules/tools/registry.rs#L354-L584)
- [session.rs:62-161](file://src-tauri/src/modules/runtime/session.rs#L62-L161)
- [mod.rs:204-380](file://src-tauri/src/modules/memory/mod.rs#L204-L380)

## Performance Considerations
- Streaming LLM responses reduces perceived latency and enables progressive UI updates
- Working memory sliding window constrains tokens sent per request
- Tool timeouts prevent runaway executions
- Context compression reduces token usage and improves throughput
- Memory importance decay and promotion reduce storage overhead

[No sources needed since this section provides general guidance]

## Troubleshooting Guide
Common issues and remedies:
- API errors: friendly messages differentiate DNS, timeouts, auth, rate limits, and service errors
- Permission denied: adjust permission mode or approve tool usage
- Session errors: verify session persistence and budget thresholds
- Tool errors: check tool availability, timeouts, and output size caps
- Prompt integrity: snapshot verification detects unexpected prompt modifications

**Section sources**
- [agent.rs:664-721](file://src-tauri/src/commands/agent.rs#L664-L721)

## Conclusion
If2Ai’s agent orchestration combines a robust runtime loop, structured conversation management, adaptive context compression, flexible tool execution, and integrated memory and learning. The design emphasizes observability, safety, and extensibility, enabling autonomous behavior while maintaining control and reliability.

[No sources needed since this section summarizes without analyzing specific files]

## Appendices

### Customization and Extension Guidelines
- Extend tools: define ToolEntry with input schema and handler; register via ToolRegistry
- Customize prompts: use SystemPromptBuilder to inject environment, project, memory, and skills context
- Integrate providers: add provider configs and leverage routing for fallback and rate limiting
- Tune memory: configure scope visibility, promotion thresholds, and decay parameters
- Optimize loops: adjust working memory budgets, iteration limits, and compaction thresholds

**Section sources**
- [tool-system.md:370-444](file://docs/design-docs/tool-system.md#L370-L444)
- [prompt.rs:355-570](file://src-tauri/src/modules/runtime/prompt.rs#L355-L570)
- [llm-routing.md:34-202](file://docs/design-docs/llm-routing.md#L34-L202)
- [memory-system.md:622-772](file://docs/design-docs/memory-system.md#L622-L772)
- [context-compression.md:397-443](file://docs/design-docs/context-compression.md#L397-L443)