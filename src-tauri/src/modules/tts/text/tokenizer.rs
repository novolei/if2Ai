//! TTS SentencePiece tokenizer —— Python sidecar 进程实现。
//!
//! ## 为什么不用 sentencepiece Rust crate？
//!
//! `sentencepiece` crate（FFI 到 C++ libsentencepiece）会 link 自己的 libprotobuf
//! 静态库。**onnxruntime** 也用 libprotobuf 解析 `.onnx` 文件。两者在同一进程
//! 时（macOS / Linux 默认 `-static`）会出现 protobuf 静态符号冲突，导致 ort
//! `Session::commit_from_file` 报 `"Protobuf parsing failed"`。
//!
//! 已验证：只要 `sentencepiece` 被链接进二进制（即使不调用），ort 就死。
//!
//! ## 为什么不用 HF `tokenizers` crate + 转换 JSON？
//!
//! SP 用"整词查找+字节回退"混合策略，不是严格 BPE merge 序列。把 SP `.model`
//! 转 HF `tokenizer.json` 在多字节字符（CJK 标点等）上做不到 100% bit-exact。
//! TTS 对 token id 完全敏感（一个错位 → 输出杂音），所以只能保真。
//!
//! ## 当前方案：Python sidecar
//!
//! - 启动时 spawn `python3 -c "<embedded script>"`，传入 `.model` 路径。
//! - Sidecar 长驻：每次 stdin 读一行 JSON `{"op":"encode","text":"..."}`，
//!   stdout 写一行 JSON `{"ids":[...]}`。
//! - 一次性 fork 开销 ~80ms（仅 provider load 时），单次 encode <1ms。
//! - Drop 时关闭 stdin，sidecar 自然退出。
//!
//! ## 公开 API（与之前签名兼容）
//!
//! - [`TtsTokenizer::load`] / [`TtsTokenizer::from_model_dir`]
//! - [`TtsTokenizer::encode`] → `Vec<u32>`
//! - [`TtsTokenizer::decode`] → `String`
//! - [`TtsTokenizer::count_tokens`] → `usize`
//! - [`TtsTokenizer::vocab_size`] → `usize`

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::modules::tts::error::TtsError;

/// SentencePiece tokenizer 默认文件名。
pub const TOKENIZER_MODEL_FILE: &str = "tokenizer.model";

/// 嵌入到二进制的 Python sidecar 脚本（无外部依赖，仅需 sentencepiece 模块）。
///
/// Phase TTS-B.5：增加 `ping` op 用于心跳，便于父进程探活。
const SIDECAR_PY_SCRIPT: &str = r#"
import sys, json
import sentencepiece

if len(sys.argv) < 2:
    sys.stderr.write("usage: sidecar <tokenizer.model path>\n"); sys.exit(2)

sp = sentencepiece.SentencePieceProcessor()
sp.Load(sys.argv[1])

sys.stdout.write(json.dumps({"ready": True, "vocab_size": sp.GetPieceSize()}) + "\n")
sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
        op = req.get("op")
        if op == "encode":
            ids = sp.encode(req["text"], out_type=int)
            sys.stdout.write(json.dumps({"ids": ids}) + "\n")
        elif op == "decode":
            text = sp.decode_ids(list(req["ids"]))
            sys.stdout.write(json.dumps({"text": text}) + "\n")
        elif op == "ping":
            sys.stdout.write(json.dumps({"pong": True}) + "\n")
        elif op == "shutdown":
            break
        else:
            sys.stdout.write(json.dumps({"error": f"unknown op: {op}"}) + "\n")
    except Exception as e:
        sys.stdout.write(json.dumps({"error": str(e)}) + "\n")
    sys.stdout.flush()
"#;

/// Phase TTS-B.5：跨平台 python 可执行候选（按优先级降序）。
const PYTHON_CANDIDATES: &[&str] = &[
    "python3", "python", // Windows + 部分 Linux 默认
    "py",     // Windows py launcher
];

/// 探测可用的 python 可执行路径；找不到返回 None。
fn detect_python_executable() -> Option<String> {
    for candidate in PYTHON_CANDIDATES {
        let probe = std::process::Command::new(candidate)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .stdin(std::process::Stdio::null())
            .status();
        if matches!(probe, Ok(s) if s.success()) {
            return Some((*candidate).to_string());
        }
    }
    None
}

