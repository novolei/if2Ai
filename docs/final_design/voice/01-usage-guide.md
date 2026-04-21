# 📚 Voice 使用指南

> TTS 文字转语音 + STT 语音转文字 —— If2Ai 核心差异化功能的使用手册。

## 🗣️ TTS 文字转语音

### 内置声音

If2Ai 内置 18 个 MOSS-TTS-Nano 声音预设，覆盖中英日多语言：

| 类型 | 数量 | 说明 |
|------|------|------|
| Builtin（内置） | 18 | Manifest 预编码，合成直接使用 prompt_audio_codes |
| Bundled（打包自定义） | 可扩展 | 应用 release 带的 `resources/voices/*.{wav,mp3}` |
| UserUploaded（用户上传） | 可扩展 | 用户上传到 `~/.if2ai/voices/` |

内置声音使用 prebaked `prompt_audio_codes`，合成时无需加载原始 WAV 文件，首次延迟更低。

### 声音克隆

提供一段参考音频，TTS 即可克隆该声音：

1. 准备参考音频（WAV / MP3，建议 5-15 秒清晰语音）
2. 选择 VoiceClone 模式
3. 输入待合成文本
4. 系统自动使用参考音频的声音特征生成语音

```rust
// 源码参考：src-tauri/src/modules/tts/mod.rs
pub struct SynthesisParams {
    pub text: String,                      // 待合成文本
    pub mode: SynthesisMode::VoiceClone,   // 声音克隆模式
    pub voice: Option<String>,             // 声音预设名
    pub prompt_audio_path: Option<PathBuf>,// 参考音频路径
    pub generation: GenerationParams,      // 生成参数
}
```

### 流式播放

TTS 支持流式合成，边生成边播放：

- 音频按 chunk 逐步推送到 `AudioSink`
- 首音延迟（TTFA）通常 < 200ms
- 实时因子 > 1 表示推理速度超过播放速度

```rust
// 源码参考：src-tauri/src/modules/tts/mod.rs
pub struct StreamResult {
    pub first_audio_latency_seconds: f32,  // 首音延迟
    pub realtime_factor: f32,              // 实时因子
    pub emitted_audio_seconds: f32,        // 已发出音频时长
    pub lead_seconds: f32,                 // 缓冲提前量
}
```

### 续写模式

Continuation 模式从已有音频 + 文本对续写语音：

- 输入：prompt_audio（参考音频）+ prompt_text（参考文本）+ 待续写文本
- 输出：在参考音频风格基础上续写的完整语音

## 🎤 STT 语音转文字

### 自动语言检测

SenseVoice 引擎支持自动语言识别，无需手动指定：

- 中文（zh）/ 英文（en）自动检测
- 转写结果包含检测到的语言代码

### 语音输入流程

```
前端录音（MediaRecorder PCM16LE）
    → base64 编码
    → stt_transcribe Tauri 命令
    → 解码 bytes → f32
    → 自动重采样到 16kHz
    → SenseVoice ONNX 推理
    → 返回 TranscribeResult { text, language, elapsed }
```

### 转写结果

```rust
// 源码参考：src-tauri/src/modules/stt/mod.rs
pub struct TranscribeResult {
    pub text: String,              // 转写文本
    pub language: String,          // 检测到的语言（"zh" / "en"）
    pub elapsed_seconds: f32,      // 耗时（秒）
}
```

## ⚙️ 语音交互模式配置

### TTS 设置

| 设置项 | 说明 | 选项 |
|--------|------|------|
| 质量预设 | 合成质量 vs 速度 | 高质量 / 均衡 / 快速 |
| 默认声音 | 默认使用的声音预设 | 18 个内置 + 自定义 |
| 语速 | 语音播放速度 | 0.5x - 2.0x |
| 采样策略 | 文本生成采样方法 | Top-k / Top-p / 贪心 |

源码参考：[`src-tauri/src/modules/tts/settings.rs`](../../../src-tauri/src/modules/tts/settings.rs)

### TTS 质量预设

