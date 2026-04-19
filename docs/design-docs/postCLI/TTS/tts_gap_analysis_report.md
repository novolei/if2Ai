# TTS 模块 Gap 分析报告 — MOSS-TTS-Nano vs if2Ai 现状

> **作者**: Staff 系统架构师 + 资深 UI/UX 设计师视角
> **日期**: 2026-04-19
> **目标**: 在 if2Ai 中实现"离线、实时、流式"的 TTS 语音生成
> **参考**: `MOSS-TTS-Nano-main`（Python ONNX CPU 参考实现）vs `src-tauri/src/modules/tts/`（Rust Tauri 实现）
> **结论一句话**: 架构骨架（traits / manager / streaming / Tauri 命令 / Settings UI）≈ 85% 完成且品味很好；**但所有真实的 ONNX 推理调用 0% 实现，现在跑出来的"音频"就是 PCM 全 0 的静音 WAV**。距离"离线实时 TTS"还有约 60% 的核心算法工作量。

---

## 0. TL;DR — 高管视角（3 行）

1. **现状**：if2Ai 已经搭好 ~7,665 行 Rust + 798 行 React 的"形"，包括 traits、配置、warmup state machine、streaming job manager、12 个 Tauri 命令、完整 Settings 测试页 UI。形态、品味、模块化、文档都是 senior 水准。
2. **致命缺口**：`OnnxTtsProvider::run_synthesis` / `decode_audio_codes` / `encode_prompt_audio` / `PrefillRunner::run` / 所有 `LocalSessions::run_*` 全是 `// TODO`，**没有一处真的 `session.run(...)`**。同时 SentencePiece tokenizer 用错 crate，`n_vq=4` 是错的（应该 16），`audio_*_token_id` 是瞎写的常量，没有 manifest 解析，没有 reference 音频解码 / 重采样，没有 codec 流式 state machine。
3. **拉直路线**：先做"骨血"（manifest 解析 + 真 ONNX run + tokenizer 修复 + reference audio I/O），约 8-12 个 slice，2 周可达成"voice-clone 跑通且能听"，再 1 周追平 streaming + 全部生成参数，再 3-5 天打磨 UI 行为/可观测性。本报告第 9 节给出建议路线图。

---

## 1. 参考实现（MOSS-TTS-Nano）——是什么

### 1.1 体量与拓扑
- **0.1B 参数主模型** `MOSS-TTS-Nano-100M-ONNX` + **20M 参数音频 Tokenizer** `MOSS-Audio-Tokenizer-Nano-ONNX`（HF 自动下载 → `./models/`）
- **48 kHz / 立体声 / RVQ 16 codebook / 12.5 Hz token 流**
- 纯 Python + `onnxruntime` CPU；MacBook Air M4 单核可流畅推理
- 运行时核心文件：
  - `ort_cpu_runtime.py`（796 行）：所有 ONNX session 创建、prefill/decode/local_*/codec_* 调用、采样、流式 codec state machine
  - `onnx_tts_runtime.py`（662 行）：SentencePiece、reference audio 加载与重采样、voice-clone 文本切块、单次 chunk 编排、长文本拼接
  - `text_normalization_pipeline.py` + `tts_robust_normalizer_single_script.py`（~700 行）：两段式文本归一化
  - `app.py`（2,904 行）：FastAPI Web Demo + 流式 PCM SSE + Web Audio API 播放
  - `infer_onnx.py` / `app_onnx.py` / `moss_tts_nano/cli.py`：CLI 入口

### 1.2 推理 Pipeline（"金标准"）
```
text
 ├─► WeTextProcessing TN (Chinese semantic, 可选)
 ├─► robust_normalizer (regex, 必跑)
 │
 ▼
SentencePiece encode  ──►  text_token_ids: list[int]
                                │
prompt audio (wav/mp3) ─► torchaudio load + resample 48k stereo
                       ─► codec_encode.run(waveform, lengths)
                       ─► prompt_audio_codes: list[list[int]]   (frames × 16)
                                │
       build_voice_clone_request_rows(prompt_codes, text_ids):
         rows[i] = [text_or_slot_id, audio_codes_or_pad...]   width = n_vq + 1
         结构 = [user_prefix + audio_start]
              + audio_prefix_rows
              + [audio_end + after_reference + text_ids + assistant_prefix + audio_start]
                                │
                                ▼
prefill ONNX:  input_ids[1,L,17] int32 + attention_mask[1,L] int32
            ─► global_hidden[1,1,H] f32 + present_key_*/present_value_* (KV cache)
                                │
                                ▼
loop  step ∈ 0..max_new_frames:
   ├─ decode_step ONNX (autoreg single token across global layers) → 新 hidden + 更新 KV
   ├─ 采样 assistant text token (slot vs end) — top-k/top-p/temperature
   ├─ if end token: break
   ├─ 采样 audio frame (16 channels):
   │     - sample_mode = "fixed":  local_fixed_sampled_frame.run(...)  一次出 16 token
   │     - sample_mode = "greedy": local_greedy_frame.run(...)         一次出 16 token
   │     - sample_mode = "full":   local_cached_step 逐 channel 跑 16 次（带 local KV cache）
   ├─ generated_frames.append(frame)
   └─ on_frame callback (stream 模式立即送 codec_decode_step)
                                │
                                ▼
streaming 模式: CodecStreamingDecodeSession.run_frames(N=自适应1/2/4/8)
              ─► 维护 transformer_specs + attention_specs 状态
              ─► 实时输出 PCM chunk → AudioContext 调度
buffered 模式: codec_decode_full.run(all_frames) 一次出整段 PCM
```