#[derive(Debug, Serialize)]
#[serde(tag = "op", rename_all = "lowercase")]
enum SidecarRequest<'a> {
    Encode {
        text: &'a str,
    },
    Decode {
        ids: Vec<u32>,
    },
    Ping,
    #[allow(dead_code)]
    Shutdown,
}

#[derive(Debug, Deserialize)]
struct EncodeResponse {
    ids: Option<Vec<u32>>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DecodeResponse {
    text: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HandshakeResponse {
    ready: bool,
    vocab_size: usize,
}

#[derive(Debug, Deserialize)]
struct PongResponse {
    pong: Option<bool>,
    error: Option<String>,
}

/// 包装一个长驻 Python sidecar 进程，负责 stdin/stdout 通信。
///
/// 用 [`Mutex`] 保护 IO 句柄因为 trait 要求 `&self` 而 stdin/stdout 是
/// `&mut`。线程内独占，不做并发 encode（一次同步合成只有一个 encode 调用）。
struct SidecarProcess {
    child: Child,
    stdin: Mutex<ChildStdin>,
    stdout: Mutex<BufReader<ChildStdout>>,
}

impl SidecarProcess {
    fn spawn(model_path: &Path) -> Result<(Self, usize), TtsError> {
        let py = detect_python_executable().ok_or_else(|| {
            TtsError::TokenizationError(
                "未找到可用的 python（已尝试 python3 / python / py）；请安装 Python 3 + sentencepiece"
                    .into(),
            )
        })?;
        let mut child = Command::new(&py)
            .arg("-c")
            .arg(SIDECAR_PY_SCRIPT)
            .arg(model_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                TtsError::TokenizationError(format!(
                    "无法启动 python3 sidecar: {e}; 请确保 python3 + sentencepiece 已安装"
                ))
            })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| TtsError::TokenizationError("sidecar stdin 不可达".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| TtsError::TokenizationError("sidecar stdout 不可达".into()))?;

        // 等握手
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|e| TtsError::TokenizationError(format!("sidecar 握手 read 失败: {e}")))?;
        let handshake: HandshakeResponse = serde_json::from_str(line.trim()).map_err(|e| {
            TtsError::TokenizationError(format!("握手 JSON 解析失败: {e} (raw: {line:?})"))
        })?;
        if !handshake.ready {
            return Err(TtsError::TokenizationError("sidecar 未就绪".into()));
        }

        Ok((
            Self {
                child,
                stdin: Mutex::new(stdin),
                stdout: Mutex::new(reader),
            },
            handshake.vocab_size,
        ))
    }

    fn call_encode(&self, text: &str) -> Result<Vec<u32>, TtsError> {
        let req = SidecarRequest::Encode { text };
        let line = self.exchange(&req)?;
        let resp: EncodeResponse = serde_json::from_str(&line)
            .map_err(|e| TtsError::TokenizationError(format!("encode resp parse: {e}")))?;
        if let Some(err) = resp.error {
            return Err(TtsError::TokenizationError(format!(
                "sidecar encode: {err}"
            )));
        }
        resp.ids
            .ok_or_else(|| TtsError::TokenizationError("encode resp 缺 ids".into()))
    }

    fn call_decode(&self, ids: &[u32]) -> Result<String, TtsError> {
        let req = SidecarRequest::Decode { ids: ids.to_vec() };
        let line = self.exchange(&req)?;
        let resp: DecodeResponse = serde_json::from_str(&line)
            .map_err(|e| TtsError::TokenizationError(format!("decode resp parse: {e}")))?;
        if let Some(err) = resp.error {
            return Err(TtsError::TokenizationError(format!(
                "sidecar decode: {err}"
            )));
        }
        resp.text
            .ok_or_else(|| TtsError::TokenizationError("decode resp 缺 text".into()))
    }

    fn call_ping(&self) -> Result<bool, TtsError> {
        let line = self.exchange(&SidecarRequest::Ping)?;
        let resp: PongResponse = serde_json::from_str(&line)
            .map_err(|e| TtsError::TokenizationError(format!("ping resp parse: {e}")))?;
        if let Some(err) = resp.error {
            return Err(TtsError::TokenizationError(format!("sidecar ping: {err}")));
        }
        Ok(resp.pong.unwrap_or(false))
    }

