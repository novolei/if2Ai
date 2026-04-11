# Hermes-Agent: Code Patterns & Implementation Reference
## Specific Patterns for Rust/Tauri Reconstruction

---

## I. Tool Registry Pattern (Python → Rust Migration)

### Python Pattern (Current)
```python
# tools/registry.py
class ToolRegistry:
    def __init__(self):
        self._tools: Dict[str, ToolEntry] = {}
    
    def register(self, name, toolset, schema, handler, check_fn=None, ...):
        self._tools[name] = ToolEntry(
            name=name,
            toolset=toolset,
            schema=schema,
            handler=handler,
            check_fn=check_fn,
            ...
        )

registry = ToolRegistry()

# Each tool file: tools/web_tools.py
registry.register(
    name="web_search",
    toolset="web",
    schema={...},
    handler=web_search_impl,
    is_async=False,
    description="Search the web",
    emoji="🔍"
)

# Dispatch: model_tools.py
def handle_function_call(function_name, function_args, task_id, user_task):
    return registry.dispatch(function_name, function_args)
```

### Rust Pattern (Proposed)
```rust
use lazy_static::lazy_static;
use std::collections::HashMap;
use async_trait::async_trait;

// Type alias for tool handlers
pub type ToolHandler = fn(serde_json::Value) -> Result<String, String>;

pub struct ToolEntry {
    pub name: String,
    pub toolset: String,
    pub schema: serde_json::Value,
    pub handler: ToolHandler,
    pub check_fn: Option<fn() -> bool>,
    pub is_async: bool,
    pub description: String,
    pub emoji: String,
}

pub struct ToolRegistry {
    tools: HashMap<String, ToolEntry>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }
    
    pub fn register(&mut self, entry: ToolEntry) {
        self.tools.insert(entry.name.clone(), entry);
    }
    
    pub async fn dispatch(&self, name: &str, args: serde_json::Value) -> Result<String, String> {
        let entry = self.tools.get(name)
            .ok_or_else(|| format!("Unknown tool: {}", name))?;
        
        (entry.handler)(args)
    }
    
    pub fn get_definitions(&self, toolset: &str) -> Vec<serde_json::Value> {
        self.tools.values()
            .filter(|t| t.toolset == toolset)
            .map(|t| t.schema.clone())
            .collect()
    }
}

// Global registry (lazy-loaded)
lazy_static! {
    pub static ref REGISTRY: std::sync::Mutex<ToolRegistry> = {
        let mut registry = ToolRegistry::new();
        // Register all tools here or via macro
        registry
    };
}

// Tool registration macro for convenience
macro_rules! register_tool {
    ($name:expr, $toolset:expr, $schema:expr, $handler:expr) => {{
        REGISTRY.lock().unwrap().register(ToolEntry {
            name: $name.to_string(),
            toolset: $toolset.to_string(),
            schema: $schema,
            handler: $handler,
            check_fn: None,
            is_async: false,
            description: String::new(),
            emoji: String::new(),
        });
    }};
}
```

---

## II. Async Bridging Pattern

### Python Pattern (Persistent Event Loops)
```python
import asyncio
import threading

_tool_loop = None
_tool_loop_lock = threading.Lock()
_worker_thread_local = threading.local()

def _get_tool_loop():
    """Return a long-lived event loop for running async tool handlers."""
    global _tool_loop
    with _tool_loop_lock:
        if _tool_loop is None or _tool_loop.is_closed():
            _tool_loop = asyncio.new_event_loop()
        return _tool_loop

def _get_worker_loop():
    """Return a persistent event loop for the current worker thread."""
    loop = getattr(_worker_thread_local, 'loop', None)
    if loop is None or loop.is_closed():
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        _worker_thread_local.loop = loop
    return loop

def _run_async(coro):
    """Run an async coroutine from a sync context."""
    try:
        loop = asyncio.get_running_loop()
    except RuntimeError:
        loop = None

    if loop and loop.is_running():
        # Inside an async context — run in a fresh thread
        import concurrent.futures
        with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
            future = pool.submit(asyncio.run, coro)
            return future.result(timeout=300)

    # If we're on a worker thread, use a per-thread persistent loop
    if threading.current_thread() is not threading.main_thread():
        worker_loop = _get_worker_loop()
        return worker_loop.run_until_complete(coro)

    # Main thread: use shared loop
    tool_loop = _get_tool_loop()
    return tool_loop.run_until_complete(coro)
```