### 1.3 流式实时性的关键设计（必须 1:1 移植）
- **自适应 decode batch budget** `_resolve_stream_decode_frame_budget(lead_seconds)`：
  - lead < 0.20s → 1 frame
  - < 0.55s → 2 frames
  - < 1.10s → 4 frames
  - 否则 8 frames
  这是把"首音延迟"压到亚秒、同时让中后段能跑得更快的核心。
- **Codec streaming session 的 state_feeds**：transformer offset 张量 + attention key/value/positions caches，每次 `run_frames` 后回灌；这是流式 codec 不需要重新 decode 全段的关键。
- **chunk 间停顿**：`estimate_voice_clone_inter_chunk_pause_seconds`（短 chunk 0.40s, 长 chunk 0.24s）保证长文本拼接听感自然。
- **Voice clone chunking**：3 段式（句末 → 子句 → token 二分）+ CJK 感知，保证每 chunk ≤ `voice_clone_max_text_tokens` (默认 75)，单 chunk 不切就 fallback 整段。

---

## 2. 当前 if2Ai 实现盘点

> 见 `src-tauri/src/modules/tts/`（30 个 Rust 文件，7,665 行）+ `src/modules/settings/pages/TtsTestPage.tsx`（798 行）+ `src-tauri/src/commands/tts.rs`（440 行）+ `tts_download.rs`（464 行）。

### 2.1 已经做得很好的部分（保留 / 不动）
| 模块                                                                                                                | 完成度 | 评价                                                                                         |
| ------------------------------------------------------------------------------------------------------------------- | ------ | -------------------------------------------------------------------------------------------- |
| `mod.rs` traits & 类型（TtsProvider / AudioSink / SynthesisParams / SynthesisResult / StreamResult / WarmupResult） | 100%   | 设计干净，命名 1:1 对齐 Python，向前兼容                                                     |
| `error.rs` TtsError enum（thiserror）                                                                               | 100%   | 符合 `.cursor/rules/rust.mdc`，命令边界友好                                                  |
| `manager/warmup.rs` 异步 state machine（pending → running → ready/failed）                                          | 90%    | 状态机和 snapshot 接口齐全                                                                   |
| `manager/jobs.rs` StreamingJobManager（生命周期 + 取消）                                                            | 85%    | 流式 job 生命周期完整                                                                        |
| `inference/streaming.rs` ChannelAudioSink + lead 计算                                                               | 70%    | 缺自适应 budget 与 Tauri Channel 推送                                                        |
| `audio/wav.rs` WAV 编/解码（hound）                                                                                 | 100%   | 但只覆盖 WAV，缺 MP3 / 重采样                                                                |
| `text/normalizer.rs` robust normalizer                                                                              | ~80%   | 582 行 Rust port，Test Cases 已经迁移；语义对照需要 contract test 验证（见 4.1）             |
| `commands/tts.rs` 12 个 Tauri 命令                                                                                  | 90%    | 命名、签名、Response 结构齐全；底层 provider 失败时全部退化为静音                            |
| `commands/tts_download.rs` HF 下载                                                                                  | ~80%   | 需进一步验证（见 5.5）                                                                       |
| `TtsTestPage.tsx` Settings 测试页                                                                                   | ~70%   | UI 框架到位、29 个 demo 已硬编码、参数面板齐；缺 Web Audio API 流式播放、播放高亮、暂停/恢复 |

### 2.2 严重缺失（红色）—— 0% 真正可用
| 模块                                                                                                                | 现状                                                                                           | 影响                                                     |
| ------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| `provider/onnx.rs::run_synthesis`                                                                                   | 完全 placeholder：构造空 frame `vec![0u32; n_vq]`                                              | 输出 PCM 全 0，听起来是静音                              |
| `provider/onnx.rs::decode_audio_codes`                                                                              | `Ok(vec![0.0f32; total_samples])`（注释 `// TODO`）                                            | 即使前面 ONNX 跑了也丢弃结果                             |
| `provider/onnx.rs::encode_prompt_audio`                                                                             | `Ok(vec![vec![0u32; self.n_vq]; 10])` 占位 10 帧                                               | Voice clone 完全失效                                     |
| `provider/onnx.rs::synthesize_stream`                                                                               | `for i in 0..10 { pcm_data: vec![0u8; pcm_size] }`                                             | 流式输出 10 段 0 字节，时长虚标                          |
| `inference/prefill.rs::run` / `run_from_session`                                                                    | 构造 ndarray 后 `let _input_ids = …;` 直接丢，返回 `ArrayD::zeros(...)`                        | 完全没有 ONNX 调用                                       |
| `inference/decode.rs` (718 行)                                                                                      | 有 `DecodeRunner` 类型/state，但所有 `run_decode_step / run_local_*` 都没有 `session.run` 调用 | 整个 autoregressive 循环不存在                           |
| `model/local.rs::run_local_decoder/cached_step/fixed_sampled`                                                       | 不存在，只有 `load()`                                                                          | 4 种采样路径全部走不通                                   |
| `model/codec.rs` CodecSessions                                                                                      | 只 `load()`，没有 `encode/decode_full/decode_step` 方法                                        | 既不能编 prompt 也不能解码音频                           |
| Codec 流式 state machine（对应 Python `CodecStreamingDecodeSession`）                                               | 不存在                                                                                         | 流式 PCM 输出根本无源                                    |
| Reference audio loader（mp3/wav → 48k stereo）                                                                      | 不存在；`symphonia` 在 Cargo 中但无模块                                                        | 用户上传 prompt audio 跑不通                             |
| Manifest / meta 解析（`browser_poc_manifest.json` / `tts_browser_onnx_meta.json` / `codec_browser_onnx_meta.json`） | 不存在                                                                                         | 模型文件名、token id、n_vq、生成默认值全部 hardcode 错值 |
| Builtin voices 的 `prompt_audio_codes`                                                                              | 只有 name 和 path，没加载                                                                      | 即使开"Voice = Junhao" 也会走空                          |