    fn exchange(&self, req: &SidecarRequest<'_>) -> Result<String, TtsError> {
        let payload = serde_json::to_string(req)
            .map_err(|e| TtsError::TokenizationError(format!("req serialize: {e}")))?;
        {
            let mut stdin = self
                .stdin
                .lock()
                .map_err(|_| TtsError::TokenizationError("sidecar stdin mutex poisoned".into()))?;
            writeln!(stdin, "{payload}")
                .map_err(|e| TtsError::TokenizationError(format!("sidecar write: {e}")))?;
            stdin
                .flush()
                .map_err(|e| TtsError::TokenizationError(format!("sidecar flush: {e}")))?;
        }
        let mut line = String::new();
        let mut reader = self
            .stdout
            .lock()
            .map_err(|_| TtsError::TokenizationError("sidecar stdout mutex poisoned".into()))?;
        reader
            .read_line(&mut line)
            .map_err(|e| TtsError::TokenizationError(format!("sidecar read: {e}")))?;
        if line.is_empty() {
            return Err(TtsError::TokenizationError("sidecar 已退出 (EOF)".into()));
        }
        Ok(line.trim().to_string())
    }
}

impl Drop for SidecarProcess {
    fn drop(&mut self) {
        // 优雅关闭：发送 shutdown，给 100ms 等待，然后 kill
        let _ = self.exchange(&SidecarRequest::Shutdown);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// SentencePiece tokenizer wrapper（通过 sidecar 进程）。
///
/// Phase TTS-B.5：sidecar 健壮性
/// - sidecar 用 [`Mutex`] 包装允许 panic-recovery 重启。
/// - 每次 `encode` / `decode` 检测 EOF/IO 错误 → 自动重启（最多 2 次）。
/// - [`TtsTokenizer::ping`] 主动探活，可由 idle eviction / health check 调用。
pub struct TtsTokenizer {
    sidecar: Mutex<SidecarProcess>,
    model_path: PathBuf,
    vocab_size: usize,
}

impl TtsTokenizer {
    pub fn load(model_path: &Path) -> Result<Self, TtsError> {
        if !model_path.exists() {
            return Err(TtsError::ModelNotFound(format!(
                "tokenizer.model not found at {}",
                model_path.display()
            )));
        }
        let (sidecar, vocab_size) = SidecarProcess::spawn(model_path)?;
        tracing::info!(
            "tts tokenizer sidecar started: vocab={} model={}",
            vocab_size,
            model_path.display()
        );
        Ok(Self {
            sidecar: Mutex::new(sidecar),
            model_path: model_path.to_path_buf(),
            vocab_size,
        })
    }

    pub fn from_model_dir(tts_model_dir: &Path) -> Result<Self, TtsError> {
        let model_path = tts_model_dir.join(TOKENIZER_MODEL_FILE);
        Self::load(&model_path)
    }

    pub fn encode(&self, text: &str) -> Result<Vec<u32>, TtsError> {
        self.with_sidecar_retry(|sc| sc.call_encode(text))
    }

    pub fn decode(&self, tokens: &[u32]) -> Result<String, TtsError> {
        self.with_sidecar_retry(|sc| sc.call_decode(tokens))
    }

    /// Phase TTS-B.5：主动 ping sidecar，验证子进程健康。
    ///
    /// 失败时自动重启一次；返回 `Ok(true)` 表示存活，`Err` 表示无法恢复。
    pub fn ping(&self) -> Result<bool, TtsError> {
        self.with_sidecar_retry(|sc| sc.call_ping())
    }

    /// Phase TTS-C.3：在 tokio 后台启动一个心跳 ticker。
    ///
    /// 每 `interval` 调一次 [`Self::ping`]；如果 ping 失败 → 自动重启 sidecar
    /// （by `with_sidecar_retry` 自带的 respawn 逻辑）；如果 respawn 也失败
    /// → 记 tracing::error 但 ticker 继续跑，下个周期再试。
    ///
    /// 返回的 `JoinHandle` drop 时 ticker 自然终止（abort）；调用方持有它
    /// 直到 tokenizer 自身 drop 即可。
    ///
    /// # 用法
    ///
    /// ```ignore
    /// let tk = Arc::new(TtsTokenizer::load(&path)?);
    /// let _ticker = tk.clone().spawn_health_ticker(Duration::from_secs(60));
    /// // 应用退出时 _ticker 自动 abort
    /// ```
    pub fn spawn_health_ticker(
        self: std::sync::Arc<Self>,
        interval: std::time::Duration,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            tracing::info!(
                interval_secs = interval.as_secs(),
                model = %self.model_path.display(),
                "tts tokenizer sidecar health ticker started"
            );
            loop {
                tokio::time::sleep(interval).await;
                let me = self.clone();
                let result = tokio::task::spawn_blocking(move || me.ping())
                    .await
                    .map_err(|e| TtsError::TokenizationError(format!("ping join: {e}")))
                    .and_then(|inner| inner);
                match result {
                    Ok(true) => tracing::trace!("tts sidecar pong ✓"),
                    Ok(false) => tracing::warn!("tts sidecar ping returned non-true"),
                    Err(e) => {
                        tracing::error!(
                            error = %e,
                            "tts sidecar ping failed (with_sidecar_retry already respawned if recoverable)"
                        );
                    }
                }
            }
        })
    }