### Rust Pattern (Tokio Runtime)
```rust
use tokio::runtime::{Runtime, Builder};
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;

// Global tokio runtime for tool execution
static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    Builder::new_multi_thread()
        .worker_threads(8)
        .thread_name("tool-worker")
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime")
});

// For spawning tasks on the main runtime
pub fn run_async<F>(future: F) -> F::Output
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    RUNTIME.block_on(future)
}

// For internal tool spawning
pub async fn execute_tool_async<F>(future: F) -> F::Output
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    future.await
}

// Example async tool
pub async fn web_search_async(query: String) -> Result<String, String> {
    // Implementation
    Ok(format!("Results for: {}", query))
}

// Sync wrapper exposed to registry
fn web_search_sync(args: serde_json::Value) -> Result<String, String> {
    let query = args["query"].as_str().ok_or("Missing query")?;
    run_async(web_search_async(query.to_string()))
}
```

---

## III. LLM Client Management Pattern

### Python Pattern (Provider Routing)
```python
# agent/auxiliary_client.py
from openai import OpenAI, AsyncOpenAI
import anthropic

def resolve_provider_client(provider="auto", model="", raw_codex=False):
    """Auto-detect credentials and build appropriate client."""
    
    # 1. Determine provider from env or config
    if provider == "auto":
        # Try env vars in order
        if os.getenv("OPENAI_API_KEY"):
            provider = "openai"
        elif os.getenv("OPENROUTER_API_KEY"):
            provider = "openrouter"
        elif os.getenv("ANTHROPIC_API_KEY"):
            provider = "anthropic"
        else:
            provider = "openrouter"  # Default fallback
    
    # 2. Get base URL and API key for provider
    auth = _resolve_provider_auth(provider)
    base_url = auth["base_url"]
    api_key = auth["api_key"]
    
    # 3. Build client with provider-specific headers
    client_kwargs = {
        "api_key": api_key,
        "base_url": base_url,
    }
    
    # Add provider-specific headers
    if provider == "openrouter":
        client_kwargs["default_headers"] = {
            "HTTP-Referer": "https://hermes-agent.nousresearch.com",
            "X-OpenRouter-Title": "Hermes Agent",
        }
    elif provider == "anthropic":
        return anthropic.Anthropic(api_key=api_key, base_url=base_url or None)
    
    # 4. Return OpenAI-compatible client
    return OpenAI(**client_kwargs), provider

def _resolve_provider_auth(provider):
    """Get base URL and API key for a provider."""
    PROVIDERS = {
        "openai": {
            "base_url": "https://api.openai.com/v1",
            "key_env": "OPENAI_API_KEY",
        },
        "openrouter": {
            "base_url": "https://openrouter.ai/api/v1",
            "key_env": "OPENROUTER_API_KEY",
        },
        "anthropic": {
            "base_url": "https://api.anthropic.com",
            "key_env": "ANTHROPIC_API_KEY",
        },
        # ... more providers
    }
    
    config = PROVIDERS.get(provider, {})
    return {
        "base_url": config.get("base_url"),
        "api_key": os.getenv(config.get("key_env", ""))
    }
```