### 2.3 已实现但有"语义错误"（黄色）
| 模块                                           | 问题                                                                                                                                                                   | 严重度                                                                                                   |
| ---------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| `text/tokenizer.rs`                            | 用 `tokenizers::Tokenizer::from_file(model_path)` 加载 SentencePiece `tokenizer.model`（protobuf）。HF tokenizers crate 不支持原生 SP `.model`，会运行时报错或行为不符 | **致命**                                                                                                 |
| `provider/onnx.rs::from_dirs`                  | 硬编码 `N_VQ=4`、`AUDIO_ASSISTANT_SLOT=1024`、`AUDIO_END=1025`、`AUDIO_PAD=0`                                                                                          | 致命：MOSS-Audio-Tokenizer-Nano 是 **16 codebooks**；token id 必须从 manifest 读取                       |
| `model/global.rs::load`                        | 期待文件名 `prefill.onnx` / `decode_step.onnx`                                                                                                                         | 实际 HF 文件名按 `tts_meta["files"]["prefill"]` 来；可能不一致                                           |
| `model/local.rs::load`                         | 期待 `decoder.onnx` / `local_cached_step.onnx` / `local_fixed_sampled_frame.onnx`；**未加载 `local_greedy_frame.onnx`**                                                | 缺 sample_mode=greedy 路径                                                                               |
| `inference/prefill.rs`                         | `input_ids` 用 `f32` 且 shape `[1, seq_len, 1]`                                                                                                                        | 真实需求：`int32`、shape `[1, seq_len, n_vq+1]`（带 audio pad 的多列结构）                               |
| `text/chunker.rs::split_text_into_chunks`      | 只有"句末分割 + 贪心"两段；缺：子句级 fallback、token-budget 二分、CJK 感知拼接、单 chunk fallback 整段、句末标点自动补齐                                              | 中：长文本切块会跑偏                                                                                     |
| `audio/wav.rs`                                 | 只覆盖 16-bit PCM WAV；不支持 MP3、不支持重采样、不支持声道转换                                                                                                        | 中：内置 `jp_1.mp3` voice 无法加载，用户传 16k mono 也用不了                                             |
| `inference/streaming.rs::compute_lead_seconds` | 缺自适应 batch budget `_resolve_stream_decode_frame_budget`                                                                                                            | 中：流式首音延迟 / 中段批量两端不能兼顾                                                                  |
| `config.rs::GenerationParams`                  | 缺 `sample_mode`（"greedy" / "fixed" / "full"）枚举、缺 `realtime_streaming_decode` 开关                                                                               | 中：等价于强制 fixed 模式                                                                                |
| `commands/tts.rs` 流式接口                     | 只用 `mpsc` + 轮询 `tts_stream_status`；没用 `tauri::ipc::Channel<T>` 推送                                                                                             | 中：~100 ms 轮询会增加首音延迟，UX 不流畅                                                                |
| `voice/presets.rs`                             | 15 个 voice 写死，但 `audio_data: Vec::new()`，且没有 prebaked `prompt_audio_codes`                                                                                    | 高：开 builtin voice 时还得跑一次 codec_encode（Python 是直接读 manifest 里 prebaked codes，省一次推理） |

---

## 3. 1:1 偏移对照表（完整）

