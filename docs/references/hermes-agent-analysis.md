# Hermes-Agent: Complete Technical Analysis
## Comprehensive Guide for Rust/Tauri Reconstruction

**Project**: hermes-agent-main (Nous Research)  
**Analysis Date**: 2026-04-11  
**Purpose**: Enable complete reconstruction in different tech stacks

---

## I. System Architecture Overview

### High-Level Data Flow

```
┌──────────────────────────────────────────────────────────────────────┐
│                          USER INPUT LAYER                            │
│  (CLI, Telegram, Discord, Slack, ACP, Email, etc.)                  │
└───────────────────────────┬──────────────────────────────────────────┘
                            │
                            ▼
┌──────────────────────────────────────────────────────────────────────┐
│                      CONVERSATION MANAGER                            │
│  (Session load, history fetch, memory prefetch)                      │
└───────────────────────────┬──────────────────────────────────────────┘
                            │
                            ▼
┌──────────────────────────────────────────────────────────────────────┐
│                      SYSTEM PROMPT BUILDER                           │
│  (Identity + Memory + Skills + Context files + Guidance)             │
└───────────────────────────┬──────────────────────────────────────────┘
                            │
                            ▼
┌──────────────────────────────────────────────────────────────────────┐
│                      LLM CLIENT ORCHESTRATOR                         │
│  (Provider detection, auth resolution, API mode selection)           │
└───────────────────────────┬──────────────────────────────────────────┘
                            │
                            ▼
┌──────────────────────────────────────────────────────────────────────┐
│                      STREAMING RESPONSE HANDLER                      │
│  (Token counting, budget tracking, thinking/reasoning blocks)        │
└───────────────────────────┬──────────────────────────────────────────┘
                            │
                            ▼
┌──────────────────────────────────────────────────────────────────────┐
│                    TOOL CALL PARSER & DISPATCHER                     │
│  (Schema validation, argument extraction, safety checks)             │
└───────────────────────────┬──────────────────────────────────────────┘
                            │
         ┌──────────────────┼──────────────────┐
         ▼                  ▼                  ▼
    ┌─────────┐        ┌─────────┐        ┌──────────┐
    │ Parallel │        │Sequential│       │Delegation│
    │Execution │        │Execution │       │ (Subagent)
    │(8 workers)        │(Safety)  │       │ (Isolated)
    └─────────┘        └─────────┘       └──────────┘
         │                  │                  │
         └──────────────────┼──────────────────┘
                            │
                            ▼
┌──────────────────────────────────────────────────────────────────────┐
│                      CONTEXT COMPRESSION                             │
│  (Prune → Protect head/tail → Summarize middle)                      │
│  (Triggered at 50% context window)                                   │
└───────────────────────────┬──────────────────────────────────────────┘
                            │
                            ▼
┌──────────────────────────────────────────────────────────────────────┐
│                      SESSION PERSISTENCE                             │
│  (Save to JSONL, SQLite, memory, trajectory)                         │
└──────────────────────────────────────────────────────────────────────┘
```

---

## II. Module Architecture & Responsibilities

### A. Core Agent Orchestration

#### `run_agent.py` — AIAgent Class (3600 lines, central hub)
**Responsibilities:**
- Conversation loop orchestration
- LLM client lifecycle and switching
- Tool execution batching and parallelization
- Budget tracking (iteration, context, token)
- Streaming response handling
- Error recovery and fallback activation

**Key Methods:**
```python
class AIAgent:
    def __init__(self, base_url, api_key, provider, model, ...)
        # Initialize client, context compressor, tools, memory
    
    def run_conversation(self, user_message, conversation_history=None)
        # Main entry point: prefetch, build prompt, stream LLM, tool loop
    
    def _call_llm(self, messages, tools, stream=False)
        # API call with retry, error classification, rate limit tracking
    
    def _execute_tool_batch(self, tool_calls, task_id)
        # Parallel or sequential execution with path conflict detection
    
    def _maybe_compress_context(self, messages)
        # Check threshold and trigger compression if needed
    
    def switch_model(self, new_model, new_provider, api_key, base_url)
        # Live model switching (no conversation reload)
```

**State Management:**
- `iteration_budget`: Shared IterationBudget across parent + children
- `_primary_runtime`: Snapshot of model/provider for per-turn restoration
- `_fallback_chain`: Ordered list [{"provider": "...", "model": "..."}]
- `valid_tool_names`: Filtered tool set for validation
- `_rate_limit_state`: Current rate limit tracking from API headers