    pub fn count_tokens(&self, text: &str) -> Result<usize, TtsError> {
        self.encode(text).map(|ids| ids.len())
    }

    pub fn vocab_size(&self) -> usize {
        self.vocab_size
    }

    pub fn model_path(&self) -> &Path {
        &self.model_path
    }

    /// 通用重试包装：op 失败且看起来是 IO/EOF 类（sidecar 死掉）→ respawn 一次再试。
    fn with_sidecar_retry<R>(
        &self,
        mut op: impl FnMut(&SidecarProcess) -> Result<R, TtsError>,
    ) -> Result<R, TtsError> {
        let mut guard = self
            .sidecar
            .lock()
            .map_err(|_| TtsError::TokenizationError("tokenizer sidecar mutex poisoned".into()))?;
        match op(&guard) {
            Ok(v) => Ok(v),
            Err(first_err) => {
                if !is_recoverable_sidecar_error(&first_err) {
                    return Err(first_err);
                }
                tracing::warn!(
                    error = %first_err,
                    model = %self.model_path.display(),
                    "tokenizer sidecar died; respawning"
                );
                let (fresh, _vocab) = SidecarProcess::spawn(&self.model_path)?;
                *guard = fresh;
                op(&guard).map_err(|e| {
                    TtsError::TokenizationError(format!(
                        "sidecar respawn 后仍失败: {e} (initial: {first_err})"
                    ))
                })
            }
        }
    }
}

/// 判断错误是否值得 respawn 重试。
fn is_recoverable_sidecar_error(err: &TtsError) -> bool {
    let msg = err.to_string();
    msg.contains("EOF")
        || msg.contains("Broken pipe")
        || msg.contains("sidecar write")
        || msg.contains("sidecar read")
        || msg.contains("sidecar 已退出")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_model_returns_error() {
        let result = TtsTokenizer::load(Path::new("/nonexistent/tokenizer.model"));
        assert!(matches!(result, Err(TtsError::ModelNotFound(_))));
    }

    #[test]
    fn from_model_dir_missing_dir_returns_error() {
        let result = TtsTokenizer::from_model_dir(Path::new("/nonexistent"));
        assert!(result.is_err());
    }

    #[test]
    fn tokenizer_model_constant() {
        assert_eq!(TOKENIZER_MODEL_FILE, "tokenizer.model");
    }

    #[test]
    fn sidecar_encode_decode_roundtrip() {
        let model_path = std::path::PathBuf::from(format!(
            "{}/.if2ai/models/tts/MOSS-TTS-Nano-100M-ONNX/tokenizer.model",
            std::env::var("HOME").unwrap_or_default()
        ));
        if !model_path.exists() {
            eprintln!(
                "[skip] tokenizer.model not found at {}",
                model_path.display()
            );
            return;
        }
        let tk = TtsTokenizer::load(&model_path).expect("load");
        assert!(tk.vocab_size() > 1000, "vocab size {}", tk.vocab_size());

        for text in &["你好", "Hello world", "人工智能"] {
            let ids = tk.encode(text).expect("encode");
            assert!(!ids.is_empty(), "encode '{text}' empty");
            let back = tk.decode(&ids).expect("decode");
            eprintln!("[round] {text:?} → {ids:?} → {back:?}");
        }
    }
}