### 3.1 ONNX 推理调用对照
| 步骤                                                                                                       | Python (`ort_cpu_runtime.py`)                          | Rust (`src-tauri/.../tts/`)   | 状态 |
| ---------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ | ----------------------------- | ---- |
| 加载 manifest                                                                                              | `_resolve_manifest_path` + `json.loads(manifest_path)` | **不存在**                    | ❌    |
| 解析 `tts_meta.files.{prefill,decode_step,local_decoder,...}`                                              | `tts_meta["files"][...]`                               | hardcode 文件名               | ❌    |
| 解析 `tts_config.{n_vq, audio_*_token_id, audio_codebook_sizes}`                                           | `manifest["tts_config"]`                               | hardcode `N_VQ=4` 等          | ❌    |
| 解析 `prompt_templates.{user_prompt_prefix, after_reference, assistant_prefix}`                            | `manifest["prompt_templates"]`                         | **不存在**                    | ❌    |
| 解析 `builtin_voices[*].prompt_audio_codes`                                                                | `manifest["builtin_voices"]`                           | 只读 name                     | ❌    |
| `prefill.run(input_ids, attention_mask)`                                                                   | `.run(None, {...})`                                    | `let _input_ids = …; // TODO` | ❌    |
| `decode_step.run(input_ids, past_valid_lengths, past_key_*, past_value_*)`                                 | 完整循环                                               | 类型已定义但无 `.run()`       | ❌    |
| `local_decoder.run(global_hidden, text_token_id, audio_prefix)`                                            | ✅                                                      | **不存在**                    | ❌    |
| `local_cached_step.run(global_hidden, text_id, audio_id, channel_idx, step_type, valid_len, local_past_*)` | ✅                                                      | **不存在**                    | ❌    |
| `local_fixed_sampled_frame.run(global_hidden, repetition_seen_mask, assistant_random_u, audio_random_u)`   | ✅                                                      | **不存在**                    | ❌    |
| `local_greedy_frame.run(global_hidden, repetition_seen_mask, repetition_penalty)`                          | ✅                                                      | **不存在**（连 load 都没有）  | ❌    |
| `codec_encode.run(waveform, input_lengths)`                                                                | ✅                                                      | **不存在**                    | ❌    |
| `codec_decode.run(audio_codes, audio_code_lengths)`                                                        | ✅                                                      | **不存在**                    | ❌    |
| `codec_decode_step.run(audio_codes, state_feeds*)`（流式）                                                 | `CodecStreamingDecodeSession`                          | **不存在**                    | ❌    |
| `build_voice_clone_request_rows()`（行宽 = n_vq+1，pad / slot 编排）                                       | ✅                                                      | **不存在**                    | ❌    |
| `_extract_last_hidden(global_hidden)`                                                                      | `[:, -1, :]`                                           | **不存在**                    | ❌    |
| `_resolve_stream_decode_frame_budget(lead)`                                                                | 1/2/4/8 自适应                                         | 只有 `compute_lead_seconds`   | ❌    |

### 3.2 文本预处理对照
| 步骤                                                                                     | Python                                                      | Rust                                | 状态                                 |
| ---------------------------------------------------------------------------------------- | ----------------------------------------------------------- | ----------------------------------- | ------------------------------------ |
| WeTextProcessing 中文 TN                                                                 | `WeTextProcessingManager.ensure_ready()`                    | 故意跳过（design 决策）             | ⚠️ 可接受                             |
| robust normalizer（30+ regex 规则）                                                      | `tts_robust_normalizer_single_script.py` 450 行             | `text/normalizer.rs` 582 行         | 🟡 实现，需 contract-test 校验        |
| `prepare_synthesis_text(text, voice, prompt_text, ...)` 二段管道                         | `text_normalization_pipeline.py::prepare_tts_request_texts` | 只调用了 `normalize_tts_text(text)` | ⚠️ 缺 voice 名 / prompt_text 处理路径 |
| `_prepare_text_for_sentence_chunking`（CJK 感知 + 句末标点补齐 + 短英文加 8 空格 trick） | ✅                                                           | **不存在**                          | ❌                                    |
| `_split_text_by_punctuation`（带闭合标点 lookahead）                                     | ✅                                                           | 简化为单字符循环                    | 🟡 不等价                             |
| `split_text_by_token_budget`（二分查找 + 优先边界回退 25 字符）                          | ✅                                                           | **不存在**                          | ❌                                    |
| `split_voice_clone_text` 三段式                                                          | sentence → clause → budget；保 `if len > 1 else [text]`     | 简单 sentence + 贪心                | 🟡 长文本切不够好                     |
| `estimate_voice_clone_inter_chunk_pause_seconds`                                         | 短 0.40s / 长 0.24s                                         | **不存在**                          | ❌                                    |

### 3.3 Reference Audio I/O 对照
| 步骤                     | Python                                 | Rust                                          | 状态 |
| ------------------------ | -------------------------------------- | --------------------------------------------- | ---- |
| 加载（wav/mp3/flac/...） | `torchaudio.load(path)`                | `audio/wav.rs::wav_decode` 仅 WAV             | 🟡    |
| 重采样到 48 kHz          | `torchaudio.functional.resample`       | **不存在**                                    | ❌    |
| 单/双声道转换            | mono→stereo 复制 / stereo→mono 平均    | **不存在**                                    | ❌    |
| MP3 解码（jp_1.mp3）     | `torchaudio` 内置                      | **不存在**（`symphonia` 已在 Cargo 但无模块） | ❌    |
| 写 WAV (`hound`)         | `_write_waveform_to_wav` numpy → PCM16 | ✅ `wav_encode`                                | ✅    |

### 3.4 流式播放（前端）对照
| 步骤                                                   | Python `app.py`                                                                      | React `TtsTestPage.tsx`          | 状态 |
| ------------------------------------------------------ | ------------------------------------------------------------------------------------ | -------------------------------- | ---- |
| 启动流（POST `/api/generate-stream/start`）            | ✅                                                                                    | `ttsStreamStart` 已接            | ✅    |
| 拉取 PCM（SSE / chunk）                                | `fetch` Response stream + ReadableStreamDefaultReader                                | **轮询 status**，无真实 PCM 拉取 | ❌    |
| Web Audio API 调度                                     | `AudioContext` + `AudioBufferSourceNode` chained scheduling，维护 `nextPlaybackTime` | **不存在**                       | ❌    |
| 播放高亮（active / played 三态）                       | `setPlaybackHighlight` + `updateRealtimePlaybackHighlightFromLocalClock`             | **不存在**                       | ❌    |
| 暂停/恢复（buffered + streaming 双模式）               | ✅                                                                                    | **不存在**                       | ❌    |
| Stream metrics（emitted / lead / first audio latency） | ✅                                                                                    | **不存在**                       | ❌    |
| 模型未下载时的引导态                                   | N/A（Python 自动下载）                                                               | ✅ `tts_model_download_status`    | ✅    |