#### `model_tools.py` — Tool System Interface
**Responsibilities:**
- Tool discovery (import all tool modules)
- Async/sync bridging with persistent event loops
- Tool dispatch proxy to registry
- Toolset filtering and validation

**Key Functions:**
```python
def get_tool_definitions(enabled_toolsets, disabled_toolsets, quiet_mode)
    # Returns OpenAI-format tool schemas for requested toolsets

def handle_function_call(function_name, function_args, task_id, user_task)
    # Dispatch to registry, return JSON string result

def _run_async(coro)
    # Persistent event loop bridging (no "Event loop is closed")

def check_toolset_requirements()
    # Dict[str, bool]: availability of each toolset
```

---

### B. Tool System (Decomposed from monolithic structure)

#### `tools/registry.py` — Singleton Registry
**Responsibilities:**
- Central tool metadata storage
- Self-registration API (called by each tool module at import)
- Schema retrieval with availability checks
- Dispatch coordination

**Key Class:**
```python
class ToolRegistry:
    def register(name, toolset, schema, handler, check_fn=None, 
                 requires_env=[], is_async=False, description="", emoji="")
        # Called by each tool module at import time
    
    def get_definitions(tool_names, quiet=False) -> List[dict]
        # Return OpenAI-format tool schemas
    
    def dispatch(name, args, **kwargs) -> str
        # Execute handler, catch exceptions, return JSON string
    
    def get_toolset_for_tool(name) -> Optional[str]
    def get_max_result_size(name, default=None) -> int
    def get_all_tool_names() -> List[str]
```

**Tool Entry Format:**
```python
registry.register(
    name="read_file",
    toolset="files",
    schema={
        "name": "read_file",
        "description": "Read contents of a file",
        "parameters": {
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "File path"},
                "offset": {"type": "integer", "description": "Start position"},
                "limit": {"type": "integer", "description": "Max chars"}
            },
            "required": ["path"]
        }
    },
    handler=lambda args: read_file_impl(args["path"], args.get("offset"), args.get("limit")),
    check_fn=None,  # Always available
    is_async=False,
    description="Read file contents with optional offset/limit",
    emoji="📄"
)
```

#### Tool Categories (40+ implementations)

| Category | Tools | Key Features |
|----------|-------|--------------|
| **Web** | web_search, web_extract | API via external providers, link formatting |
| **Files** | read_file, write_file, patch, search_files | Size guards, path validation, redaction |
| **Terminal** | terminal (6 backends), process | Local, Docker, Modal, SSH, Daytona, Singularity |
| **Vision** | vision_analyze, perception | Image URL or base64, multi-model fallback |
| **Browser** | navigate, click, type, screenshot, etc. | Puppeteer/Playwright via providers |
| **Skills** | skills_list, skill_view, skill_manage | YAML parsing, platform filtering |
| **Memory** | memory (read/write/list) | MEMORY.md + USER.md, TUI for editing |
| **Code Exec** | execute_code | Safety: timeout, environment isolation |
| **Delegation** | delegate_task (creates subagent) | Shared budget, interrupt propagation |
| **Other** | clarify, todo, session_search, cronjob, etc. | Utility + platform-specific |

#### `toolsets.py` — Grouping & Filtering
**Pattern:** Dictionary of toolset definitions
```python
TOOLSETS = {
    "web": {
        "description": "Web research and extraction",
        "tools": ["web_search", "web_extract"],
        "includes": []
    },
    "full_stack": {
        "description": "Terminal + files + web + browser",
        "tools": [],
        "includes": ["terminal", "files", "web", "browser"]
    },
    # ... 30+ more
}

def resolve_toolset(toolset_name: str) -> Set[str]
    # Recursively resolve includes, return all tool names
```

---

### C. LLM Integration & Provider Routing

#### `agent/auxiliary_client.py` — Provider Router
**Responsibilities:**
- Auto-detect credentials (env vars, config)
- Select base URL and API mode
- Build OpenAI client with proper headers for each provider

**Supported Providers:**
- **OpenRouter** (default aggregator, 200+ models)
- **OpenAI** (GPT-4, GPT-5 with reasoning)
- **Anthropic** (Claude native API + compatible endpoints)
- **GitHub Copilot** (via api.githubcopilot.com)
- **Kimi/Moonshot** (Chinese LLM)
- **MiniMax** (Anthropic-compatible)
- **Alibaba/DashScope** (Qwen models, Anthropic-compatible)
- **Deepseek** (via OpenRouter or native)
- **Custom endpoints** (any OpenAI-compatible API)

