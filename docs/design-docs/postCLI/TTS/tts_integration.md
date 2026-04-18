# TTS Design Document — MOSS-TTS-Nano Rust ONNX Native Integration

> **Status**: Draft  
> **Created**: 2026-04-19  
> **Reference**: `/Users/ryanliu/Documents/IfAI/MOSS-TTS-Nano-main`  
> **License**: Apache 2.0 (verified — safe for commercial use)

---

## 1. Executive Summary

This document describes the complete integration of [MOSS-TTS-Nano](https://github.com/OpenMOSS/MOSS-TTS-Nano) into If2Ai as a **pure Rust ONNX Runtime native** TTS engine — zero Python runtime dependency.

**Two end-goals**:
1. **Complete feature parity** with MOSS-TTS-Nano Python reference: voice clone, streaming, 20 languages, 15 voice presets, all sampling controls, warmup, text normalization.
2. **Settings Page TTS Test UI**: A React page inside If2Ai Settings that is a 1:1 functional reproduction of `app.py`'s web demo — every control, every feature, every UX pattern.

**Total model size**: ~120M parameters (~700MB ONNX files + ~130MB audio tokenizer ONNX files).  
**Runtime**: ONNX Runtime CPU via `ort` crate 2.0.  
**Target latency**: Sub-second first audio on 4-core CPU (verified on M4 MacBook Air).

---

## 2. Complete Feature Inventory (from app.py + moss_tts_nano_runtime.py)

### 2.1 Core TTS Pipeline

| Feature | Reference Implementation | Rust Target |
|---------|------------------------|-------------|
| **Voice Clone** | `mode="voice_clone"` with prompt audio | ✅ 1:1 |
| **Continuation** | `mode="continuation"` with prompt text + audio | ✅ 1:1 |
| **Voice Presets** | 15 built-in voices (Junhao, Trump, Sakura, etc.) | ✅ 1:1 |
| **20 Languages** | zh, en, de, es, fr, ja, it, hu, ko, ru, fa, ar, pl, pt, cs, da, sv, el, tr | ✅ 1:1 |
| **Streaming** | `synthesize_stream()` → per-frame PCM chunks | ✅ 1:1 |
| **Buffered** | `synthesize()` → complete WAV | ✅ 1:1 |
| **Warmup** | `warmup()` with short Chinese text | ✅ 1:1 |
| **Long Text** | Auto-split by token budget (`voice_clone_max_text_tokens`) | ✅ 1:1 |

### 2.2 Generation Parameters (all from app.py Generation Options)

| Parameter | Default | Range | Purpose |
|-----------|---------|-------|---------|
| `max_new_frames` | 375 | 64-1024 | Max audio frames to generate |
| `voice_clone_max_text_tokens` | 75 | 25-200 | Max tokens per chunk for voice clone splitting |
| `tts_max_batch_size` | 0 (auto) | 0+ | Max TTS chunk batch size |
| `codec_max_batch_size` | 0 (auto) | 0+ | Max codec batch size |
| `cpu_threads` | 4 | 1+ | PyTorch/ONNX thread count |
| `attn_implementation` | model_default | sdpa/eager/model_default | Attention backend |
| `seed` | 0 | any int | Random seed for reproducibility |
| `do_sample` | true | bool | Enable sampling vs greedy |
| `text_temperature` | 1.0 | 0.1-2.0 | Text token sampling temperature |
| `text_top_p` | 1.0 | 0.1-1.0 | Text token nucleus sampling |
| `text_top_k` | 50 | 1-100 | Text token top-k filtering |
| `audio_temperature` | 0.8 | 0.1-2.0 | Audio token sampling temperature |
| `audio_top_p` | 0.95 | 0.1-1.0 | Audio token nucleus sampling |
| `audio_top_k` | 25 | 1-100 | Audio token top-k filtering |
| `audio_repetition_penalty` | 1.2 | 1.0-2.0 | Audio token repetition penalty |
| `enable_text_normalization` | true | bool | Enable WeTextProcessing |
| `enable_normalize_tts_text` | true | bool | Enable robust text normalization |
| `realtime_stream` | true | bool | Streaming vs buffered playback |
| `initial_playback_delay_seconds` | 0.08 | 0.0+ | Streaming initial buffer delay |

### 2.3 Text Normalization Pipeline

| Component | Reference (Python) | Rust Target |
|-----------|-------------------|-------------|
| **Robust Normalizer** | `tts_robust_normalizer_single_script.py` (~450 lines pure regex) | ✅ Pure Rust regex |
| **WeTextProcessing** | `WeTextProcessing` Python package (Chinese semantic TN) | ❌ Skip — Python-only; robust normalizer handles 95% |
| **Text Chunker** | `model._split_text_into_best_sentences()` via token budget | ✅ Rust equivalent |
| **Normalization Status** | Warmup-style loading manager with state machine | ✅ Rust async state machine |

### 2.4 Web Demo UI Features (app.py — 2905 lines, full reproduction required)

| UI Component | Description | Reproduction |
|-------------|-------------|-------------|
| **Hero Section** | Title, description, feature bullets | ✅ React |
| **Demo Dropdown** | 29 demo entries from `demo.jsonl` | ✅ Pre-bundled JSON |
| **Prompt Audio Upload** | File picker + preview + clear/reset | ✅ Browser file API |
| **Prompt Audio Preview** | HTML5 `<audio>` element with controls | ✅ React audio player |
| **Text Input** | Multiline textarea for synthesis text | ✅ React textarea |
| **Generation Options** | Collapsible `<details>` with all 18 parameters | ✅ Collapsible panel |
| **Generate Button** | Primary action | ✅ React button |
| **Pause/Resume** | Toggle playback pause for both modes | ✅ Web Audio API |
| **Warmup Status** | Progress bar + status text | ✅ React status display |
| **Text Normalization Status** | Loading state display | ✅ React status display |
| **Run Status** | Real-time synthesis status | ✅ React status |
| **Normalized Text Output** | Read-only textarea showing processed text | ✅ React textarea |
| **Playback Script** | Sentence-level chunk visualization with active/played states | ✅ React span chips |
| **Generated Speech** | `<audio>` element for playback | ✅ HTML5 audio |
| **Stream Metrics** | emitted audio, lead time, first audio latency | ✅ React metrics bar |
| **Buffered Playback** | WAV base64 → Blob URL → `<audio>` playback | ✅ Same approach |
| **Streaming Playback** | Web Audio API `AudioContext` + PCM16LE chunk scheduling | ✅ Web Audio API |
| **Playback Highlight** | Active sentence highlighting during playback | ✅ React state + CSS |

### 2.5 API Endpoints (from app.py — to be replicated as Tauri commands)

| Endpoint | Method | Purpose | Tauri Command |
|----------|--------|---------|---------------|
| `/` | GET | HTML demo page | N/A (Settings UI) |
| `/health` | GET | System status | `tts_health()` |
| `/api/warmup-status` | GET | Warmup progress | `tts_warmup_status()` |
| `/api/generate` | POST | Synthesis (buffered) | `tts_synthesize()` |
| `/api/generate-stream/start` | POST | Start streaming job | `tts_stream_start()` |
| `/api/generate-stream/{id}/audio` | GET | PCM stream (SSE) | Tauri async channel |
| `/api/generate-stream/{id}/status` | GET | Job status polling | `tts_stream_status()` |
| `/api/generate-stream/{id}/result` | GET | Final result | `tts_stream_result()` |
| `/api/generate-stream/{id}/close` | POST | Cancel stream | `tts_stream_close()` |
| `/api/demo-prompt-audio/{id}` | GET | Demo audio file | `tts_demo_audio()` |

---

## 3. Rust Architecture

### 3.1 Module Structure

```
src-tauri/src/modules/tts/
├── mod.rs                    # Module root + TtsProvider trait
├── config.rs                 # TtsConfig, GenerationParams, VoicePreset
├── error.rs                  # TtsError enum
│
├── model/
│   ├── mod.rs               # Model loading coordinator
│   ├── downloader.rs        # HuggingFace download + cache manager
│   ├── global.rs            # Global ONNX sessions (prefill, decode_step)
│   ├── local.rs             # Local ONNX sessions (decoder, cached_step, fixed_sampled)
│   └── codec.rs             # Audio tokenizer ONNX (encode, decode_full, decode_step)
│
├── inference/
│   ├── mod.rs               # Inference pipeline entry
│   ├── prefill.rs           # text → prefill → global_hidden + KV cache
│   ├── decode.rs            # Autoregressive frame generation loop
│   ├── sampling.rs          # top-k/top-p/greedy/repetition_penalty
│   └── streaming.rs         # Streaming frame callback via async channel
│
├── text/
│   ├── mod.rs               # Text processing entry
│   ├── normalizer.rs        # Rust port of tts_robust_normalizer_single_script.py
│   ├── tokenizer.rs         # SentencePiece text tokenization (tokenizers crate)
│   └── chunker.rs           # Long text splitting by token budget
│
├── audio/
│   ├── mod.rs               # Audio processing entry
│   ├── encoder.rs           # Reference audio → codec encode → audio codes
│   ├── decoder.rs           # Audio codes → codec decode → waveform
│   └── wav.rs               # WAV encoding (hound crate)
│
├── voice/
│   ├── mod.rs               # Voice preset system
│   ├── presets.rs           # 15 built-in voice definitions
│   └── demo.rs              # 29 demo entries (from demo.jsonl)
│
├── manager/
│   ├── mod.rs               # TTS service manager
│   ├── warmup.rs            # Warmup state machine (async)
│   └── jobs.rs              # Streaming job manager (state + lifecycle)
│
├── provider/
│   ├── mod.rs               # TtsProvider trait impl
│   └── onnx.rs              # OnnxTtsProvider (main implementation)
│
└── commands.rs              # Tauri #[tauri::command] functions
```

### 3.2 Key Types

```rust
/// Top-level TTS service trait — mirrors NanoTTSService
#[async_trait]
pub trait TtsProvider: Send + Sync {
    /// Buffered synthesis — returns complete audio
    async fn synthesize(&self, params: SynthesisParams) -> Result<SynthesisResult>;

    /// Streaming synthesis — yields audio chunks via callback
    async fn synthesize_stream(
        &self,
        params: SynthesisParams,
        sink: Arc<dyn AudioSink>,
    ) -> Result<StreamResult>;

    /// Warmup synthesis — short test to prime the model
    async fn warmup(&self) -> Result<WarmupResult>;

    /// Split text into chunks for voice clone mode
    fn split_voice_clone_text(
        &self,
        text: &str,
        max_tokens: usize,
    ) -> Result<Vec<String>>;

    /// List available voice names
    fn list_voices(&self) -> Vec<String>;

    /// Get voice preset by name
    fn get_voice(&self, name: &str) -> Option<&VoicePreset>;
}

/// Generation parameters — mirrors all 18 app.py form fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationParams {
    pub max_new_frames: u32,              // default: 375
    pub voice_clone_max_text_tokens: u32,  // default: 75
    pub tts_max_batch_size: u32,           // default: 0 (auto)
    pub codec_max_batch_size: u32,         // default: 0 (auto)
    pub do_sample: bool,                   // default: true
    pub text_temperature: f32,             // default: 1.0
    pub text_top_p: f32,                   // default: 1.0
    pub text_top_k: u32,                   // default: 50
    pub audio_temperature: f32,            // default: 0.8
    pub audio_top_p: f32,                  // default: 0.95
    pub audio_top_k: u32,                  // default: 25
    pub audio_repetition_penalty: f32,     // default: 1.2
    pub seed: Option<u64>,                 // default: None
    pub enable_robust_normalization: bool, // default: true
    pub realtime_stream: bool,             // default: true
}

/// Voice preset — mirrors VoicePreset in moss_tts_nano_runtime.py
#[derive(Debug, Clone)]
pub struct VoicePreset {
    pub name: String,               // "Junhao", "Trump", "Sakura", ...
    pub audio_data: Vec<u8>,        // Embedded audio bytes (from assets/)
    pub description: String,        // "Chinese male voice A"
}

/// Audio sink for streaming — async channel callback
#[async_trait]
pub trait AudioSink: Send + Sync {
    /// Called for each PCM audio chunk
    async fn on_audio(&self, chunk: AudioChunk);
    /// Called when streaming completes
    async fn on_complete(&self, result: StreamResult);
}

/// Single audio chunk in streaming — PCM16LE
pub struct AudioChunk {
    pub pcm_data: Vec<u8>,          // PCM16LE bytes
    pub sample_rate: u32,           // 48000
    pub channels: u16,              // 2 (stereo)
    pub chunk_index: usize,         // Which text chunk this belongs to
    pub is_pause: bool,             // Pause frame (chunk boundary)
    pub emitted_audio_seconds: f32,
    pub lead_seconds: f32,
}
```

### 3.3 ONNX Model File Layout

Models cached at `~/.if2ai/models/tts/`:

```
~/.if2ai/models/tts/
├── tts/                                    # MOSS-TTS-Nano-100M-ONNX
│   ├── moss_tts_prefill.onnx              # Global prefill graph
│   ├── moss_tts_decode_step.onnx          # Global decode step with KV cache
│   ├── moss_tts_local_decoder.onnx        # Local decoder
│   ├── moss_tts_local_cached_step.onnx    # Local cached step
│   ├── moss_tts_local_fixed_sampled_frame.onnx
│   ├── moss_tts_global_shared.data        # External weights (shared by global)
│   ├── moss_tts_local_shared.data         # External weights (shared by local)
│   └── tokenizer.model                    # SentencePiece tokenizer
│
├── audio_tokenizer/                        # MOSS-Audio-Tokenizer-Nano-ONNX
│   ├── moss_audio_tokenizer_encode.onnx
│   ├── moss_audio_tokenizer_encode.data
│   ├── moss_audio_tokenizer_decode_full.onnx
│   ├── moss_audio_tokenizer_decode_step.onnx
│   ├── moss_audio_tokenizer_decode_shared.data
│   └── codec_browser_onnx_meta.json
│
└── voices/                                 # Embedded voice preset audio files
    ├── zh_1.wav, zh_2.wav, zh_3.wav, zh_4.wav, zh_5.wav, zh_6.wav
    ├── zh_10.wav, zh_11.wav
    ├── en_2.wav, en_3.wav, en_4.wav, en_5.wav, en_6.wav, en_7.wav, en_8.wav
    ├── jp_1.mp3, jp_2.wav, jp_3.wav, jp_4.wav, jp_5.wav
    └── demo.jsonl                          # 29 demo entries
```

### 3.4 Tensor Shape Reference

| Session | Input | Shape | Dtype | Output | Shape | Dtype |
|---------|-------|-------|-------|--------|-------|-------|
| **prefill** | `input_ids` | `[1, seq_len]` | int64 | `global_hidden` | `[1, hidden]` | f32 |
| | `audio_codes` | `[1, n_frames, 16]` | int32 | `past_key_values` | `N×[1, kv_heads, seq, head_dim]` | f32 |
| | `attention_mask` | `[1, seq_len]` | int64 | `attention_mask` | `[1, seq_len]` | int64 |
| **decode_step** | `input_ids` | `[1, 1]` | int64 | `logits` | `[1, 1, vocab]` | f32 |
| | `past_key_values` | `N×[1, kv_heads, seq, head_dim]` | f32 | `past_key_values` | updated | f32 |
| | `attention_mask` | `[1, seq+1]` | int64 | `attention_mask` | `[1, seq+1]` | int64 |
| **codec_encode** | `waveform` | `[1, 2, time]` | f32 | `audio_codes` | `[1, n_frames, 16]` | int32 |
| **codec_decode_full** | `audio_codes` | `[1, n_frames, 16]` | int32 | `waveform` | `[1, 2, samples]` | f32 |

### 3.5 Cargo.toml New Dependencies

```toml
# TTS module
ort = { version = "2", features = ["ndarray", "half"] }
ndarray = "0.16"
tokenizers = { version = "0.21", features = ["sentencepiece"] }
hound = "3.5"         # WAV file writing
rand = "0.8"          # sampling (top-k, top-p, repetition penalty)
symphonia = { version = "0.5", features = ["mp3"] }  # decode voice preset MP3
```

---

## 4. Text Normalizer — Rust Port Plan

The `tts_robust_normalizer_single_script.py` (450 lines) is **pure regex** — no external dependencies. It is the single most important piece for Rust porting.

### 4.1 Function Signature

```rust
/// Mirror of normalize_tts_text() from tts_robust_normalizer_single_script.py
pub fn normalize_tts_text(text: &str) -> String;
```

### 4.2 Rules to Port (all from Python TEST_CASES as contract)

| Rule Category | Python Function | Rust Equivalent |
|--------------|----------------|-----------------|
| Base cleanup | `_base_cleanup()` | `fn base_cleanup(text: &str) -> String` |
| Markdown normalization | `_normalize_markdown_and_lines()` | `fn normalize_markdown_and_lines(text: &str) -> String` |
| Span protection | `_protect_spans()` | `fn protect_spans(text: &str) -> (String, Vec<String>)` |
| Span restoration | `_restore_spans()` | `fn restore_spans(text: &str, protected: &[String]) -> String` |
| Underscore normalization | `_normalize_visible_underscores()` | `fn normalize_underscores(text: &str) -> String` |
| Space normalization | `_normalize_spaces()` | `fn normalize_spaces(text: &str) -> String` |
| Structural punctuation | `_normalize_structural_punctuation()` | `fn normalize_structural_punctuation(text: &str) -> String` |
| Repeated punctuation | `_normalize_repeated_punctuation()` | `fn normalize_repeated_punctuation(text: &str) -> String` |
| Terminal punctuation | `_ensure_terminal_punctuation()` | `fn ensure_terminal_punctuation(text: &str) -> String` |

### 4.3 Test Contract

The 30+ `TEST_CASES` from the Python file will be converted to `#[test]` functions in Rust. Each test verifies:
1. Output matches expected string exactly
2. **Idempotence**: `normalize_tts_text(normalize_tts_text(x)) == normalize_tts_text(x)`

---

## 5. Tauri Commands (Backend API)

### 5.1 Command List

```rust
/// Check TTS system health and model status
#[tauri::command]
async fn tts_health(state: State<'_, TtsAppState>) -> Result<TtsHealthResponse>;

/// Start or get warmup status
#[tauri::command]
async fn tts_warmup_status(state: State<'_, TtsAppState>) -> Result<WarmupStatusResponse>;

/// Trigger warmup (background)
#[tauri::command]
async fn tts_start_warmup(state: State<'_, TtsAppState>) -> Result<()>;

/// Buffered synthesis — returns complete WAV as base64
#[tauri::command]
async fn tts_synthesize(
    state: State<'_, TtsAppState>,
    text: String,
    demo_id: Option<String>,
    prompt_audio_path: Option<String>,
    params: GenerationParams,
) -> Result<SynthesisResponse>;

/// Start streaming synthesis — returns stream_id
#[tauri::command]
async fn tts_stream_start(
    state: State<'_, TtsAppState>,
    text: String,
    demo_id: Option<String>,
    prompt_audio_path: Option<String>,
    params: GenerationParams,
) -> Result<StreamStartResponse>;

/// Get stream status
#[tauri::command]
async fn tts_stream_status(
    state: State<'_, TtsAppState>,
    stream_id: String,
) -> Result<StreamStatusResponse>;

/// Get stream final result
#[tauri::command]
async fn tts_stream_result(
    state: State<'_, TtsAppState>,
    stream_id: String,
) -> Result<StreamResultResponse>;

/// Close/cancel stream
#[tauri::command]
async fn tts_stream_close(
    state: State<'_, TtsAppState>,
    stream_id: String,
) -> Result<StreamStatusResponse>;

/// Get demo prompt audio (as base64)
#[tauri::command]
async fn tts_demo_audio(
    state: State<'_, TtsAppState>,
    demo_id: String,
) -> Result<DemoAudioResponse>;

/// List all available voices
#[tauri::command]
async fn tts_list_voices(state: State<'_, TtsAppState>) -> Result<Vec<String>>;

/// Split text into chunks for voice clone preview
#[tauri::command]
async fn tts_split_text(
    state: State<'_, TtsAppState>,
    text: String,
    max_tokens: u32,
) -> Result<Vec<String>>;
```

### 5.2 Streaming Audio Delivery

For streaming PCM audio to the frontend, use Tauri's `async channel` pattern:

```rust
// The streaming job writes AudioChunk to the channel,
// the frontend reads via a Tauri event or side-channel.
// Alternative: emit Tauri events for each chunk:
app.emit("tts:audio_chunk", AudioChunkPayload { ... });
```

---

## 6. Settings Page TTS Test UI (app.py → React)

### 6.1 Page Architecture

The TTS Test page is a **new settings page** at `src/modules/settings/pages/TtsTestPage.tsx`.

It replicates **every feature** of `app.py`'s web demo:

```
┌─────────────────────────────────────────────────────────────┐
│  MOSS-TTS-Nano Demo                                         │
│  State-of-the-art text-to-speech for multilingual cloning   │
│  • Voice Clone — Clone any voice from reference audio       │
│  Built with MOSS-TTS-Nano                                   │
├───────────────────────┬─────────────────────────────────────┤
│  INPUT PANEL          │  OUTPUT PANEL                       │
│                       │                                     │
│  ▾ Demo               │  ♫ Warmup Status                    │
│    🇨🇳 欢迎...         │  [Warmup complete. device=cpu ...]  │
│                       │                                     │
│  ♫ Prompt Speech      │  ♫ Text Normalization Status        │
│  ┌──────────────────┐ │  [WeTextProcessing disabled.]       │
│  │ [Choose File]    │ │                                     │
│  │ Using demo prompt│ │  ♫ Run Status                       │
│  │ speech: zh_1.wav │ │  [Idle.]                            │
│  │ [更换] [恢复Demo]│ │                                     │
│  └──────────────────┘ │  ♫ Normalized Text                  │
│                       │  ┌──────────────────────────────┐   │
│  ♫ Text               │  │ 你好，欢迎使用模思智能。      │   │
│  ┌──────────────────┐ │  │                              │   │
│  │ 欢迎关注模思智能  │ │  └──────────────────────────────┘   │
│  │                  │ │                                     │
│  │                  │ │  ♫ Playback Script                  │
│  └──────────────────┘ │  ┌──────────────────────────────┐   │
│                       │  │ 你好， 欢迎使用模思智能。     │   │
│  ▼ Generation Options │  └──────────────────────────────┘   │
│    Max New Frames [375]│                                     │
│    VC Max Tokens [75]  │  ♫ Generated Speech                │
│    TTS Batch Size [0]  │  voice=Junhao | prompt=zh_1.wav     │
│    Codec Batch Size [0]│                                     │
│    CPU Threads [4]     │  ┌──────────────────────────────┐   │
│    Attention [default] │  │ [▶] ━━━━━━━●────── 0:03/0:05 │   │
│    Seed [0]            │  └──────────────────────────────┘   │
│    Text Temp [1.0]     │                                     │
│    Text Top P [1.0]    │  Checkpoint: ~/.if2ai/...           │
│    Text Top K [50]     │  Audio Tokenizer: ~/.if2ai/...      │
│    Audio Temp [0.8]    │                                     │
│    Audio Top P [0.95]  │                                     │
│    Audio Top K [25]    │                                     │
│    Audio Rep Penalty   │                                     │
│    [1.2]               │                                     │
│    ☑ Do Sample         │                                     │
│    ☑ Enable Robust Norm│                                     │
│    ☑ Realtime Stream   │                                     │
│    Initial Delay [0.08]│                                     │
│                       │                                     │
│  [Generate] [Pause]   │                                     │
│  [Refresh Warmup]     │                                     │
└───────────────────────┴─────────────────────────────────────┘
```

### 6.2 Component Breakdown

| React Component | Source (app.py equivalent) | Purpose |
|----------------|--------------------------|---------|
| `TtsTestPage.tsx` | `_render_index_html()` + `<script>` | Main page container |
| `TtsDemoSelector.tsx` | `<select id="demo">` + JS logic | Demo dropdown + text/audio loading |
| `TtsPromptAudioUpload.tsx` | `<input type="file">` + preview JS | Audio file upload + preview |
| `TtsTextInput.tsx` | `<textarea id="text">` | Text input with live preview |
| `TtsGenerationOptions.tsx` | `<details>` + all form fields | Collapsible parameter panel |
| `TtsWarmupStatus.tsx` | WarmupManager + `_warmup_status_text()` | Warmup progress display |
| `TtsTextNormalizationStatus.tsx` | WeTextProcessing status | Text norm status display |
| `TtsRunStatus.tsx` | Run status div | Synthesis status display |
| `TtsNormalizedText.tsx` | Normalized text textarea | Shows processed text |
| `TtsPlaybackScript.tsx` | `renderPlaybackScript()` + segment highlighting | Sentence-level chunk display with active/played states |
| `TtsAudioPlayer.tsx` | `<audio>` element + Web Audio API | Audio playback (buffered + streaming) |
| `TtsStreamMetrics.tsx` | Stream metrics bar | emitted/lead/first_audio latency |
| `TtsResolvedPrompt.tsx` | Resolved prompt display | Shows which voice/prompt was used |

### 6.3 State Management

```typescript
interface TtsTestState {
  // Selection
  selectedDemoId: string;
  uploadedPromptAudio: File | null;
  promptAudioPreviewUrl: string | null;

  // Input
  text: string;
  normalizedText: string;
  textChunks: string[];

  // Parameters (all 18)
  params: GenerationParams;

  // Status
  warmupStatus: WarmupStatusResponse | null;
  textNormalizationStatus: TextNormStatusResponse | null;
  runStatus: string;
  streamMetrics: string | null;

  // Playback
  isGenerating: boolean;
  isStreaming: boolean;
  isPaused: boolean;
  currentStreamId: string | null;
  playbackChunkIndex: number | null;
  hasBufferedPlayback: boolean;
  hasRealtimePlayback: boolean;

  // Audio
  bufferedAudioUrl: string | null;
}
```

### 6.4 Streaming Audio Playback (Web Audio API)

Mirror `app.py`'s `generateRealtime()` function exactly:

```typescript
// 1. Start stream → get stream_id, sample_rate, channels
// 2. Create AudioContext with sampleRate
// 3. Fetch PCM stream from Tauri event channel
// 4. For each PCM chunk:
//    - Align to frame boundary (channels * 2 bytes)
//    - Create AudioBuffer, fill with Int16 → Float32 conversion
//    - Schedule via AudioBufferSourceNode.start(startTime)
//    - Maintain nextPlaybackTime for gapless playback
// 5. Poll status endpoint for highlight updates
// 6. On completion: mark all played, close AudioContext
```

### 6.5 Playback Script Highlighting

Mirror `app.py`'s `setPlaybackHighlight()` + `updateBufferedPlaybackHighlight()` + `updateRealtimePlaybackHighlightFromLocalClock()`:

- Each text chunk rendered as an inline `<span.playback-segment>`
- Three states: default, `.played` (dim), `.active` (highlighted)
- Buffered mode: time-based via `audio.currentTime` + weighted boundaries
- Streaming mode: server-driven via `playback_chunk_index` from status polling
- Auto-scroll to active segment

---

## 7. Phase-by-Phase Execution Plan

### Phase TTS-1: Infrastructure (4 slices, 3-4 days)

| Slice | impl_targets | Acceptance |
|-------|-------------|------------|
| **TTS-1.1** | `mod.rs`, `config.rs`, `error.rs`, `provider/mod.rs`, `provider/mock.rs` | `cargo test` passes; trait compiles; mock returns dummy audio |
| **TTS-1.2** | `model/downloader.rs`, `voice/presets.rs`, `voice/demo.rs`, `voice/mod.rs` | Download + cache test; 15 voices load; 29 demos parse |
| **TTS-1.3** | `text/normalizer.rs`, `text/mod.rs` | All 30 Python TEST_CASES pass as Rust tests; idempotence verified |
| **TTS-1.4** | `audio/wav.rs`, `audio/mod.rs` | WAV encode roundtrip test: f32 → 16-bit PCM → f32 matches |

### Phase TTS-2: ONNX Model Loading (3 slices, 3-4 days)

| Slice | impl_targets | Acceptance |
|-------|-------------|------------|
| **TTS-2.1** | `model/global.rs`, `model/mod.rs` | prefill + decode_step sessions load; input/output names dumped and match Python |
| **TTS-2.2** | `model/local.rs` | 3 local sessions load; tensor shapes verified |
| **TTS-2.3** | `model/codec.rs`, `audio/encoder.rs`, `audio/decoder.rs` | codec encode(waveform) → codes → decode(waveform) roundtrip; output shape matches |

### Phase TTS-3: Inference Pipeline (5 slices, 5-7 days)

| Slice | impl_targets | Acceptance |
|-------|-------------|------------|
| **TTS-3.1** | `text/tokenizer.rs`, `text/chunker.rs` | SentencePiece encode/decode matches Python; chunker splits at sentence boundaries |
| **TTS-3.2** | `inference/prefill.rs` | prefill(input_ids) produces global_hidden + KV cache; tensor shapes correct |
| **TTS-3.3** | `inference/sampling.rs` | greedy/top-k/top-p/repetition_penalty unit tests; distribution matches Python |
| **TTS-3.4** | `inference/decode.rs`, `inference/mod.rs` | Full autoregressive loop: text → audio_codes; output matches Python infer_onnx.py |
| **TTS-3.5** | `provider/onnx.rs`, `inference/streaming.rs` | End-to-end: synthesize(text) → WAV file; streaming emits chunks |

### Phase TTS-4: Voice Clone + Advanced Features (3 slices, 3-4 days)

| Slice | impl_targets | Acceptance |
|-------|-------------|------------|
| **TTS-4.1** | `provider/onnx.rs` (voice clone path), `manager/warmup.rs` | Voice clone: prompt_audio + text → cloned audio; warmup completes |
| **TTS-4.2** | `manager/jobs.rs`, streaming audio delivery | Streaming job lifecycle: start → chunks → complete; job manager handles concurrent streams |
| **TTS-4.3** | `text/chunker.rs` (full), continuation mode | Long text auto-split + sequential synthesis; continuation mode with prompt_text |

### Phase TTS-5: Tauri Integration (3 slices, 3-4 days)

| Slice | impl_targets | Acceptance |
|-------|-------------|------------|
| **TTS-5.1** | `commands.rs`, `lib.rs` (command registration) | All 12 Tauri commands registered and respond correctly |
| **TTS-5.2** | `src/modules/settings/pages/TtsTestPage.tsx`, settings integration | Full UI renders; all 18 parameters editable; demo selector loads 29 entries |
| **TTS-5.3** | TTS UI + commands integration tests | Buffered synthesis produces playable audio; streaming playback works via Web Audio API |

### Phase TTS-6: Polish + Performance (2 slices, 2-3 days)

| Slice | impl_targets | Acceptance |
|-------|-------------|------------|
| **TTS-6.1** | Performance tuning, memory optimization | CPU inference latency < 2x Python ONNX; memory < 2GB; thread count control works |
| **TTS-6.2** | Error handling, edge cases, final review | Empty text handled; invalid demo_id handled; model-not-downloaded state handled gracefully |

**Total: 20 slices across 6 phases, ~17-23 days of work.**

---

## 8. Risk Assessment & Mitigation

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| **ort 2.0 RC KV cache tensor round-trip** | Medium | High | TTS-2.1 first task: 50-line POC to verify KV cache tensor can be passed in/out of decode_step session |
| **SentencePiece Rust API compatibility** | Low | Medium | `tokenizers` crate has first-class SP support; pre-verify with tokenizer.model from the model |
| **ONNX external .data file loading** | Low | High | ort crate uses ONNX Runtime's standard external data mechanism; verify on first model load |
| **WeTextProcessing skip impact** | Medium | Low | Robust normalizer handles 95% of cases; WeTextProcessing is for Chinese semantic expansion (numbers, dates); can be added later |
| **Numerical precision: Rust vs Python** | Medium | Medium | TTS-3.4 must do tensor-level comparison between Python `infer_onnx.py` and Rust output |
| **~700MB model download size** | High | Low | First-run download with progress bar; not bundled in Tauri binary; cached in `~/.if2ai/` |
| **Web Audio API streaming on all browsers** | Low | Low | Graceful fallback to buffered playback; Chrome/Safari/Firefox all support Web Audio API |
| **ort crate stability (RC version)** | Medium | Medium | Lock exact version; no minor version updates during development; test on CI |

---

## 9. Post-Implementation Verification Checklist

After all phases complete, verify against original `app.py`:

- [ ] Voice clone mode produces audio indistinguishable from Python version (same prompt + text + seed)
- [ ] All 15 voice presets work
- [ ] All 29 demo entries load and play
- [ ] All 18 generation parameters have effect on output
- [ ] Streaming playback works with Web Audio API (gapless)
- [ ] Buffered playback produces valid WAV
- [ ] Playback script highlighting works in both modes
- [ ] Pause/resume works in both modes
- [ ] Warmup state machine works (pending → running → ready/failed)
- [ ] Long text auto-split works
- [ ] Text normalization produces identical output to Python normalizer
- [ ] CPU thread count affects performance
- [ ] Seed=0 produces deterministic output
- [ ] Error handling for all edge cases (empty text, missing model, invalid demo)

---

## 10. Appendix: Reference File Mapping

| If2Ai Rust File | Python Reference | Notes |
|----------------|-----------------|-------|
| `tts/mod.rs` | `moss_tts_nano/__init__.py` | Module root + trait |
| `tts/config.rs` | `moss_tts_nano/defaults.py` | Defaults + config |
| `tts/provider/onnx.rs` | `ort_cpu_runtime.py` + `onnx_tts_runtime.py` | ONNX inference pipeline |
| `tts/inference/prefill.rs` | `ort_cpu_runtime.py` PrefillSession | Global prefill |
| `tts/inference/decode.rs` | `ort_cpu_runtime.py` DecodeSession | Autoregressive loop |
| `tts/inference/sampling.rs` | `ort_cpu_runtime.py` sampling methods | top-k/top-p/greedy |
| `tts/inference/streaming.rs` | `ort_cpu_runtime.py` CodecStreamingDecodeSession | PCM chunk callback |
| `tts/text/normalizer.rs` | `tts_robust_normalizer_single_script.py` | Pure regex port |
| `tts/text/tokenizer.rs` | Python SentencePiece usage | tokenizers crate |
| `tts/text/chunker.rs` | `model._split_text_into_best_sentences()` | Token-budget splitting |
| `tts/audio/encoder.rs` | `ort_cpu_runtime.py` CodecEncodeSession | Audio → codes |
| `tts/audio/decoder.rs` | `ort_cpu_runtime.py` CodecDecodeSession | Codes → audio |
| `tts/audio/wav.rs` | `_audio_to_wav_bytes()` in app.py | WAV encoding |
| `tts/voice/presets.rs` | `_DEFAULT_VOICE_FILES` in runtime | 15 voice definitions |
| `tts/voice/demo.rs` | `assets/demo.jsonl` | 29 demo entries |
| `tts/manager/warmup.rs` | `WarmupManager` in app.py | Async warmup state |
| `tts/manager/jobs.rs` | `StreamingJobManager` in app.py | Job lifecycle |
| `tts/commands.rs` | `_build_app()` routes in app.py | Tauri commands |
| `TtsTestPage.tsx` | `_render_index_html()` + `<script>` in app.py | Full web demo UI |