### 3.5 Tauri 命令对照（设计 → 实现）
| 设计文档命令                                                                  | 实现          | 备注                                                |
| ----------------------------------------------------------------------------- | ------------- | --------------------------------------------------- |
| `tts_health`                                                                  | ✅             | 但因为 provider 是 mock，永远返回 ready 或假态      |
| `tts_warmup_status` / `tts_start_warmup`                                      | ✅             | warmup 实质是跑 placeholder synthesis，~ms 级"完成" |
| `tts_synthesize`                                                              | ✅             | 返回静音 WAV                                        |
| `tts_stream_start` / `_status` / `_result` / `_close`                         | ✅             | 空 PCM 流 + 时长虚标                                |
| `tts_demo_audio`                                                              | ✅             | 但 `resolve_demo_audio_path` 依赖 voice 文件存在    |
| `tts_list_voices`                                                             | ✅             |                                                     |
| `tts_split_text`                                                              | ✅             | 用简化 chunker                                      |
| `tts_model_status` / `tts_model_download_start` / `tts_model_download_status` | ✅（命令存在） | HF 下载链路需独立验证                               |

---

## 4. UX 与产品视角（资深 UI/UX 设计师视角）

### 4.1 当前 TTS 测试页（`TtsTestPage.tsx`）的优点
- 信息层级清晰：左输入 / 右输出双栏，符合"创作-反馈"的对照心智
- 29 个 demo 内嵌（zh/en/ja/de/fr/es/ko/ru/it/ar/pl/pt/cs/da/sv/el/tr/hu）—— 多语种心智强
- `SectionLabel` 微排版（10.5px uppercase tracking-widest 黑度 30%）—— 安静、有 Apple 系产品味
- 模型未就绪时有引导态（`ttsModelStatus + ttsModelDownloadStart`）

### 4.2 必须补齐的 UX 缺口（按优先级）
| P      | 项                                            | 现状             | 期望                                                                                               |
| ------ | --------------------------------------------- | ---------------- | -------------------------------------------------------------------------------------------------- |
| **P0** | 点击 Generate 后听到的是真音频，不是静音      | 静音             | 真实 voice clone / preset 输出                                                                     |
| **P0** | 首音延迟可见（first-audio-latency）           | 不显示           | 流式开始播第一秒前显示一个 "首音 0.6s" 计时；播放后变 "lead +0.4s"                                 |
| **P0** | Stream 模式下逐句播放高亮                     | 无               | 当前 chunk 高亮、已播变灰；点击高亮可跳转                                                          |
| **P1** | 暂停/恢复在 buffered + streaming 都能工作     | 无               | 一个 toggle 同时控制两种模式；streaming 暂停 = `audioContext.suspend()`                            |
| **P1** | 参数面板分主次                                | 18 个参数全平铺  | 主面板只露 5 个（voice / max_new_frames / temperature / streaming / seed），其余进 "Advanced" 折叠 |
| **P1** | 文本归一化预览（normalized vs original 对照） | 文档列出但未显示 | 右侧出 normalized text 块，hover 时显示 diff                                                       |
| **P2** | 模型下载进度可视化                            | 命令存在         | 进度条 + 速度 + ETA + 取消                                                                         |
| **P2** | 错误状态结构化呈现                            | toast 一句       | 错误代码 + 文档链接 + "重试 / 切换 mock" 双按钮                                                    |
| **P2** | 移动端 / 窄屏自适应                           | 双栏写死         | 768px 以下变上下两段                                                                               |
| **P3** | 性能 dashboard                                | 无               | 显示 CPU 占用、threads、RTF（real-time factor），帮用户调 cpu_threads                              |

### 4.3 设计原则建议（写进 design-doc）
1. **"听到才算成功"原则**：在端到端跑通真实音频前，所有 UI 都标"PREVIEW"，不要给用户错觉
2. **对每一个 ONNX 阶段加 telemetry**：prefill/decode/local/codec 各自的 wall time 全部 emit 出来，UI 可以做 stage-level 进度
3. **流式播放是"承诺"，不是 "feature"**：首音 < 1s 是这个 0.1B 模型的核心卖点，UX 必须把它显化（"实时 0.6s 首音 ✓"）
4. **Voice clone 上传体验**：拖拽 + 实时波形 + 时长校验（< 30s 提示警告，因为 voice clone 用 prompt 越短越能避免长程偏移）

---

## 5. 风险盘点与未爆雷