**Detection Logic:**
```python
def resolve_provider_client(provider="auto", model="", raw_codex=False)
    # Env var detection: OPENAI_API_KEY, OPENROUTER_API_KEY, ANTHROPIC_API_KEY, etc.
    # Returns: (client_instance, provider_name_string)
    # raw_codex=True: access to responses.stream() for streaming
```

#### API Modes
1. **chat_completions** (OpenAI-compatible): Standard `/v1/chat/completions`
2. **codex_responses** (OpenAI experimental): `/v1/chat/completions.with_streaming` for reasoning models
3. **anthropic_messages** (Anthropic native): `/messages` endpoint, fully async

#### `agent/anthropic_adapter.py` — Native Anthropic Support
**Responsibilities:**
- Build native Anthropic client (async)
- Message format translation
- Tool call extraction from Anthropic response
- OAuth token detection
- Prompt caching headers

```python
def build_anthropic_client(api_key, base_url=None)
    # Returns: anthropic.Anthropic() instance

def apply_anthropic_cache_control(messages, system_prompt)
    # Inject cache_control breakpoints for prompt caching
    # Strategy: system_and_3 (system + 3 message boundaries)
```

---

### D. Prompt Construction & Context

#### `agent/prompt_builder.py` — Modular Prompt Assembly
**Components (all stateless functions):**

```python
DEFAULT_AGENT_IDENTITY
    # Core persona: helpful, knowledgeable, direct

MEMORY_GUIDANCE
    # Instructions for saving facts to MEMORY.md/USER.md

SKILLS_GUIDANCE
    # How to use available skills

SESSION_SEARCH_GUIDANCE
    # Remind model to search session history when relevant

PLATFORM_HINTS
    # Format hints: CLI linebreaks, Telegram formatting, etc.

TOOL_USE_ENFORCEMENT_GUIDANCE
    # "Always use tools rather than refusing" for reasoning models

def build_system_prompt(
    agent_identity=DEFAULT_AGENT_IDENTITY,
    memory_context="",  # Prefetched from memory provider
    skills_index="",    # Concatenated skill descriptions
    context_files="",   # SOUL.md, AGENTS.md, .cursorrules
    platform_hints=True,
    include_memory_guidance=True,
)
    # Assemble all parts in order

def build_skills_system_prompt()
    # Generate skills section from ~/.hermes/skills/

def build_context_files_prompt()
    # Load SOUL.md, AGENTS.md, .cursorrules with injection scanning

def load_soul_md()
    # Personal file (persona/instructions)
```

**Threat Scanning (probe_builder.py patterns):**
```
Blocked patterns:
  - "ignore previous instructions"
  - "do not tell the user" (deception attempt)
  - "<!--.*override.*-->" (HTML hidden comments)
  - Curl with ${TOKEN} exfil patterns
  - Invisible unicode (U+200B, U+FEFF, etc.)
  
Result: Signal warning, return sanitized text
```

#### `agent/memory_manager.py` — Memory Orchestration
**Responsibilities:**
- Register built-in + at most one external provider
- Prefetch memory at turn start
- Fence recalled context to prevent injection
- Sync after LLM response

```python
class MemoryManager:
    def add_provider(provider: MemoryProvider)
        # Builtin always accepted, max 1 external
    
    def prefetch_all(user_message: str) -> str
        # Returns fenced memory context block
    
    def build_system_prompt() -> str
        # Memory section of system prompt
    
    def sync_all(user_msg, assistant_response)
        # Async background write to providers

def build_memory_context_block(raw_context: str) -> str
    # Wraps in <memory-context> tags
```

#### `agent/skill_utils.py` — Lightweight Skill Metadata
**Responsibilities:**
- Parse YAML frontmatter
- Platform matching
- No dependency on tool registry (safe to import early)

```python
def parse_frontmatter(content: str) -> Tuple[Dict, str]
    # YAML → dict, body → str

def skill_matches_platform(frontmatter: Dict) -> bool
    # Check platforms[] list vs os.name + platform detection

def get_all_skills_dirs() -> List[Path]
    # Builtin + optional + user ~/.hermes/skills/

def iter_skill_index_files()
    # Walk all dirs, yield .md files

def extract_skill_description(content: str) -> str
    # First line or intro paragraph
```

---

### E. Context Management & Compression

#### `agent/context_compressor.py` — Automatic Compression
**Trigger:** When token count reaches 50% of context limit