### Rust Pattern (Provider Router)
```rust
use reqwest::Client;
use serde::Serialize;

pub enum Provider {
    OpenAI,
    OpenRouter,
    Anthropic,
    Custom(String),
}

pub struct LLMClient {
    client: Client,
    base_url: String,
    api_key: String,
    provider: Provider,
    model: String,
}

impl LLMClient {
    pub fn new(provider: Option<&str>, model: &str) -> Result<Self, String> {
        let (provider, base_url, api_key) = Self::resolve_auth(provider)?;
        
        let client = Client::new();
        
        Ok(Self {
            client,
            base_url,
            api_key,
            provider,
            model: model.to_string(),
        })
    }
    
    fn resolve_auth(provider: Option<&str>) -> Result<(Provider, String, String), String> {
        let provider = provider.unwrap_or("auto");
        
        // Try to auto-detect from env vars
        let (effective_provider, key_env, default_url) = if provider == "auto" {
            if std::env::var("OPENAI_API_KEY").is_ok() {
                (Provider::OpenAI, "OPENAI_API_KEY", "https://api.openai.com/v1")
            } else if std::env::var("OPENROUTER_API_KEY").is_ok() {
                (Provider::OpenRouter, "OPENROUTER_API_KEY", "https://openrouter.ai/api/v1")
            } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
                (Provider::Anthropic, "ANTHROPIC_API_KEY", "https://api.anthropic.com")
            } else {
                (Provider::OpenRouter, "OPENROUTER_API_KEY", "https://openrouter.ai/api/v1")
            }
        } else {
            match provider {
                "openai" => (Provider::OpenAI, "OPENAI_API_KEY", "https://api.openai.com/v1"),
                "openrouter" => (Provider::OpenRouter, "OPENROUTER_API_KEY", "https://openrouter.ai/api/v1"),
                "anthropic" => (Provider::Anthropic, "ANTHROPIC_API_KEY", "https://api.anthropic.com"),
                custom => (Provider::Custom(custom.to_string()), "CUSTOM_API_KEY", "http://localhost:8000"),
            }
        };
        
        let api_key = std::env::var(key_env)
            .map_err(|_| format!("API key not found for provider: {}", key_env))?;
        
        Ok((effective_provider, default_url.to_string(), api_key))
    }
    
    pub async fn chat_completions(
        &self,
        messages: Vec<serde_json::Value>,
        tools: Option<Vec<serde_json::Value>>,
        stream: bool,
    ) -> Result<String, String> {
        let url = format!("{}/chat/completions", self.base_url);
        
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0.7,
        });
        
        if let Some(tools) = tools {
            body["tools"] = serde_json::json!(tools);
        }
        
        if stream {
            body["stream"] = serde_json::json!(true);
        }
        
        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;
        
        response.text().await.map_err(|e| e.to_string())
    }
}
```

---

## IV. Conversation Loop Pattern

### Python Pattern (Main Orchestration)
```python
class AIAgent:
    def run_conversation(self, user_message, conversation_history=None):
        """Main conversation loop."""
        
        # 1. Initialize/load session
        if conversation_history is None:
            messages = []
        else:
            messages = list(conversation_history)
        
        # 2. Prefetch memory and build system prompt
        memory_context = self._memory_manager.prefetch_all(user_message)
        system_prompt = self._build_system_prompt(memory_context=memory_context)
        
        # 3. Append user message
        messages.append({"role": "user", "content": user_message})
        
        # 4. Tool execution loop
        iteration_count = 0
        while iteration_count < self.max_iterations:
            # Check budget
            if not self.iteration_budget.consume():
                break
            iteration_count += 1
            
            # 5. Stream LLM response
            response = self._call_llm(
                messages=messages,
                tools=self.tools,
                system_prompt=system_prompt,
                stream=True,
            )
            
            # Accumulate response
            full_response = ""
            tool_calls = []
            
            for chunk in response:
                if chunk.type == "content_block":
                    full_response += chunk.delta.text
                elif chunk.type == "tool_use":
                    tool_calls.append({
                        "id": chunk.id,
                        "type": "function",
                        "function": {
                            "name": chunk.name,
                            "arguments": chunk.input,
                        }
                    })
            
            # Append assistant response to messages
            msg = {"role": "assistant", "content": full_response}
            if tool_calls:
                msg["tool_calls"] = tool_calls
            messages.append(msg)
            
            # If no tool calls, we're done
            if not tool_calls:
                break
            
            # 6. Execute tools
            for tool_call in tool_calls:
                result = self._execute_tool(tool_call)
                messages.append({
                    "role": "tool",
                    "content": result,
                    "tool_call_id": tool_call["id"],
                })
            
            # 7. Check if context compression needed
            if self.compression_enabled:
                self._maybe_compress_context(messages)
        
        # 8. Save session
        self._save_session(messages)
        
        # 9. Sync memory
        self._memory_manager.sync_all(user_message, full_response)
        
        return full_response
    
    def _call_llm(self, messages, tools, system_prompt, stream):
        """Call LLM API with retry logic."""
        attempt = 0
        max_attempts = 3
        
        while attempt < max_attempts:
            try:
                response = self.client.chat.completions.create(
                    model=self.model,
                    messages=[
                        {"role": "system", "content": system_prompt},
                        *messages
                    ],
                    tools=tools,
                    stream=stream,
                    temperature=0.7,
                )
                return response
            except Exception as e:
                attempt += 1
                reason = classify_api_error(e)
                
                if reason == FailoverReason.RATE_LIMIT and self._fallback_chain:
                    # Switch to fallback
                    self._try_activate_fallback()
                else:
                    # Retry with exponential backoff
                    wait = jittered_backoff(attempt)
                    time.sleep(wait)
        
        raise RuntimeError("LLM API call failed after retries")
    
    def _execute_tool(self, tool_call):
        """Execute a single tool."""
        name = tool_call["function"]["name"]
        args = json.loads(tool_call["function"]["arguments"])
        
        try:
            result = handle_function_call(name, args, self.session_id, None)
            return result
        except Exception as e:
            return json.dumps({"error": str(e)})
    
    def _maybe_compress_context(self, messages):
        """Compress if over threshold."""
        estimated_tokens = estimate_messages_tokens_rough(messages)
        
        if estimated_tokens >= self.context_compressor.threshold_tokens:
            compressed = self.context_compressor.compress(messages)
            # Replace messages in-place
            messages.clear()
            messages.extend(compressed)
```

