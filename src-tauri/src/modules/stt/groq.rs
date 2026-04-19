//! Groq Whisper API STT 后端。
#![allow(dead_code)]

use serde::Deserialize;

const GROQ_WHISPER_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";
pub const GROQ_DEFAULT_MODEL: &str = "whisper-large-v3-turbo";

#[derive(Debug, Deserialize)]
struct GroqTranscriptionResponse {
    text: String,
    #[serde(default)]
    language: Option<String>,
}

pub async fn transcribe_via_groq(
    wav_bytes: Vec<u8>,
    api_key: &str,
    language: Option<&str>,
    model: Option<&str>,
) -> Result<crate::modules::stt::TranscribeResult, String> {
    let start = std::time::Instant::now();
    let model_id = model.unwrap_or(GROQ_DEFAULT_MODEL).to_string();

    let part = reqwest::multipart::Part::bytes(wav_bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| format!("groq mime: {e}"))?;
    let mut form = reqwest::multipart::Form::new()
        .text("model", model_id)
        .text("response_format", "json")
        .part("file", part);
    if let Some(lang) = language {
        form = form.text("language", lang.to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("reqwest client: {e}"))?;

    let resp = client
        .post(GROQ_WHISPER_URL)
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("groq request failed: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!(
            "Groq Whisper {status}: {}",
            body.chars().take(300).collect::<String>()
        ));
    }

    let parsed: GroqTranscriptionResponse = resp
        .json()
        .await
        .map_err(|e| format!("groq json parse: {e}"))?;

    Ok(crate::modules::stt::TranscribeResult {
        text: parsed.text.trim().to_string(),
        language: language
            .map(|s| s.to_string())
            .or(parsed.language)
            .unwrap_or_default(),
        elapsed_seconds: start.elapsed().as_secs_f32(),
    })
}

/// 把 PCM16LE bytes 打包成 WAV 字节流（含 RIFF header），方便直接 POST 给 Groq。
pub fn pcm16le_to_wav(pcm: &[u8], sample_rate: u32, channels: u16) -> Vec<u8> {
    let byte_rate = sample_rate * channels as u32 * 2;
    let block_align = channels * 2;
    let data_size = pcm.len() as u32;
    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_size).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_size.to_le_bytes());
    out.extend_from_slice(pcm);
    out
}