**Algorithm:**
```
1. Prune old tool results (in-place)
   └─ Find tool result messages > 2 turns old
   └─ Replace content with "[Old tool output cleared...]"
   
2. Estimate tokens, check if still over threshold
   └─ If under threshold after pruning, RETURN (no LLM call)
   
3. Protect head (system + 3 messages)
   └─ Mark as "protected" (do not summarize)
   
4. Protect tail by token budget
   └─ Most recent N messages that sum to tail_token_budget tokens
   
5. Summarize middle turns
   └─ Structured LLM prompt:
        Goal: [extract primary objective]
        Progress: [what was accomplished]
        Decisions: [key choices made]
        Files: [modified files/artifacts]
        Next Steps: [what comes next]
   
6. Replace middle with summary
   └─ Inject [CONTEXT COMPACTION] prefix
   └─ Guard against iterative summary bloat
```

**Key Fields:**
```python
class ContextCompressor:
    context_length: int          # Model's max tokens
    threshold_tokens: int        # 50% of context_length
    compress_at: int            # Trigger threshold
    
    last_prompt_tokens: int     # Previous API call's input
    last_completion_tokens: int # Previous API call's output
    compression_count: int      # Track compressions per session
    
    _previous_summary: str      # Iterative update on next compression
```

**Configuration (config.yaml):**
```yaml
compression:
  enabled: true
  threshold: 0.50         # Compress at 50% of context
  target_ratio: 0.20      # Summary is 20% of compressed content
  summary_model: null     # Use same model, or override
  protect_last_n: 20      # Protect last 20 messages
```

---

### F. Tree-Style State Machines

#### `agent/error_classifier.py` — Failure Classification
**Purpose:** Decide whether to retry, fallback, or give up

```python
class FailoverReason(Enum):
    RATE_LIMIT = "rate_limited"
    OVERLOADED = "server_overloaded"
    INVALID_TOKEN = "auth_invalid"
    CONTEXT_LENGTH = "context_exceeded"
    MODEL_NOT_FOUND = "model_not_found"
    TOOL_ERROR = "tool_execution_error"
    UNKNOWN = "unknown_error"

def classify_api_error(exception, response_code, error_text) -> FailoverReason
    # Inspect error pattern, return enum for decision tree
```

#### `agent/retry_utils.py` — Exponential Backoff
```python
def jittered_backoff(attempt: int, base: float = 1.0, max_wait: float = 60.0) -> float
    # Returns: base * (2 ** attempt) + random jitter, capped at max_wait
```

---

### G. Sessions & Persistence

#### Session Storage Types
1. **JSONL Files** (~/.hermes/sessions/session_*.json)
   ```json
   {
     "session_id": "20260411_192300_a1b2c3",
     "timestamp": "2026-04-11T19:23:00Z",
     "platform": "cli",
     "model": "anthropic/claude-opus-4.6",
     "messages": [
       {"role": "user", "content": "..."},
       {"role": "assistant", "content": "..."},
       {"role": "tool", "content": "{...}", "tool_call_id": "..."}
     ],
     "metadata": {
       "total_tokens": 5234,
       "api_calls": 3,
       "compression_count": 0
     }
   }
   ```

2. **Trajectories** (for RL training, optional)
   ```jsonl
   {"conversation": [...], "timestamp": "...", "model": "..."}
   {"conversation": [...], "timestamp": "...", "model": "..."}
   ```

3. **SQLite** (optional, for gateway)
   - session_id, source, model, user_id, parent_session_id, created_at
   - Enables /usage, /insights, /sessions commands

---

### H. Tool Execution & Parallelization

#### Execution Strategy Decision Tree
```
If len(tool_calls) == 1:
    Execute sequentially

If any tool in ["clarify"]:  # User interaction tools
    Execute sequentially

If path-scoped tools (read_file, write_file, patch):
    Check for overlapping file paths
    If overlapping:
        Execute sequentially
    Else:
        Reserve paths, execute in parallel

If all tools in ["ha_get_state", "web_search", "vision_analyze", ...]:
    Execute in parallel (safe read-only)

Else:
    Execute sequentially (conservatively)
```

#### Parallel Execution
```python
class ThreadPoolExecutor(max_workers=8)
    # Limited to 8 concurrent tool workers
    # Each worker gets its own async event loop
    # Prevents contention and "Event loop is closed" errors
```