### Rust Pattern (Tokio-based)
```rust
use tokio::sync::mpsc;
use futures::stream::StreamExt;

pub struct AIAgent {
    client: LLMClient,
    tools: Vec<ToolEntry>,
    iteration_budget: Arc<Mutex<IterationBudget>>,
    context_compressor: ContextCompressor,
}

impl AIAgent {
    pub async fn run_conversation(
        &mut self,
        user_message: String,
        history: Option<Vec<Message>>,
    ) -> Result<String, String> {
        // 1. Initialize history
        let mut messages = history.unwrap_or_default();
        
        // 2. Prefetch memory
        let memory_context = self.memory_manager.prefetch(&user_message).await;
        
        // 3. Build system prompt
        let system_prompt = self.build_system_prompt(&memory_context);
        
        // 4. Append user message
        messages.push(Message {
            role: "user".to_string(),
            content: user_message.clone(),
            tool_calls: None,
        });
        
        // 5. Tool execution loop
        let mut iteration_count = 0;
        loop {
            if iteration_count >= 90 {
                break;
            }
            
            // Check budget
            {
                let mut budget = self.iteration_budget.lock().unwrap();
                if !budget.consume() {
                    break;
                }
            }
            iteration_count += 1;
            
            // 6. Call LLM
            let llm_response = self.call_llm_with_retry(
                &messages,
                &system_prompt,
            ).await?;
            
            // 7. Parse response
            let (full_response, tool_calls) = Self::parse_llm_response(&llm_response);
            
            // 8. Append assistant response
            messages.push(Message {
                role: "assistant".to_string(),
                content: full_response.clone(),
                tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls.clone()) },
            });
            
            // If no tool calls, we're done
            if tool_calls.is_empty() {
                break;
            }
            
            // 9. Execute tools in parallel
            let mut join_handles = vec![];
            for tool_call in tool_calls {
                let tool_registry = self.tool_registry.clone();
                let handle = tokio::spawn(async move {
                    tool_registry.dispatch(&tool_call.name, tool_call.args).await
                });
                join_handles.push((tool_call.id, handle));
            }
            
            // Collect results
            for (tool_call_id, handle) in join_handles {
                let result = handle.await.map_err(|e| e.to_string())??;
                messages.push(Message {
                    role: "tool".to_string(),
                    content: result,
                    tool_calls: None,
                });
            }
            
            // 10. Check compression
            if self.context_compressor.should_compress(&messages) {
                self.context_compressor.compress(&mut messages).await?;
            }
        }
        
        // 11. Save session
        self.save_session(&messages).await?;
        
        // 12. Sync memory
        self.memory_manager.sync(&user_message, &full_response).await;
        
        Ok(full_response)
    }
    
    async fn call_llm_with_retry(
        &mut self,
        messages: &[Message],
        system_prompt: &str,
    ) -> Result<String, String> {
        let mut attempt = 0;
        
        loop {
            match self.client.chat_completions(
                self.format_messages(messages, system_prompt),
                Some(self.format_tools()),
                true, // stream
            ).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    attempt += 1;
                    if attempt >= 3 {
                        return Err(format!("LLM call failed: {}", e));
                    }
                    
                    let wait_ms = 100 * (2_u64.pow(attempt as u32));
                    tokio::time::sleep(
                        std::time::Duration::from_millis(wait_ms)
                    ).await;
                }
            }
        }
    }
}
```

