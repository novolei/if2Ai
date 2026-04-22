# Budget Tracking

<cite>
**Referenced Files in This Document**
- [budget.rs](file://src-tauri/src/modules/runtime/budget.rs)
- [usage.rs](file://rust/crates/runtime/src/usage.rs)
- [usage.rs](file://src-tauri/src/modules/runtime/usage.rs)
- [stream_budget.rs](file://src-tauri/src/modules/tts/inference/stream_budget.rs)
- [tokenizer.rs](file://src-tauri/src/modules/tts/text/tokenizer.rs)
- [ADR-004-Token-Budget-Allocation.md](file://docs/design-docs/postCLI/ADR/ADR-004-Token-Budget-Allocation.md)
- [agent-orchestrator.md](file://docs/design-docs/agent-orchestrator.md)
- [lib.rs](file://rust/crates/runtime/src/lib.rs)
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
This document explains the budget tracking and usage monitoring system for token budgets, cost estimation, and resource consumption. It covers:
- Token budget allocation across memory slots (context window)
- Usage tracking for input, output, and cache tokens
- Cost estimation and reporting
- Budget enforcement and usage limits
- Resource optimization via token estimation and adaptive streaming
- Examples of initialization, tracking, and enforcement

## Project Structure
The budget and usage systems span both Rust runtime libraries and Tauri modules:
- Rust runtime crate provides token usage models and cost estimation
- Tauri runtime module manages context window budget allocation and token estimation
- TTS inference module adapts decoding budget based on streaming lead
- Tokenizer module provides deterministic token counting for text

```mermaid
graph TB
subgraph "Rust Runtime"
RU["rust/crates/runtime/src/usage.rs"]
RL["rust/crates/runtime/src/lib.rs"]
end
subgraph "Tauri Runtime"
BR["src-tauri/src/modules/runtime/budget.rs"]
RU2["src-tauri/src/modules/runtime/usage.rs"]
end
subgraph "TTS Inference"
SB["src-tauri/src/modules/tts/inference/stream_budget.rs"]
TK["src-tauri/src/modules/tts/text/tokenizer.rs"]
end
RL --> RU
BR --> TK
RU2 --> RU
SB --> TK
```

**Diagram sources**
- [usage.rs:1-311](file://rust/crates/runtime/src/usage.rs#L1-L311)
- [lib.rs:84-86](file://rust/crates/runtime/src/lib.rs#L84-L86)
- [budget.rs:1-546](file://src-tauri/src/modules/runtime/budget.rs#L1-L546)
- [usage.rs:156-212](file://src-tauri/src/modules/runtime/usage.rs#L156-L212)
- [stream_budget.rs:1-83](file://src-tauri/src/modules/tts/inference/stream_budget.rs#L1-L83)
- [tokenizer.rs:1-199](file://src-tauri/src/modules/tts/text/tokenizer.rs#L1-L199)

**Section sources**
- [usage.rs:1-311](file://rust/crates/runtime/src/usage.rs#L1-L311)
- [lib.rs:84-86](file://rust/crates/runtime/src/lib.rs#L84-L86)
- [budget.rs:1-546](file://src-tauri/src/modules/runtime/budget.rs#L1-L546)
- [usage.rs:156-212](file://src-tauri/src/modules/runtime/usage.rs#L156-L212)
- [stream_budget.rs:1-83](file://src-tauri/src/modules/tts/inference/stream_budget.rs#L1-L83)
- [tokenizer.rs:1-199](file://src-tauri/src/modules/tts/text/tokenizer.rs#L1-L199)

## Core Components
- Context budget allocation and slot management
- Token usage tracking and cost estimation
- Streaming decode budget adaptation
- Token counting for text and memory entries

Key responsibilities:
- Allocate and enforce a total token budget across System, Episodic, Semantic, and Working memory slots
- Track cumulative and per-turn token usage and estimate USD costs
- Provide adaptive streaming budgeting for audio decoding
- Count tokens deterministically for accurate budgeting

**Section sources**
- [budget.rs:106-176](file://src-tauri/src/modules/runtime/budget.rs#L106-L176)
- [budget.rs:264-311](file://src-tauri/src/modules/runtime/budget.rs#L264-L311)
- [usage.rs:29-161](file://rust/crates/runtime/src/usage.rs#L29-L161)
- [usage.rs:163-210](file://rust/crates/runtime/src/usage.rs#L163-L210)
- [stream_budget.rs:18-54](file://src-tauri/src/modules/tts/inference/stream_budget.rs#L18-L54)
- [tokenizer.rs:314-342](file://src-tauri/src/modules/tts/text/tokenizer.rs#L314-L342)

## Architecture Overview
The system integrates three layers:
- Context window budgeting: distributes tokens across memory slots
- Usage tracking: accumulates token usage and estimates cost
- Streaming budgeting: adapts decoding frames based on lead time

```mermaid
graph TB
CB["ContextBudget<br/>Total + Percentages"]
CS["ContextSlots<br/>System/Episodic/Semantic/Working"]
EST["estimate_tokens()<br/>BPE-based counting"]
UT["UsageTracker<br/>cumulative + per-turn"]
CE["cost_for_tokens()<br/>USD estimation"]
SB["StreamBudget<br/>lead seconds → decode frames"]
CB --> CS
EST --> CS
EST --> UT
UT --> CE
SB -. monitors audio timing .-> EST
```

**Diagram sources**
- [budget.rs:106-176](file://src-tauri/src/modules/runtime/budget.rs#L106-L176)
- [budget.rs:264-311](file://src-tauri/src/modules/runtime/budget.rs#L264-L311)
- [budget.rs:313-342](file://src-tauri/src/modules/runtime/budget.rs#L313-L342)
- [usage.rs:29-161](file://rust/crates/runtime/src/usage.rs#L29-L161)
- [usage.rs:154-161](file://rust/crates/runtime/src/usage.rs#L154-L161)
- [stream_budget.rs:18-54](file://src-tauri/src/modules/tts/inference/stream_budget.rs#L18-L54)

## Detailed Component Analysis

### Context Budget Allocation
ContextBudget defines the total token budget and slot percentages. ContextSlots instantiate per-slot budgets and track used tokens. Token estimation uses a BPE tokenizer for accuracy.

```mermaid
classDiagram
class ContextBudget {
+usize total
+f32 system_pct
+f32 episodic_pct
+f32 semantic_pct
+f32 working_pct
+system_tokens() usize
+episodic_tokens() usize
+semantic_tokens() usize
+working_tokens() usize
+validate() Result
}
class ContextSlots {
+Slot system
+Slot episodic
+Slot semantic
+Slot working
+new(budget : ContextBudget) ContextSlots
+available(slot_type) usize
+total_available() usize
+slot_mut(slot_type) Result<&mut Slot,BudgetError>
}
class Slot {
+usize budget
+usize used
+entries : Vec<MemoryEntry>
+new(budget : usize) Slot
+available() usize
+add(entry, token_count : usize) void
+evict(entry_token_fn) void
+evict_lowest_priority(entry_token_fn) void
}
ContextBudget --> ContextSlots : "creates"
ContextSlots --> Slot : "owns"
```

**Diagram sources**
- [budget.rs:106-176](file://src-tauri/src/modules/runtime/budget.rs#L106-L176)
- [budget.rs:264-311](file://src-tauri/src/modules/runtime/budget.rs#L264-L311)
- [budget.rs:46-104](file://src-tauri/src/modules/runtime/budget.rs#L46-L104)

**Section sources**
- [budget.rs:106-176](file://src-tauri/src/modules/runtime/budget.rs#L106-L176)
- [budget.rs:264-311](file://src-tauri/src/modules/runtime/budget.rs#L264-L311)
- [budget.rs:46-104](file://src-tauri/src/modules/runtime/budget.rs#L46-L104)
- [ADR-004-Token-Budget-Allocation.md:49-146](file://docs/design-docs/postCLI/ADR/ADR-004-Token-Budget-Allocation.md#L49-L146)

### Token Usage Tracking and Cost Estimation
TokenUsage aggregates input, output, cache creation, and cache read tokens. UsageTracker records per-turn and cumulative usage. Cost estimation converts tokens to USD using model-specific pricing tiers.

```mermaid
classDiagram
class TokenUsage {
+u32 input_tokens
+u32 output_tokens
+u32 cache_creation_input_tokens
+u32 cache_read_input_tokens
+total_tokens() u32
+estimate_cost_usd() UsageCostEstimate
+estimate_cost_usd_with_pricing(pricing) UsageCostEstimate
+summary_lines(label) Vec~String~
+summary_lines_for_model(label, model) Vec~String~
}
class UsageTracker {
-TokenUsage latest_turn
-TokenUsage cumulative
-u32 turns
+new() UsageTracker
+from_session(session) UsageTracker
+record(usage) void
+current_turn_usage() TokenUsage
+cumulative_usage() TokenUsage
+turns() u32
}
class UsageCostEstimate {
+f64 input_cost_usd
+f64 output_cost_usd
+f64 cache_creation_cost_usd
+f64 cache_read_cost_usd
+total_cost_usd() f64
}
class ModelPricing {
+f64 input_cost_per_million
+f64 output_cost_per_million
+f64 cache_creation_cost_per_million
+f64 cache_read_cost_per_million
+default_sonnet_tier() ModelPricing
}
TokenUsage --> UsageCostEstimate : "estimates"
UsageTracker --> TokenUsage : "records"
UsageCostEstimate --> ModelPricing : "uses"
```

**Diagram sources**
- [usage.rs:29-161](file://rust/crates/runtime/src/usage.rs#L29-L161)
- [usage.rs:163-210](file://rust/crates/runtime/src/usage.rs#L163-L210)

**Section sources**
- [usage.rs:29-161](file://rust/crates/runtime/src/usage.rs#L29-L161)
- [usage.rs:163-210](file://rust/crates/runtime/src/usage.rs#L163-L210)
- [lib.rs:84-86](file://rust/crates/runtime/src/lib.rs#L84-L86)

### Streaming Decode Budget Adaptation
Streaming budget adjusts the number of frames decoded per step based on lead seconds between emitted audio duration and wall-clock time since first audio.

```mermaid
flowchart TD
Start(["Compute lead seconds"]) --> CheckFirst["Has first_audio_at?"]
CheckFirst --> |No| Min["Return budget 1"]
CheckFirst --> |Yes| CalcLead["lead = emitted_sec - elapsed_sec"]
CalcLead --> Branch{"lead threshold"}
Branch --> |< 0.20| B1["budget = 1"]
Branch --> |< 0.55| B2["budget = 2"]
Branch --> |< 1.10| B4["budget = 4"]
Branch --> |else| B8["budget = 8"]
Min --> End(["Done"])
B1 --> End
B2 --> End
B4 --> End
B8 --> End
```

**Diagram sources**
- [stream_budget.rs:18-54](file://src-tauri/src/modules/tts/inference/stream_budget.rs#L18-L54)

**Section sources**
- [stream_budget.rs:1-83](file://src-tauri/src/modules/tts/inference/stream_budget.rs#L1-L83)

### Token Counting for Text and Entries
Token counting uses a BPE tokenizer when available, with a fallback heuristic. Memory entries are counted by combining key and content token counts.

```mermaid
sequenceDiagram
participant Caller as "Caller"
participant Est as "estimate_tokens()"
participant BPE as "tiktoken-rs BPE"
participant Entry as "estimate_entry_tokens()"
Caller->>Est : estimate_tokens(text)
alt BPE available
Est->>BPE : encode_with_special_tokens(text)
BPE-->>Est : token_len
else Fallback
Est-->>Est : text.len()/4 + 1
end
Caller->>Entry : estimate_entry_tokens(entry)
Entry->>Est : estimate_tokens(entry.key)
Entry->>Est : estimate_tokens(entry.content)
Entry-->>Caller : key_tokens + content_tokens
```

**Diagram sources**
- [budget.rs:313-342](file://src-tauri/src/modules/runtime/budget.rs#L313-L342)
- [budget.rs:321-336](file://src-tauri/src/modules/runtime/budget.rs#L321-L336)

**Section sources**
- [budget.rs:313-342](file://src-tauri/src/modules/runtime/budget.rs#L313-L342)

## Dependency Analysis
- Rust runtime exports usage types and functions for external consumers
- Tauri runtime module depends on memory entries and uses BPE tokenization
- Stream budget depends on audio timing and sample rate
- Tokenizer is independent and deterministic

```mermaid
graph LR
RL["rust/crates/runtime/src/lib.rs"] --> RU["usage.rs (rust)"]
RU2["usage.rs (tauri)"] --> RU
BR["budget.rs (tauri)"] --> TK["tokenizer.rs (tauri)"]
SB["stream_budget.rs (tauri)"] --> TK
```

**Diagram sources**
- [lib.rs:84-86](file://rust/crates/runtime/src/lib.rs#L84-L86)
- [usage.rs:1-311](file://rust/crates/runtime/src/usage.rs#L1-L311)
- [usage.rs:156-212](file://src-tauri/src/modules/runtime/usage.rs#L156-L212)
- [budget.rs:1-546](file://src-tauri/src/modules/runtime/budget.rs#L1-L546)
- [tokenizer.rs:1-199](file://src-tauri/src/modules/tts/text/tokenizer.rs#L1-L199)
- [stream_budget.rs:1-83](file://src-tauri/src/modules/tts/inference/stream_budget.rs#L1-L83)

**Section sources**
- [lib.rs:84-86](file://rust/crates/runtime/src/lib.rs#L84-L86)
- [budget.rs:1-546](file://src-tauri/src/modules/runtime/budget.rs#L1-L546)
- [usage.rs:1-311](file://rust/crates/runtime/src/usage.rs#L1-L311)
- [usage.rs:156-212](file://src-tauri/src/modules/runtime/usage.rs#L156-L212)
- [stream_budget.rs:1-83](file://src-tauri/src/modules/tts/inference/stream_budget.rs#L1-L83)
- [tokenizer.rs:1-199](file://src-tauri/src/modules/tts/text/tokenizer.rs#L1-L199)

## Performance Considerations
- BPE tokenization is initialized once and reused; initial load cost is minimal
- Slot eviction removes entries from the front or by lowest priority to maintain budget
- Streaming decode budget scales with lead to balance latency and throughput
- Cost estimation avoids repeated conversions by caching pricing per model

## Troubleshooting Guide
Common issues and resolutions:
- Invalid budget percentages: ensure the sum equals 1.0; otherwise validation fails and falls back to defaults
- Budget exceeded errors: reduce token usage or increase total budget
- Unknown model pricing: cost estimation falls back to default tier with a note in summaries
- Streaming budget anomalies: verify emitted samples and sample rate; lead should be non-negative

**Section sources**
- [budget.rs:168-176](file://src-tauri/src/modules/runtime/budget.rs#L168-L176)
- [budget.rs:221-261](file://src-tauri/src/modules/runtime/budget.rs#L221-L261)
- [usage.rs:115-151](file://rust/crates/runtime/src/usage.rs#L115-L151)
- [stream_budget.rs:18-54](file://src-tauri/src/modules/tts/inference/stream_budget.rs#L18-L54)

## Conclusion
The budget tracking system provides a robust foundation for managing context window allocation, tracking token usage, and estimating costs. It integrates deterministic token counting, adaptive streaming, and clear enforcement boundaries across memory slots. Together with cost reporting and validation, it enables predictable resource control and optimization for both LLM interactions and TTS decoding.