#### Budget Enforcement
```python
def enforce_turn_budget(result: str, max_size_chars: int) -> str
    # If len(result) > max_size_chars:
    #   Save to temp file
    #   Return "Result saved to /tmp/xxx (N chars)"
    
Tool result defaults:
  - Most tools: 100K chars
  - Large-output tools (terminal, files): configurable
```

---

## III. Key Data Structures

### Message Format (OpenAI-compatible)
```python
messages = [
    {"role": "system", "content": "You are Hermes Agent..."},
    {"role": "user", "content": "user question here"},
    {"role": "assistant", "content": "...", "tool_calls": [...]},
    {"role": "tool", "content": "tool result", "tool_call_id": "call_..."}
]
```

### Tool Call Format
```python
{
    "id": "call_abc123",
    "type": "function",
    "function": {
        "name": "read_file",
        "arguments": '{"path": "/etc/hosts"}'
    }
}
```

### Tool Definition (OpenAI format)
```python
{
    "type": "function",
    "function": {
        "name": "web_search",
        "description": "Search the web for current information",
        "parameters": {
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "num_results": {
                    "type": "integer",
                    "description": "Number of results (default 10)"
                }
            },
            "required": ["query"]
        }
    }
}
```

### Iteration Budget
```python
class IterationBudget:
    max_total: int     # e.g., 90
    _used: int         # Current count
    
    def consume() -> bool:  # Returns: allowed this iteration?
    def refund():           # Give back one iteration (for execute_code)
    
    @property
    def remaining() -> int  # How many left?
```

### Rate Limit State
```python
@dataclass
class RateLimitState:
    limit: int              # X-RateLimit-Limit
    remaining: int          # X-RateLimit-Remaining
    reset_at: datetime      # When limit resets
    requests_per_minute: int
```

---

## IV. Workflow Execution Model

### Conversation Turn Flow

```
INPUT: user_message, conversation_history (optional)
  │
  ├─→ [Session Loading]
  │    Load or create session from SQLite/file
  │    Append message history
  │
  ├─→ [Memory Prefetch]
  │    memory_manager.prefetch_all(user_message)
  │    Returns: fenced memory context block
  │
  ├─→ [System Prompt Building]
  │    base = DEFAULT_AGENT_IDENTITY
  │    + memory guidance
  │    + skills index
  │    + memory context (prefetched)
  │    + context files (SOUL.md, AGENTS.md)
  │    + platform hints
  │    + budget warnings (if applicable)
  │
  ├─→ [LLM API Call]
  │    messages = [system, ...history, user_message]
  │    response = client.chat.completions.create(
  │        messages=messages,
  │        tools=tools,
  │        stream=True,
  │        temperature=...,
  │        max_tokens=...,
  │        thinking=... (if applicable)
  │    )
  │
  ├─→ [Stream Processing]
  │    FOR chunk IN response:
  │        IF chunk.type == "content_block_start" && is_thinking:
  │            thinking_callback(thinking_text)
  │        ELIF token_delta:
  │            Accumulate text, call stream_delta_callback()
  │        ELIF tool_call:
  │            Parse tool calls from streamed chunks
  │
  ├─→ [Tool Execution Loop]  (repeat while tool_calls present)
  │    1. Validate tool names against registry
  │    2. Decide: parallel or sequential?
  │    3. Execute tools with budget warnings
  │    4. Append tool call + results to messages
  │    5. Call LLM again to continue (stream → tool → stream → ...)
  │    6. Check iteration budget
  │       IF remaining == 0:
  │           Stop iteration loop, return current response
  │
  ├─→ [Context Compression] (if threshold reached)
  │    estimate_tokens(messages) >= threshold?
  │    IF YES:
  │        compressor.compress(messages)
  │        → prune old tool results
  │        → protect head (3) + tail (20 messages)
  │        → summarize middle
  │        → replace middle with [CONTEXT COMPACTION] + summary
  │
  ├─→ [Session Persistence]
  │    Save messages to:
  │        - ~/.hermes/sessions/session_*.json
  │        - SQLite (if available)
  │        - Trajectory JSONL (if enabled)
  │
  ├─→ [Memory Sync]
  │    memory_manager.sync_all(user_message, assistant_response)
  │    → Background write to MEMORY.md, external providers
  │    → May queue nudges for skill creation
  │
  └─→ OUTPUT: assistant_response, token_usage, metadata

INTERRUPTS:
  - set_interrupt() called by signal handler or user cancel
  - Checked in tool_loop
  - Kills active subagents (propagates through _delegate_depth)
  - Returns final response with "interrupted" flag
```

---

## V. Core Abstractions & Base Classes

