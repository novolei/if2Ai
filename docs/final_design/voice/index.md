# Voice 语音系统

> If2Ai 独有的 TTS + STT 语音交互系统 —— 18 个内置声音、声音克隆、流式合成、自动语言识别。

## 📚 文档目录

| 文件 | 面向 | 内容 |
|------|------|------|
| [01-usage-guide.md](./01-usage-guide.md) | 用户 | TTS/STT 使用方法、声音管理、语音交互模式 |
| [02-implementation.md](./02-implementation.md) | 开发者 | ONNX 推理管线、音频处理、模型加载、流式架构 |

## 📍 核心概念速查

| 概念 | 说明 |
|------|------|
| **TTS** | Text-to-Speech，文字转语音，MOSS-TTS-Nano ONNX 推理 |
| **STT** | Speech-to-Text，语音转文字，SenseVoice ONNX 推理 |
| **VoiceAsset** | 语音资产元数据（id / display_name / kind / audio_path） |
| **VoiceKind** | 语音来源：`Builtin`（18 个内置）/ `Bundled`（打包自定义）/ `UserUploaded`（用户上传） |
| **SynthesisMode** | 合成模式：`VoiceClone`（声音克隆）/ `Continuation`（续写） |
| **AudioSink** | 流式音频接收器，逐 chunk 回调 |
| **AudioPreprocessor** | STT 音频预处理：重采样 + fbank 特征提取 + CMVN 归一化 |
| **ORT** | ONNX Runtime，CPU 推理后端 |

## 🏗️ 源码位置

```
src-tauri/src/modules/
├── tts/                        # ⭐ TTS 模块（~5,500 行）
│   ├── mod.rs                  # 模块入口 + TtsProvider trait
│   ├── config.rs               # 生成参数 + AudioChunk + VoicePreset
│   ├── error.rs                # TtsError 错误类型
│   ├── settings.rs             # TtsSettings + 质量预设
│   ├── profile.rs              # TtsProfile + 文本后处理
│   ├── performance.rs          # SynthesisProfile 性能统计
│   ├── audio/                  # 音频处理
│   │   ├── wav.rs              #    WAV 编解码
│   │   ├── reference.rs        #    参考音频处理
│   │   └── streaming_decoder.rs#    流式音频解码
│   ├── inference/              # ⭐ ONNX 推理管线
│   │   ├── prefill.rs          #    Prefill 阶段
│   │   ├── decode.rs           #    Decode 阶段（自回归）
│   │   ├── sampling.rs         #    采样策略
│   │   ├── streaming.rs        #    流式合成控制
│   │   ├── runner.rs           #    推理运行器
│   │   ├── request_builder.rs  #    请求构建
│   │   └── stream_budget.rs    #    流式预算
│   ├── model/                  # 模型管理
│   │   ├── global.rs           #    ONNX Session 管理
│   │   ├── local.rs            #    本地模型路径
│   │   ├── codec.rs            #    音频编解码器
│   │   ├── downloader.rs       #    HuggingFace 下载
│   │   ├── ort_io.rs           #    ORT 张量 I/O
│   │   └── global.rs           #    全局 Session 池
│   ├── voice/                  # ⭐ 语音资产
│   │   ├── registry.rs         #    VoiceRegistry 3 类来源
│   │   ├── presets.rs          #    内置声音预设
│   │   ├── demo.rs             #    试听演示
│   │   └── mod.rs
│   ├── text/                   # 文本预处理
│   ├── manifest/               # 模型清单
│   ├── provider/               # TtsProvider 实现
│   └── manager/                # TTS 生命周期管理
│
└── stt/                        # ⭐ STT 模块（~1,000 行）
    ├── mod.rs                  # 模块入口 + TranscribeResult
    ├── settings.rs             # STT 设置
    └── openflow/               # OpenFlow/SenseVoice 引擎
        ├── engine.rs           #    OpenFlowAsrEngine 懒加载
        ├── onnx_inference.rs   #    ONNX 推理封装
        ├── decoder.rs          #    CTC 解码器
        ├── preprocess.rs       #    音频预处理（fbank + CMVN）
        ├── downloader.rs       #    模型下载
        └── mod.rs
```

## 🔗 相关链接

- [Security 模块](../security/) — 语音文件路径验证
- [Skills 模块](../skills/) — 语音技能可调用 TTS/STT
- [cc-haha 语音差距分析](./02-implementation.md#️-与-cc-haha-差距分析)
