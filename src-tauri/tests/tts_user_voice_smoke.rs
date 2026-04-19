//! Phase TTS-E.4：用户上传声纹流程的 smoke test。
//!
//! 验证：
//! 1. `voice/registry.rs` 能从临时目录扫到自定义 voice
//! 2. sidecar `<stem>.json` 正确读出 display_name
//! 3. `bundled` voice 与 `user` voice 能共存且按 kind 分类
//!
//! 不依赖真模型：只测 registry 扫描行为。

use std::path::PathBuf;

use if2ai_backend::modules::tts::voice::registry::{VoiceKind, VoiceRegistry};

/// 把 `bytes` 写到 `dir/<filename>`。
fn write_file(dir: &std::path::Path, filename: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.join(filename);
    std::fs::write(&p, bytes).expect("write test file");
    p
}

#[test]
fn user_voice_with_sidecar_metadata_picked_up() {
    let user_dir = tempfile::tempdir().unwrap();
    // 模拟用户上传：sjl.mp3 + sjl.json
    write_file(user_dir.path(), "sjl.mp3", b"fake mp3 content");
    write_file(
        user_dir.path(),
        "sjl.json",
        r#"{"display_name": "孙俊雷的声音"}"#.as_bytes(),
    );

    // 模拟 bundled：把另一个 wav 放到独立的 resource_dir
    let bundled_dir = tempfile::tempdir().unwrap();
    write_file(bundled_dir.path(), "demo_voice.wav", b"fake wav content");

    let registry = VoiceRegistry::scan(
        &[("Junhao", "Junhao")],
        Some(bundled_dir.path()),
        Some(user_dir.path()),
        None,
    );

    let sjl = registry
        .find("sjl")
        .expect("sjl should be found in user dir");
    assert_eq!(sjl.kind, VoiceKind::UserUploaded);
    assert_eq!(
        sjl.display_name, "孙俊雷的声音",
        "应使用 sidecar 的 display_name"
    );
    assert!(sjl.audio_path.is_some());

    let demo = registry
        .find("demo_voice")
        .expect("demo_voice should be bundled");
    assert_eq!(demo.kind, VoiceKind::Bundled);

    let junhao = registry.find("Junhao").expect("Junhao always builtin");
    assert_eq!(junhao.kind, VoiceKind::Builtin);
}

#[test]
fn user_voice_without_sidecar_falls_back_to_prettify() {
    let user_dir = tempfile::tempdir().unwrap();
    write_file(user_dir.path(), "myrecording.wav", b"fake");
    let registry = VoiceRegistry::scan(&[], None, Some(user_dir.path()), None);
    let v = registry.find("myrecording").unwrap();
    assert_eq!(v.display_name, "myrecording"); // 非 zh_/en_/jp_ 前缀直接 stem
    assert_eq!(v.kind, VoiceKind::UserUploaded);
}

#[test]
fn user_voice_m4a_extension_supported() {
    let user_dir = tempfile::tempdir().unwrap();
    write_file(user_dir.path(), "memo.m4a", b"fake");
    let registry = VoiceRegistry::scan(&[], None, Some(user_dir.path()), None);
    assert!(registry.find("memo").is_some(), ".m4a should be recognized");
}

#[test]
fn invalid_extension_ignored() {
    let user_dir = tempfile::tempdir().unwrap();
    write_file(user_dir.path(), "notes.txt", b"fake");
    write_file(user_dir.path(), "data.json", b"{}");
    let registry = VoiceRegistry::scan(&[], None, Some(user_dir.path()), None);
    assert!(
        registry.find("notes").is_none(),
        ".txt should not be a voice"
    );
    assert!(
        registry.find("data").is_none(),
        "stray json should not be a voice"
    );
}