### Memory Provider Interface
```python
class MemoryProvider:
    """Abstract base for memory backends."""
    
    name: str  # e.g., "builtin", "honcho"
    
    async def prefetch(user_message: str) -> str
        # Return recall context for this turn
    
    async def sync(user_msg: str, assistant_response: str)
        # Background save after LLM response
    
    def get_tool_schemas() -> List[dict]
        # Tools this provider exposes
    
    @classmethod
    def is_available() -> bool
        # Check credentials/requirements
```

### Tool Handler Signature
```python
def tool_handler(args: dict) -> str:
    """All tool handlers:
    - Accept dict of arguments
    - Return JSON string (not dict)
    - Catch all exceptions, return {"error": "..."}
    """
    try:
        result = do_work(args["param1"], args.get("param2"))
        return json.dumps({"success": True, "data": result})
    except Exception as e:
        return json.dumps({"error": str(e)})
```

### Skill Format (Markdown + YAML)
```markdown
---
platforms: [macos, linux]
requires: [web, files]
description: |
  Advanced skill for research
---

# Skill: Research Assistant

This skill helps...

## Usage

You can use this by...
```

---

## VI. Configuration System

### Config File Location
`~/.hermes/config.yaml`

### Major Sections
```yaml
agent:
  model: "anthropic/claude-opus-4.6"
  provider: "openrouter"
  max_iterations: 90
  tool_use_enforcement: "auto"  # or true/false/[substrings]
  enabled_toolsets: null        # or ["web", "terminal"]
  disabled_toolsets: null       # or ["image_gen"]

memory:
  memory_enabled: true
  user_profile_enabled: true
  nudge_interval: 10
  flush_min_turns: 6
  memory_char_limit: 2200
  user_char_limit: 1375
  provider: null                # or "honcho" for external

skills:
  creation_nudge_interval: 10

compression:
  enabled: true
  threshold: 0.50
  target_ratio: 0.20
  summary_model: null
  protect_last_n: 20

model:
  context_length: null          # Auto-detect if null
  ollama_num_ctx: null

custom_providers:
  - base_url: "http://localhost:8000"
    models:
      llama2:
        context_length: 4096
```

---

## VII. Integration Points & External Systems

### LLM Providers (External APIs)
- **OpenRouter**: Default, 200+ models, cost aggregation
- **OpenAI**: Direct GPT-4/5 + reasoning models
- **Anthropic**: Claude models, native API
- **Others**: Custom one-off providers (Kimi, MiniMax, etc.)

### Optional Tool Backends
- **Web Search**: OpenRouter's search, SerpAPI, or fallback
- **Browser**: Browserbase, Firecrawl, BrowserUse
- **Environment**: Docker, Modal, SSH, Daytona, Singularity
- **Vision**: GPT-4 Vision, Claude, Qwen
- **Voice**: ElevenLabs TTS, Deepgram transcription

### Memory Plugins
- **Builtin**: File-based (MEMORY.md, USER.md)
- **Honcho**: External service (dialectic memory, learned user models)

### Messaging Platforms (Gateway)
- **Telegram**: bot via python-telegram-bot
- **Discord**: bot via discord.py
- **Slack**: bolt-python
- **Email**: aiosmtplib
- **WhatsApp/Signal**: via external services

### MCP (Model Context Protocol)
- Dynamic tool discovery from MCP servers
- Tool schemas injected into agent's tool list
- Notification-based server-to-agent updates

---

## VIII. Design Patterns & Architectural Principles

### 1. Registry Pattern (Tools)
- Self-registration at module import time
- No central manifest file needed
- New tools = new file + `registry.register(...)` call
- Enables plugin-style extensibility

### 2. Dependency Injection (Clients)
- Client/API key resolution separated from usage
- Routing layer (`auxiliary_client.py`) handles detection
- Easy to swap providers without code changes

### 3. Composition over Inheritance
- Memory system: multiple providers chained together
- Context: compressor is a separate class
- Skills: metadata parsing is isolated utility

### 4. Persistent Event Loops (Async Bridging)
- Prevents "Event loop is closed" errors with cached clients
- Per-thread loops for parallel execution
- Single shared loop for main CLI thread

### 5. Budget Tracking (Layered)
- **Iteration budget**: Shared across parent + children
- **Context budget**: Compression threshold (50% of model max)
- **Turn budget**: Max chars per tool result
- **Token budget**: Tail protection during compression

### 6. Message Translation
- Internal model: OpenAI message format (role/content/tool_calls)
- Anthropic → OpenAI translation layer
- Codex Responses API wrapper for streaming access