---

## V. Context Compression Algorithm

### Python Pattern (Exact Algorithm)
```python
class ContextCompressor:
    def compress(self, messages):
        """Compress conversation when over threshold."""
        
        # 1. Prune old tool results
        messages = self._prune_old_tool_results(messages)
        
        # 2. Re-estimate tokens
        estimated = estimate_messages_tokens_rough(messages)
        if estimated < self.threshold_tokens:
            return messages  # No more compression needed
        
        # 3. Identify which messages to protect
        protected_indices = set()
        
        # Protect head (first 3 messages)
        for i in range(min(3, len(messages))):
            protected_indices.add(i)
        
        # Protect tail by token budget
        tail_tokens = 0
        tail_threshold = int(self.context_length * 0.2)  # 20%
        
        for i in range(len(messages) - 1, -1, -1):
            if i not in protected_indices:
                msg_tokens = estimate_message_tokens(messages[i])
                if tail_tokens + msg_tokens > tail_threshold:
                    break
                tail_tokens += msg_tokens
                protected_indices.add(i)
        
        # 4. Middle messages = those not protected
        middle_indices = [i for i in range(len(messages)) 
                         if i not in protected_indices]
        
        # 5. Summarize middle
        middle_messages = [messages[i] for i in middle_indices]
        
        summary_prompt = f"""
Summarize the following conversation turns. 
Extract the key information in this format:

Goal: [Primary objective being worked on]
Progress: [What has been accomplished so far]
Decisions: [Key decisions made]
Files: [Files modified or created]
Next Steps: [What should be done next]

Conversation:
{self._format_for_summary(middle_messages)}
"""
        
        summary = self._call_summary_model(summary_prompt)
        
        # 6. Replace middle with summary
        result = []
        for i in range(len(messages)):
            if i in protected_indices:
                result.append(messages[i])
            elif i == middle_indices[0]:  # First middle message
                result.append({
                    "role": "assistant",
                    "content": f"[CONTEXT COMPACTION] {summary}"
                })
        
        return result
    
    def _prune_old_tool_results(self, messages):
        """Remove oversized tool results from 2+ turns ago."""
        result = []
        
        for i, msg in enumerate(messages):
            if msg["role"] != "tool":
                result.append(msg)
                continue
            
            # How old is this tool result?
            age = len(messages) - i - 1
            
            if age >= 2 and len(msg.get("content", "")) > 5000:
                # Prune it
                msg_copy = msg.copy()
                msg_copy["content"] = "[Old tool output cleared to save context space]"
                result.append(msg_copy)
            else:
                result.append(msg)
        
        return result
```