| #   | 风险                                                                       | 影响                                   | 提前规避                                                                                                                                                                                    |
| --- | -------------------------------------------------------------------------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| R1  | `tokenizers` crate 不支持 SentencePiece `.model`                           | tokenizer 加载失败/编码不一致          | 改用 [`sentencepiece` crate](https://crates.io/crates/sentencepiece) 或 [`tokenizers` 加上 `sentencepiece` feature 后用 `convert_slow.py` 预转 JSON]，做 1 个 50 行 POC 在 TTS-3.1 之前完成 |
| R2  | `ort` crate 2.x KV cache tensor 来回喂                                     | 推理跑不通                             | 第一个 ONNX slice 做 50 行 POC：prefill → 拿 present_key_0 → 改名 past_key_0 → 喂 decode_step，确认 tensor 来回路径 OK                                                                      |
| R3  | ONNX 文件名不一致（`prefill.onnx` vs `moss_tts_prefill.onnx`）             | `ModelNotFound` 报错                   | 必须实现 manifest 解析；先 `tts_meta_path.parent.join(tts_meta["files"]["prefill"])`                                                                                                        |
| R4  | `n_vq=4` vs 真实 16 / token id hardcode 错                                 | 即使 ONNX 跑了输出也是噪音             | manifest 解析 + assertion 防御                                                                                                                                                              |
| R5  | `numpy.random.default_rng(seed)` 与 `rand::StdRng::seed_from_u64` 不同算法 | 同 seed 生成不可复现                   | 接受不一致；UI 不承诺"跨语言可复现"；只承诺"同实现可复现"                                                                                                                                   |
| R6  | torchaudio resample 算法（sinc / kaiser_window）vs Rust `rubato`/`dasp`    | 不同采样实现导致 prompt embedding 漂移 | 用 `rubato` 的 `SincFixedIn` + `WindowFunction::BlackmanHarris2` 接近 torchaudio 默认；做 frame-level codec encode 对比测试                                                                 |
| R7  | Tauri Channel<T> + 高频小包                                                | UI 卡顿                                | 把 PCM chunk 限制在 ≥ 80ms / chunk（80ms × 48kHz × 2ch × i16 = ~30KB），保持 < 12 chunks/s                                                                                                  |
| R8  | macOS notarization & ONNX runtime dylib 签名                               | 用户首次启动报"未受信任"               | tauri.conf 的 `bundle.macOS` 配 codeSignIdentity；ONNX `onnxruntime.dylib` 走 `tauri-plugin-shell` 还是嵌入需提前定                                                                         |
| R9  | 模型 ~700MB 首次下载体验                                                   | 用户 3-5 分钟卡白屏                    | 进度 + 可断点续传（HF mirror 支持 Range）+ 本地缓存复用                                                                                                                                     |
| R10 | 模型常驻内存 ~ 1.5-2 GB                                                    | 与 Agent / Memory 模块抢内存           | 增加 "TTS Idle Eviction"：N 秒无请求自动 unload，下次请求先 fast warmup                                                                                                                     |

---

## 6. 当前 Rust 架构哪里值得保留 / 哪里要重构

### 6.1 保留（不动）
- `mod.rs` traits / SynthesisParams / SynthesisResult / StreamResult — 设计精炼
- `error.rs` — 完全合规
- `manager/warmup.rs`, `manager/jobs.rs` — 异步状态机抽象正确，未来填真实 provider 即可
- `inference/streaming.rs::ChannelAudioSink` — 抽象正确
- `commands/tts.rs` — Response 结构 / Tauri 集成层都对，只是底层是 mock

### 6.2 需要重构
| 项                                    | 原因                                       | 建议                                                                                                           |
| ------------------------------------- | ------------------------------------------ | -------------------------------------------------------------------------------------------------------------- |
| `provider/onnx.rs`                    | 800+ 行的"推理空壳"                        | **拆出 `provider/onnx_real.rs`** 重写；不要在原文件 patch                                                      |
| `model/{global,local,codec}.rs`       | 只做了 session.load，没做 run 方法         | **每个 session 类型各自加 `fn run_*(&mut self, …) -> Result<...>` 方法**；调用者不直接 touch `Session`         |
| `inference/prefill.rs` 与 `decode.rs` | placeholder 混在真实接口里                 | 删除所有占位 return；改成 `unimplemented!("slice TTS-3.x")` 直到真实化（违反 CLAUDE.md，但比"假装跑通"更安全） |
| `text/tokenizer.rs`                   | 用错 crate                                 | 切换 `sentencepiece` crate；保持公开 API 不变                                                                  |
| `text/chunker.rs`                     | 算法不等价                                 | 1:1 翻译 Python `split_voice_clone_text` 三段式                                                                |
| `audio/` 缺 reference loader          | 模块缺失                                   | 新增 `audio/reference.rs`（symphonia 解码 + rubato 重采样 + 通道转换）                                         |
| `config.rs::GenerationParams`         | 缺 sample_mode / realtime_streaming_decode | 新增字段 + serde rename_all camelCase 与前端一致                                                               |

### 6.3 新增模块建议
```
src-tauri/src/modules/tts/
├── manifest/                  ← 新增
│   ├── mod.rs
│   ├── browser_poc.rs         ← parse browser_poc_manifest.json
│   ├── tts_meta.rs            ← parse tts_browser_onnx_meta.json
│   └── codec_meta.rs          ← parse codec_browser_onnx_meta.json
├── audio/
│   ├── reference.rs           ← 新增（symphonia + rubato）
│   └── streaming_decoder.rs   ← 新增（CodecStreamingDecodeSession 等价）
├── inference/
│   ├── request_builder.rs     ← 新增（build_voice_clone_request_rows）
│   ├── local_runner.rs        ← 新增（run_local_decoder/cached/fixed/greedy）
│   └── stream_budget.rs       ← 新增（_resolve_stream_decode_frame_budget）
└── provider/
    └── onnx_real.rs           ← 替换现 onnx.rs
```

---

## 7. 与"离线、实时"目标的对齐度

| 维度                  | 目标                           | 当前                                                 | 达标？               |
| --------------------- | ------------------------------ | ---------------------------------------------------- | -------------------- |
| **离线**（无云）      | 100% local                     | mock 是 local；真实路径未跑通；HF 下载是必要一次联网 | 🟡 一次性下载后离线   |
| **实时**（首音 < 1s） | 0.6-1.0s on M-series CPU       | placeholder 直接返回 0ms；真路径 0%                  | ❌ 待真路径跑通后再测 |
| **流式 PCM**          | < 200ms 块、Web Audio gapless  | 有 `mpsc` 框架，但前端无 Web Audio 调度              | ❌                    |
| **多语言**            | 20 语种                        | 文本归一化已有；29 demo UI 已多语                    | 🟡 验证待真模型       |
| **Voice clone**       | reference audio → cloned voice | 0%                                                   | ❌                    |
| **多 voice preset**   | 15 个内置                      | 名单存在，无音频/无 prebaked codes                   | 🟡 待加载             |
| **CPU 友好**          | 4 核流畅                       | 配置有 `DEFAULT_THREAD_COUNT=4`                      | 🟡                    |
| **跨平台**            | macOS/Win/Linux                | 依赖 ort + symphonia + rubato，全跨平台 OK           | ✅                    |

---

## 8. 必须补齐的工作量估算

> 假设 1 名熟悉 Rust + ONNX 的 senior 开发者全职投入。

| 工作模块                                                                       | 估时       | 优先级 |
| ------------------------------------------------------------------------------ | ---------- | ------ |
| Manifest / meta 解析（browser_poc + tts_meta + codec_meta）                    | 1.0 d      | P0     |
| Tokenizer 改用 `sentencepiece` crate + 行为契约测试                            | 0.5 d      | P0     |
| Reference audio loader（symphonia + rubato + channel ops）                     | 1.5 d      | P0     |
| Codec encode/decode_full ONNX 真实调用 + tensor shape 测试                     | 1.0 d      | P0     |
| Prefill ONNX 真实调用 + KV cache 来回喂 POC                                    | 1.0 d      | P0     |
| Decode_step ONNX 真实调用 + 自回归循环                                         | 2.0 d      | P0     |
| Local sessions 4 个 run 方法（decoder/cached_step/fixed_sampled/greedy_frame） | 2.5 d      | P0     |
| `build_voice_clone_request_rows` 协议组装                                      | 0.5 d      | P0     |
| Voice clone end-to-end（chunk → 拼接 → 输出 WAV）                              | 1.0 d      | P0     |
| 三段式 chunker + inter-chunk pause                                             | 1.0 d      | P1     |
| `CodecStreamingDecodeSession` + 自适应 batch budget                            | 2.0 d      | P1     |
| 流式 PCM 通过 `tauri::ipc::Channel<T>` 推送                                    | 1.0 d      | P1     |
| Builtin voices 加载 prebaked `prompt_audio_codes` from manifest                | 0.5 d      | P1     |
| 前端 Web Audio API 流式调度（gapless）                                         | 1.5 d      | P1     |
| 前端播放高亮（buffered + streaming）                                           | 1.0 d      | P2     |
| 前端暂停/恢复 / Stream metrics 显示                                            | 0.5 d      | P2     |
| 端到端 QA + Python 输出比对                                                    | 2.0 d      | P0     |
| **小计**                                                                       | **20.5 d** |        |

→ **约 4 周一人单独跑可以达到"voice clone 真实可听 + 流式实时"**。

---

## 9. 推荐的 Phase 切分（可直接落到 `docs/exec-plans/active/`）

### Phase TTS-A: 推理底座修复（P0，必须先做）
| Slice   | 任务                                                                       | 验收                                                    |
| ------- | -------------------------------------------------------------------------- | ------------------------------------------------------- |
| TTS-A.1 | 实现 `manifest/` 三个 parser；`OnnxTtsProvider::from_dirs` 改为读 manifest | unit test：能从测试 fixture 读出 n_vq、token id、文件名 |
| TTS-A.2 | 切换 `text/tokenizer.rs` 到 `sentencepiece` crate                          | 5 条多语种 round-trip 一致                              |
| TTS-A.3 | `model/{global,local,codec}.rs` 加 `run_*` 方法（薄包装）                  | 加载真模型 → dump I/O names = Python 期待               |
| TTS-A.4 | `audio/reference.rs`（symphonia + rubato）                                 | 加载 wav/mp3 → 48k stereo f32 → shape 一致              |
| TTS-A.5 | `inference/request_builder.rs::build_voice_clone_request_rows`             | 与 Python 同 prompt 比对，逐 row 一致                   |

### Phase TTS-B: 端到端 Voice Clone（P0）
| Slice   | 任务                                                    | 验收                                               |
| ------- | ------------------------------------------------------- | -------------------------------------------------- |
| TTS-B.1 | Prefill ONNX 真实调用 + KV cache extract                | 输出 `global_hidden.shape == [1, hidden]`          |
| TTS-B.2 | Decode_step 自回归循环（先用 local_decoder 路径，最简） | 1 chunk 跑通 → 拿到 generated_frames               |
| TTS-B.3 | Codec_decode_full → PCM → WAV                           | 写出 WAV 在 macOS Preview 能播                     |
| TTS-B.4 | Voice clone：codec_encode prompt + 拼接 + chunker       | 用 `assets/audio/zh_1.wav` 跑出 vs Python 听感一致 |

### Phase TTS-C: 实时流式（P1）
| Slice   | 任务                                                                                  | 验收                                     |
| ------- | ------------------------------------------------------------------------------------- | ---------------------------------------- |
| TTS-C.1 | `local_fixed_sampled_frame` + `local_greedy_frame` + `local_cached_step` 三种采样路径 | sample_mode 三种值都跑得通               |
| TTS-C.2 | `audio/streaming_decoder.rs` Codec streaming session（带 state_feeds）                | 增量 decode 输出 gapless PCM             |
| TTS-C.3 | `_resolve_stream_decode_frame_budget` 自适应批量                                      | 首音 < 1s（M-series），中段 throughput ↑ |
| TTS-C.4 | `tauri::ipc::Channel<T>` 推送 PCM                                                     | 前端 console 收到 chunk 时间序列         |
| TTS-C.5 | 前端 `WebAudioStreamPlayer` 类（gapless + suspend/resume）                            | 听感无 click/gap                         |

### Phase TTS-D: 体验打磨（P2）
| Slice   | 任务                                               | 验收                      |
| ------- | -------------------------------------------------- | ------------------------- |
| TTS-D.1 | 播放高亮（active / played / pending 三态）         | 跟随 chunk_index 自动滚动 |
| TTS-D.2 | Stream metrics（first audio latency / lead / RTF） | 显示在 UI 上方            |
| TTS-D.3 | 参数面板分 Basic / Advanced + 18 参数实时校验      | hover 提示 + 边界报错     |
| TTS-D.4 | 模型下载进度 / 错误体验改造                        | 进度条 + ETA + 重试       |
| TTS-D.5 | TTS Idle Eviction（5 分钟无请求 unload sessions）  | RSS 内存释放可观察        |

### Phase TTS-E: QA & 离线分发（P1）
| Slice   | 任务                                              | 验收                      |
| ------- | ------------------------------------------------- | ------------------------- |
| TTS-E.1 | Python ↔ Rust 同 prompt+seed 输出 PCM-MSE 比对    | MSE < 0.01 或感知一致     |
| TTS-E.2 | macOS / Windows / Linux 跨平台 smoke              | 三端都跑出 WAV            |
| TTS-E.3 | 性能基线：M4 Air 单核首音 < 1.0s、4 核 RTF > 1.5x | 报告写入 QUALITY_SCORE.md |

---

## 10. 关键决策清单（建议在动手前一次性拍板）

1. **WeTextProcessing 是否纳入**？ — 现状跳过；建议保持跳过，必要时通过 sidecar Python 进程 lazy 调用，不污染主推理路径
2. **SentencePiece crate 选哪个**？ — 推荐 [`sentencepiece` 0.11+](https://crates.io/crates/sentencepiece)（FFI 到官方 C++），跨平台预编译可用
3. **重采样 crate 选哪个**？ — 推荐 [`rubato`](https://crates.io/crates/rubato)（高质量 Sinc）；`dasp` 适合简单 ratio 但音质不如 rubato
4. **流式推送通道**？ — 推荐 `tauri::ipc::Channel<TtsAudioEvent>`（push）+ 状态 polling 兜底；不要纯 polling
5. **模型缓存路径**？ — `tauri::Manager::path().app_data_dir()/models/tts/`（不要 `~/.if2ai`，违反 Tauri 沙箱）；与现 `tts_download.rs` 保持一致
6. **离线包装**？ — 阶段 1：HF 首次下载；阶段 2：可选打包到 `resources/`（增加 ~830MB 安装包但完全离线）
7. **是否做 Metal / CoreML 加速**？ — Phase TTS-F（后续）；先把 CPU 路径做对
8. **Voice clone 的"提示音频时长上限"** UI 提示？ — 建议 ≤ 30s；超过给 warning（长 prompt 影响 voice 稳定性）
9. **多请求并发策略**？ — 单实例 `OnnxTtsProvider` + `tokio::sync::Mutex` 排队；不做请求级并发（ONNX session 非线程安全 + 内存压力）
10. **Mock 在 production 是否保留**？ — 保留，作为 e2e 测试和首次启动 model 未下载时的占位；UI 必须显示 "MOCK MODE" 横幅

---

## 11. 总结（一页话）

if2Ai 的 TTS 模块在**架构与产品形态**上已经做到 senior 水准——traits 干净、状态机清晰、Tauri 命令齐全、Settings UI 信息层级正确。

但**核心推理 0% 真实跑通**：所有 ONNX `.run()` 都是 `// TODO`，tokenizer 用错 crate，token id / n_vq 是瞎写的常量，没有 manifest 解析，没有 reference audio I/O，没有 codec 流式状态机，前端没有 Web Audio gapless 播放。

距离"离线实时 voice clone"还有约 **4 周一人** 的工作量（详见第 8、9 节），但**没有方向性返工**：保持现有 traits，按 Phase TTS-A → B → C → D → E 顺序填实、改 tokenizer、补 ~600 行真实 ONNX 调用、加 ~400 行 reference/重采样/流式 codec、改 ~300 行前端 Web Audio 调度即可。

**第一步建议**：拉一个 50 行的 spike，验证两件事——(a) `sentencepiece` crate 能正确解 `tokenizer.model`；(b) `ort 2.x` 能把 prefill 的 `present_key_0` tensor 改名为 `past_key_0` 喂回 decode_step。这两件事 work，整条路径就 work。

> *— Staff 系统架构师 + 资深 UI/UX 设计师*
> *2026-04-19*