### 7. Graceful Degradation
- Tool errors don't crash agent (caught, returned as tool result)
- Memory provider failure → skip memory (agent continues)
- Context compression failure → truncate (worst case)

---

## IX. Error Handling & Recovery Strategies

### API Error Classification
```
Rate Limited (429, 503)
  → Exponential backoff + retry
  → Consider fallback provider if repeated

Invalid Token (401, 403)
  → Try fallback provider
  → Log auth error
  → Return user-friendly message

Model Not Found (404)
  → Try fallback provider
  → Warn user model doesn't exist

Server Overload (502, 503, 504)
  → Backoff + retry
  → Fallback to next provider

Network Error
  → Retry with backoff
  → Fallback on repeated failures

Context Length Exceeded
  → Trigger compression
  → If still over: fallback to larger-context model
```

### Tool Error Handling
```
All exceptions caught in registry.dispatch()
→ Return JSON: {"error": "Tool execution failed: ..."}
→ Appended to messages as tool result
→ LLM sees error and can retry or escalate
```

### Graceful Shutdown
```
on_interrupt():
  - Set _interrupt_requested flag
  - Propagate to active children (subagents)
  - Kill long-running tool processes
  - Collect partial results
  - Save session/trajectory
  - Return with "interrupted" metadata
```

---

## X. Performance Characteristics

### Token Estimation
- Rough estimate: 1 token ≈ 4 characters
- Used for compression triggers and budget decisions
- Claude-specific: `estimate_messages_tokens_rough()` using OpenRouter metadata

### Concurrency
- Max 8 parallel tool workers (tunable)
- Tools are I/O-bound, not CPU-bound
- Async event loop per thread (prevents event loop conflicts)

### Context Window Management
- Compression triggered at 50% (default, configurable)
- Typical agent: 20–50 iterations before compression needed
- Compression itself: ~5–10 second LLM call (cheap auxiliary model)

### Memory Usage
- ~1–2 MB base + tool modules
- Context compressor: temporary copies during compression
- Session history: stored on disk (JSONL), loaded on demand

---

## XI. Testing & Validation Surfaces

### Unit Test Points
1. Tool registry registration and dispatch
2. Prompt builder composition
3. Context compressor algorithm (pruning → summarizing)
4. Budget tracking (iteration, context, token)
5. Error classification
6. Memory prefetch/sync
7. Skill metadata parsing

### Integration Test Points
1. Full conversation loop (mock LLM)
2. Tool execution (mock tools)
3. Compression trigger + execution
4. Provider fallback chain
5. Session persistence and resume
6. Memory sync background tasks
7. Subagent delegation

### E2E Test Points
1. Real LLM + OpenRouter (with mock tools)
2. Real tools (terminal, files, browser)
3. Full workflow: input → tool loop → compression → output
4. Interruption handling
5. Model switching mid-conversation

---

## XII. Reconstruction Roadmap (Rust/Tauri)

### Phase 1: Core Infrastructure
```
□ AIAgent struct with tokio async
□ Tool registry (lazy-static + trait dispatch)
□ LLM client routing (provider detection, auth)
□ Basic conversation loop
```

### Phase 2: Tool System
```
□ Tool trait + handler registration
□ 10 essential tools (web_search, read_file, write_file, terminal, etc.)
□ Budget tracking (iteration + tokens)
□ Parallel execution with tokio tasks
```

### Phase 3: Context Management
```
□ Prompt builder (compose identity + memory + skills)
□ Context compressor (chunk protection + summarization)
□ Session storage (JSON files + optional SQLite)
□ Memory system (file-based)
```

### Phase 4: Advanced Features
```
□ Model switching (live provider change)
□ Provider fallback chain
□ Prompt caching (Anthropic)
□ Thinking/reasoning blocks
□ Subagent delegation
```

### Phase 5: UI Integration (Tauri)
```
□ Expose run_conversation() as Tauri command
□ Stream results via Tauri events
□ Handle interrupts via Tauri cancellation
□ Session browser UI
□ Memory editor UI
□ Skills browser UI
```

---

## XIII. Database Schema (SQLite)