```rust
// 源码参考：src-tauri/src/modules/tts/settings.rs
pub enum TtsQualityPreset {
    HighQuality,   // 高质量：大 batch、多采样轮次
    Balanced,      // 均衡：中等参数
    Fast,          // 快速：小 batch、贪心采样
}
```

### 语音模式

If2Ai 支持三种语音交互模式：

| 模式 | 说明 |
|------|------|
| 仅 TTS | AI 回复自动转为语音播放 |
| 仅 STT | 用户语音输入转为文本 |
| 双向 | TTS + STT 完整语音交互 |

### STT 设置

| 设置项 | 说明 | 默认值 |
|--------|------|--------|
| 自动语言检测 | 是否自动检测语言 | 开启 |
| 采样率 | 输入音频采样率 | 16kHz（自动重采样） |
| VAD 灵敏度 | 语音活动检测灵敏度 | 中等 |

## 📦 模型下载与管理

### TTS 模型

首次使用时自动从 HuggingFace 下载并缓存：

| 模型 | 参数量 | 缓存路径 | 说明 |
|------|--------|----------|------|
| MOSS-TTS-Nano-100M-ONNX | ~100M | `~/.if2ai/models/tts/` | 主 TTS 模型 |
| MOSS-Audio-Tokenizer-Nano-ONNX | ~20M | `~/.if2ai/models/tts/` | 音频分词器 |

模型文件结构：
```
~/.if2ai/models/tts/
├── prefill.onnx          # Prefill 阶段权重
├── decode_step.onnx      # Decode 阶段权重
├── browser_poc_manifest.json  # 18 个内置声音的 prompt_audio_codes
└── ...
```

源码参考：[`src-tauri/src/modules/tts/model/downloader.rs`](../../../src-tauri/src/modules/tts/model/downloader.rs)

### STT 模型

SenseVoice ONNX 模型自动下载到 `~/.if2ai/models/sensevoice/`：

| 文件 | 说明 |
|------|------|
| `model.onnx` | ONNX 权重（量化版，约 230MB） |
| `tokens.json` | CTC 词表 |
| `am.mvn` | CMVN 归一化参数 |
| `config.yaml` | 元信息 |

### 模型就绪检查

```rust
// 源码参考：src-tauri/src/modules/stt/openflow/engine.rs
pub fn model_is_ready(dir: &Path) -> bool {
    let has_model = dir.join("model.onnx").exists() || dir.join("model_quant.onnx").exists();
    let has_tokens = dir.join("tokens.json").exists();
    has_model && has_tokens
}
```

### 模型下载器

STT 模型下载器（`stt/openflow/downloader.rs`）支持：

- 从 HuggingFace 下载 SenseVoice ONNX 模型
- 断点续传
- 下载进度回调
- 模型完整性校验

TTS 模型下载器（`tts/model/downloader.rs`）同理：

- MOSS-TTS-Nano-100M-ONNX 主模型
- MOSS-Audio-Tokenizer-Nano-ONNX 分词器
- 首次使用自动触发下载

## 📝 核心概念速查表

| 操作 | 接口 | 关键文件 |
|------|------|----------|
| TTS 合成 | `TtsProvider.synthesize()` | `tts/mod.rs` |
| TTS 流式 | `TtsProvider.synthesize_stream()` | `tts/inference/streaming.rs` |
| 声音克隆 | `SynthesisMode::VoiceClone` | `tts/mod.rs` |
| 列出声音 | `TtsProvider.list_voices()` | `tts/voice/registry.rs` |
| 试听声音 | `VoiceAsset.is_previewable()` | `tts/voice/registry.rs` |
| STT 转写 | `OpenFlowAsrEngine.transcribe()` | `stt/openflow/engine.rs` |
| 模型下载 | `ensure_models_cached()` | `tts/model/downloader.rs` |

## 🔗 相关资源

- [Voice 实现文档](./02-implementation.md) — 开发者架构详解
- [TTS Provider trait](../../../src-tauri/src/modules/tts/mod.rs)
- [STT Engine](../../../src-tauri/src/modules/stt/openflow/engine.rs)