### Rust Pattern (Equivalent)
```rust
pub struct ContextCompressor {
    context_length: usize,
    threshold_percent: f32,
}

impl ContextCompressor {
    pub async fn compress(&self, messages: &mut Vec<Message>) -> Result<(), String> {
        // 1. Prune old tool results
        self.prune_old_tool_results(messages);
        
        // 2. Re-estimate
        let estimated = self.estimate_tokens(messages);
        if estimated < (self.context_length as f32 * self.threshold_percent) as usize {
            return Ok(());
        }
        
        // 3. Identify protected messages
        let mut protected = std::collections::HashSet::new();
        
        // Head protection
        for i in 0..std::cmp::min(3, messages.len()) {
            protected.insert(i);
        }
        
        // Tail protection by token budget
        let tail_budget = self.context_length / 5; // 20%
        let mut tail_tokens = 0;
        
        for i in (0..messages.len()).rev() {
            if protected.contains(&i) {
                continue;
            }
            
            let msg_tokens = self.estimate_message_tokens(&messages[i]);
            if tail_tokens + msg_tokens > tail_budget {
                break;
            }
            
            tail_tokens += msg_tokens;
            protected.insert(i);
        }
        
        // 4. Get middle indices
        let middle: Vec<usize> = (0..messages.len())
            .filter(|i| !protected.contains(i))
            .collect();
        
        if middle.is_empty() {
            return Ok(());
        }
        
        // 5. Summarize middle
        let middle_messages: Vec<_> = middle.iter()
            .map(|&i| messages[i].clone())
            .collect();
        
        let summary = self.summarize_messages(middle_messages).await?;
        
        // 6. Rebuild messages
        let mut result = Vec::new();
        for i in 0..messages.len() {
            if protected.contains(&i) {
                result.push(messages[i].clone());
            } else if i == middle[0] {
                result.push(Message {
                    role: "assistant".to_string(),
                    content: format!("[CONTEXT COMPACTION] {}", summary),
                    tool_calls: None,
                });
            }
        }
        
        *messages = result;
        Ok(())
    }
    
    fn prune_old_tool_results(&self, messages: &mut Vec<Message>) {
        for i in 0..messages.len() {
            if messages[i].role == "tool" {
                let age = messages.len() - i - 1;
                if age >= 2 && messages[i].content.len() > 5000 {
                    messages[i].content = 
                        "[Old tool output cleared to save context space]".to_string();
                }
            }
        }
    }
    
    async fn summarize_messages(&self, messages: Vec<Message>) -> Result<String, String> {
        let prompt = format!(
            "Summarize in:\nGoal:\nProgress:\nDecisions:\nFiles:\nNext Steps:\n\n{}",
            self.format_for_summary(&messages)
        );
        
        // Call LLM summarizer
        // Implementation: call auxiliary client
        Ok("Summary text here".to_string())
    }
}
```

---

## VI. Memory Integration Pattern

### Python Pattern (Dual-Provider System)
```python
# agent/memory_manager.py
class MemoryManager:
    def __init__(self):
        self._providers: List[MemoryProvider] = []
        self._tool_to_provider: Dict[str, MemoryProvider] = {}
        self._has_external = False
    
    def add_provider(self, provider: MemoryProvider):
        """Add builtin (always first) or at most one external."""
        if provider.name != "builtin" and self._has_external:
            logger.warning("Only one external provider allowed, ignoring %s", provider.name)
            return
        
        self._providers.append(provider)
        
        # Register provider's tools
        for schema in provider.get_tool_schemas():
            tool_name = schema.get("name")
            self._tool_to_provider[tool_name] = provider
        
        if provider.name != "builtin":
            self._has_external = True
    
    def prefetch_all(self, user_message: str) -> str:
        """Prefetch context from all providers."""
        context_blocks = []
        
        for provider in self._providers:
            try:
                context = provider.prefetch(user_message)
                if context:
                    context_blocks.append(context)
            except Exception as e:
                logger.warning("Prefetch from %s failed: %s", provider.name, e)
        
        # Fence and return combined
        combined = "\n".join(context_blocks)
        return build_memory_context_block(combined)
    
    def sync_all(self, user_msg: str, assistant_response: str):
        """Background sync to all providers."""
        for provider in self._providers:
            try:
                # Fire-and-forget background sync
                asyncio.create_task(
                    provider.sync(user_msg, assistant_response)
                )
            except Exception as e:
                logger.warning("Sync to %s failed: %s", provider.name, e)

# tools/memory_tool.py - builtin provider
class BuiltinMemoryProvider(MemoryProvider):
    def __init__(self, memory_dir: Path):
        self.name = "builtin"
        self.memory_file = memory_dir / "MEMORY.md"
        self.user_file = memory_dir / "USER.md"
    
    def prefetch(self, user_message: str) -> str:
        """Load latest memory entries."""
        memory = ""
        if self.memory_file.exists():
            memory = self.memory_file.read_text()
        
        user_profile = ""
        if self.user_file.exists():
            user_profile = self.user_file.read_text()
        
        return f"Memory:\n{memory}\n\nUser Profile:\n{user_profile}"
    
    async def sync(self, user_msg: str, assistant_response: str):
        """Auto-save after LLM response."""
        # Append new memory entries
        # Could use clarify tool to ask user what to save
```