### Sessions Table
```sql
CREATE TABLE sessions (
    session_id TEXT PRIMARY KEY,
    source TEXT,                    -- "cli", "telegram", "discord", etc.
    model TEXT,
    user_id TEXT,                   -- Optional gateway user
    parent_session_id TEXT,         -- For subagent nesting
    created_at TIMESTAMP,
    modified_at TIMESTAMP,
    messages_count INTEGER,
    tokens_total INTEGER,
    state TEXT                      -- JSON: metadata
);

CREATE TABLE session_messages (
    id INTEGER PRIMARY KEY,
    session_id TEXT,
    msg_index INTEGER,              -- Order in conversation
    role TEXT,                       -- "system", "user", "assistant", "tool"
    content TEXT,
    tool_call_id TEXT,              -- If role=="tool"
    created_at TIMESTAMP
);
```

---

## XIV. Key Insights for Reconstruction

### Must-Have Features
1. **Flexible tool registration** (not hard-coded list)
2. **Async bridging** (avoid "Event loop is closed")
3. **Composition over inheritance** (memory, tools, context)
4. **Budget tracking** (iteration + context + token)
5. **Context compression** (essential for long conversations)
6. **Provider routing** (don't lock to one LLM)
7. **Error classification** (decide: retry vs fallback vs fail)
8. **Session persistence** (resume conversations)

### Nice-to-Have Features
1. Prompt caching (10–20% cost saving on repeating context)
2. Subagent delegation (parallel problem-solving)
3. Memory providers (persistent user modeling)
4. Skill creation (auto-extract procedures)
5. MCP integration (extend tools dynamically)

### Common Pitfalls
1. Hardcoding tool list → use registry pattern
2. Single event loop (main thread) → per-thread loops prevent contention
3. Message format confusion (OpenAI vs Anthropic) → normalize early
4. Context bloat → compress at 50%, not 80%
5. Tool error crashes agent → catch all, return JSON error
6. Budget under-tracking → track iteration, context, AND token budgets
7. Fallback model ignored → activate on rate limit / auth error, restore next turn

---

## Appendix: File Structure Reference

```
hermes-agent-main/
├── run_agent.py                 # AIAgent class (3600 lines)
├── cli.py                        # CLI entry point (TUI with prompt_toolkit)
├── batch_runner.py              # Batch conversation processing
├── model_tools.py               # Tool discovery + dispatch
├── toolsets.py                  # Tool grouping definitions
├── tools/
│   ├── registry.py              # Singleton tool registry
│   ├── file_tools.py            # read_file, write_file, patch, etc.
│   ├── terminal_tool.py         # Command execution (6 backends)
│   ├── web_tools.py             # Web search, extraction
│   ├── browser_tool.py          # Browser automation
│   ├── vision_tools.py          # Image analysis
│   ├── skills_tool.py           # Skill management
│   ├── memory_tool.py           # Memory read/write/list
│   ├── delegate_tool.py         # Subagent creation
│   ├── ...                      # 30+ more tools
│   ├── environments/            # Terminal backends
│   │   ├── local.py
│   │   ├── docker.py
│   │   ├── modal.py
│   │   ├── ssh.py
│   │   ├── daytona.py
│   │   └── singularity.py
│   └── budget_config.py        # Tool result size limits
├── agent/
│   ├── memory_manager.py        # Memory provider orchestration
│   ├── memory_provider.py       # MemoryProvider base class
│   ├── context_compressor.py    # Auto-compression
│   ├── prompt_builder.py        # System prompt assembly
│   ├── skill_utils.py           # Lightweight skill parsing
│   ├── anthropic_adapter.py     # Native Anthropic support
│   ├── auxiliary_client.py      # Provider routing
│   ├── error_classifier.py      # Error type classification
│   ├── retry_utils.py           # Exponential backoff
│   ├── model_metadata.py        # Context length, token estimation
│   ├── rate_limit_tracker.py    # Rate limit state
│   ├── usage_pricing.py         # Token → cost estimation
│   └── display.py               # CLI spinners, formatting
├── acp_adapter/                 # Agent Communication Protocol
│   ├── entry.py
│   ├── server.py
│   ├── tools.py
│   └── ...
├── plugins/                      # Extended functionality
│   └── memory/
│       └── honcho/              # Optional Honcho provider
├── skills/                       # Bundled skills (30+ categories)
│   ├── github/
│   ├── software-development/
│   ├── research/
│   └── ...
├── utils.py                      # atomic_json_write, etc.
├── hermes_constants.py          # HERMES_HOME, get_hermes_dir()
└── README.md                     # Setup + usage documentation
```

---

**Document Version**: 1.0  
**Last Updated**: 2026-04-11  
**Analysis Scope**: Complete hermes-agent-main codebase  
**Intended Use**: Rust/Tauri reconstruction reference