### Rust Pattern (Memory Providers)
```rust
#[async_trait]
pub trait MemoryProvider: Send + Sync {
    fn name(&self) -> &str;
    
    async fn prefetch(&self, user_message: &str) -> Result<String, String>;
    async fn sync(&self, user_msg: &str, assistant_response: &str) -> Result<(), String>;
    fn get_tool_schemas(&self) -> Vec<serde_json::Value>;
}

pub struct BuiltinMemoryProvider {
    name: String,
    memory_path: std::path::PathBuf,
    user_path: std::path::PathBuf,
}

#[async_trait]
impl MemoryProvider for BuiltinMemoryProvider {
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn prefetch(&self, user_message: &str) -> Result<String, String> {
        let mut memory = String::new();
        
        if let Ok(content) = tokio::fs::read_to_string(&self.memory_path).await {
            memory = format!("Memory:\n{}", content);
        }
        
        if let Ok(content) = tokio::fs::read_to_string(&self.user_path).await {
            memory.push_str(&format!("\n\nUser Profile:\n{}", content));
        }
        
        Ok(memory)
    }
    
    async fn sync(&self, user_msg: &str, assistant_response: &str) -> Result<(), String> {
        // Append to memory files asynchronously
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let entry = format!(
            "\n[{}]\nUser: {}\nAssistant: {}\n",
            timestamp, user_msg, assistant_response
        );
        
        tokio::fs::write(
            &self.memory_path,
            entry
        ).await.map_err(|e| e.to_string())
    }
    
    fn get_tool_schemas(&self) -> Vec<serde_json::Value> {
        vec![
            serde_json::json!({
                "name": "memory_read",
                "description": "Read memory entries",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            }),
            serde_json::json!({
                "name": "memory_write",
                "description": "Write memory entry",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "content": {
                            "type": "string",
                            "description": "Memory content to append"
                        }
                    },
                    "required": ["content"]
                }
            }),
        ]
    }
}

pub struct MemoryManager {
    providers: Vec<Arc<dyn MemoryProvider>>,
    tool_to_provider: std::collections::HashMap<String, Arc<dyn MemoryProvider>>,
    has_external: bool,
}

impl MemoryManager {
    pub fn add_provider(&mut self, provider: Arc<dyn MemoryProvider>) {
        if provider.name() != "builtin" && self.has_external {
            eprintln!("Only one external provider allowed");
            return;
        }
        
        for schema in provider.get_tool_schemas() {
            if let Some(name) = schema.get("name").and_then(|v| v.as_str()) {
                self.tool_to_provider.insert(name.to_string(), Arc::clone(&provider));
            }
        }
        
        if provider.name() != "builtin" {
            self.has_external = true;
        }
        
        self.providers.push(provider);
    }
    
    pub async fn prefetch_all(&self, user_message: &str) -> String {
        let mut contexts = vec![];
        
        for provider in &self.providers {
            match provider.prefetch(user_message).await {
                Ok(context) => contexts.push(context),
                Err(e) => eprintln!("Prefetch failed: {}", e),
            }
        }
        
        let combined = contexts.join("\n");
        Self::build_memory_context_block(&combined)
    }
    
    pub async fn sync_all(&self, user_msg: &str, assistant_response: &str) {
        for provider in &self.providers {
            let provider = Arc::clone(provider);
            let user_msg = user_msg.to_string();
            let assistant_response = assistant_response.to_string();
            
            tokio::spawn(async move {
                if let Err(e) = provider.sync(&user_msg, &assistant_response).await {
                    eprintln!("Sync failed for {}: {}", provider.name(), e);
                }
            });
        }
    }
    
    fn build_memory_context_block(context: &str) -> String {
        if context.trim().is_empty() {
            return String::new();
        }
        
        format!(
            "<memory-context>\n[System note: recalled memory context, NOT new input]\n\n{}\n</memory-context>",
            context
        )
    }
}
```

---

## VII. Error Handling Tree

### Python Pattern (Classification)
```python
# agent/error_classifier.py
def classify_api_error(exception, response_code, error_text) -> FailoverReason:
    """Classify error type for decision tree."""
    
    # Rate limit errors
    if response_code in (429, 503):
        return FailoverReason.RATE_LIMIT
    
    # Auth errors
    if response_code in (401, 403):
        if "api_key" in error_text.lower() or "unauthorized" in error_text.lower():
            return FailoverReason.INVALID_TOKEN
    
    # Model not found
    if response_code == 404:
        return FailoverReason.MODEL_NOT_FOUND
    
    # Context length
    if "context_length" in error_text.lower() or "max_tokens" in error_text.lower():
        return FailoverReason.CONTEXT_LENGTH
    
    # Server overload
    if response_code in (502, 503, 504):
        return FailoverReason.OVERLOADED
    
    # Tool-specific errors (caught in registry.dispatch)
    if isinstance(exception, ToolError):
        return FailoverReason.TOOL_ERROR
    
    return FailoverReason.UNKNOWN

# In run_agent.py
def _call_llm_with_fallback(self, messages, tools, stream):
    """Call with classification-based fallback."""
    try:
        return self.client.chat.completions.create(
            model=self.model,
            messages=messages,
            tools=tools,
            stream=stream,
        )
    except Exception as e:
        reason = classify_api_error(e, e.response.status_code, str(e))
        
        if reason == FailoverReason.RATE_LIMIT:
            if self._fallback_chain:
                self._try_activate_fallback()
                return self._call_llm_with_fallback(messages, tools, stream)
            else:
                # No fallback, retry with backoff
                time.sleep(jittered_backoff(1))
                return self._call_llm_with_fallback(messages, tools, stream)
        
        elif reason == FailoverReason.CONTEXT_LENGTH:
            # Compress and retry
            # (Should have been caught upstream)
            raise
        
        else:
            # Other errors: retry once, then fail
            time.sleep(jittered_backoff(1))
            return self._call_llm_with_fallback(messages, tools, stream)
```

### Rust Pattern (Error Classification)
```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ErrorReason {
    RateLimit,
    ServerOverloaded,
    AuthInvalid,
    ContextLengthExceeded,
    ModelNotFound,
    ToolExecutionError,
    Unknown,
}

pub fn classify_error(status: u16, message: &str) -> ErrorReason {
    match status {
        429 | 503 => ErrorReason::RateLimit,
        401 | 403 => {
            if message.to_lowercase().contains("api_key") {
                ErrorReason::AuthInvalid
            } else {
                ErrorReason::AuthInvalid
            }
        }
        404 => ErrorReason::ModelNotFound,
        502 | 504 => ErrorReason::ServerOverloaded,
        _ => {
            if message.to_lowercase().contains("context_length") {
                ErrorReason::ContextLengthExceeded
            } else {
                ErrorReason::Unknown
            }
        }
    }
}

// In AIAgent
async fn call_llm_with_fallback(
    &mut self,
    messages: &[Message],
) -> Result<String, String> {
    match self.client.chat_completions(
        messages.to_vec(),
        Some(self.format_tools()),
        true,
    ).await {
        Ok(response) => Ok(response),
        Err(e) => {
            let reason = self.classify_error(&e);
            
            match reason {
                ErrorReason::RateLimit => {
                    if !self.fallback_chain.is_empty() {
                        self.try_activate_fallback();
                        self.call_llm_with_fallback(messages).await
                    } else {
                        Err(format!("Rate limited and no fallback: {}", e))
                    }
                }
                ErrorReason::ContextLengthExceeded => {
                    Err("Context length exceeded, should be caught upstream".to_string())
                }
                _ => {
                    // Retry once
                    tokio::time::sleep(
                        std::time::Duration::from_millis(1000)
                    ).await;
                    self.client.chat_completions(
                        messages.to_vec(),
                        Some(self.format_tools()),
                        true,
                    ).await
                }
            }
        }
    }
}
```

---

**This reference guide covers the essential patterns needed for a complete Rust/Tauri reconstruction.**

---

Version: 1.0  
Last Updated: 2026-04-11  
Focus: Immediate implementation patterns for developers
