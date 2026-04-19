//! Integration tests for the multimodal tool output pipeline (Phase 7C, slice 7C.2).
//!
//! Validates end-to-end behaviour without spinning up a real Chrome:
//! 1. Legacy `Result<String, ToolError>` handlers still work and surface as `Text` parts.
//! 2. Per-modality byte caps (`max_text_bytes` / `max_image_bytes`) reject correctly.
//! 3. `ToolResultContentBlock::Image` round-trips through serde and the OpenAI
//!    flatten helper degrades gracefully (image → `[image: <mime> <bytes>B]`).
//! 4. Anthropic-shaped serialisation produces the wire format Claude expects.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::json;

use if2ai_backend::modules::api::{ImageSource, ToolResultContentBlock};
use if2ai_backend::modules::runtime::permissions::PermissionMode;
use if2ai_backend::modules::tools::context::ToolContext;
use if2ai_backend::modules::tools::output::{ToolOutput, ToolResultPart};
use if2ai_backend::modules::tools::registry::{ToolEntry, ToolError, ToolHandler, ToolRegistry};

fn registry() -> ToolRegistry {
    let ctx = ToolContext {
        session_id: None,
        project_id: None,
        workdir: PathBuf::from("."),
        permission_mode: PermissionMode::DangerFullAccess,
    };
    ToolRegistry::new(Arc::new(Mutex::new(ctx)))
}

fn legacy_text_handler(text: &'static str) -> ToolHandler {
    Arc::new(move |_args, _ctx| {
        let owned = text.to_string();
        Box::pin(async move { Ok(owned) })
    })
}

#[tokio::test]
async fn legacy_handler_lifts_to_single_text_part() {
    let r = registry();
    r.register(ToolEntry {
        name: "legacy_text".into(),
        toolset: "test".into(),
        description: "legacy".into(),
        input_schema: json!({"type": "object"}),
        max_result_size: None,
        max_text_bytes: None,
        max_image_bytes: None,
        timeout_secs: None,
        disabled: false,
        handler: legacy_text_handler("hello world"),
        multimodal_handler: None,
    })
    .unwrap();

    let out = r.dispatch("legacy_text", json!({})).await.unwrap();
    assert_eq!(out.parts.len(), 1);
    match &out.parts[0] {
        ToolResultPart::Text { text } => assert_eq!(text, "hello world"),
        _ => panic!("expected text part"),
    }
}

#[tokio::test]
async fn multimodal_handler_returns_image_part() {
    let r = registry();
    let mm: if2ai_backend::modules::tools::registry::ToolHandlerMultimodal =
        Arc::new(|_args, _ctx| {
            Box::pin(async move {
                Ok(ToolOutput::text_then_image(
                    "ctx",
                    "image/jpeg",
                    "BASE64",
                    Some("alt".into()),
                ))
            })
        });

    r.register(ToolEntry {
        name: "vision".into(),
        toolset: "test".into(),
        description: "mm".into(),
        input_schema: json!({"type": "object"}),
        max_result_size: None,
        max_text_bytes: Some(64),
        max_image_bytes: Some(1024),
        timeout_secs: None,
        disabled: false,
        handler: legacy_text_handler("unused"),
        multimodal_handler: Some(mm),
    })
    .unwrap();

    let out = r.dispatch("vision", json!({})).await.unwrap();
    assert_eq!(out.parts.len(), 2);
    assert!(out.has_image());
    assert_eq!(out.text_byte_size(), 3); // "ctx"
    assert_eq!(out.image_byte_size(), "BASE64".len());
}

#[tokio::test]
async fn dispatch_enforces_text_cap_before_image_cap() {
    let r = registry();
    let mm: if2ai_backend::modules::tools::registry::ToolHandlerMultimodal =
        Arc::new(|_args, _ctx| Box::pin(async move { Ok(ToolOutput::text("a".repeat(200))) }));
    r.register(ToolEntry {
        name: "big_text".into(),
        toolset: "test".into(),
        description: "".into(),
        input_schema: json!({"type": "object"}),
        max_result_size: None,
        max_text_bytes: Some(50),
        max_image_bytes: Some(1_000_000),
        timeout_secs: None,
        disabled: false,
        handler: legacy_text_handler("noop"),
        multimodal_handler: Some(mm),
    })
    .unwrap();

    let err = r.dispatch("big_text", json!({})).await.unwrap_err();
    match err {
        ToolError::OutputTooLarge { size, max } => {
            assert_eq!(max, 50);
            assert_eq!(size, 200);
        }
        other => panic!("expected OutputTooLarge, got {other:?}"),
    }
}

#[tokio::test]
async fn dispatch_image_cap_independent_of_text_cap() {
    let r = registry();
    let mm: if2ai_backend::modules::tools::registry::ToolHandlerMultimodal =
        Arc::new(|_args, _ctx| {
            Box::pin(async move { Ok(ToolOutput::image("image/png", "X".repeat(4_000))) })
        });
    r.register(ToolEntry {
        name: "big_img".into(),
        toolset: "test".into(),
        description: "".into(),
        input_schema: json!({"type": "object"}),
        max_result_size: None,
        max_text_bytes: Some(64),
        max_image_bytes: Some(1024),
        timeout_secs: None,
        disabled: false,
        handler: legacy_text_handler("noop"),
        multimodal_handler: Some(mm),
    })
    .unwrap();

    let err = r.dispatch("big_img", json!({})).await.unwrap_err();
    assert!(matches!(err, ToolError::OutputTooLarge { max: 1024, .. }));
}

#[tokio::test]
async fn legacy_dispatch_string_collapses_image_to_placeholder() {
    let r = registry();
    let mm: if2ai_backend::modules::tools::registry::ToolHandlerMultimodal =
        Arc::new(|_args, _ctx| {
            Box::pin(async move {
                Ok(ToolOutput::text_then_image(
                    "summary",
                    "image/jpeg",
                    "yyyy",
                    Some("scene".into()),
                ))
            })
        });
    r.register(ToolEntry {
        name: "shot".into(),
        toolset: "test".into(),
        description: "".into(),
        input_schema: json!({"type": "object"}),
        max_result_size: None,
        max_text_bytes: None,
        max_image_bytes: None,
        timeout_secs: None,
        disabled: false,
        handler: legacy_text_handler("noop"),
        multimodal_handler: Some(mm),
    })
    .unwrap();

    let s = r
        .dispatch_with_context_legacy(
            "shot",
            json!({}),
            Arc::new(Mutex::new(ToolContext {
                session_id: None,
                project_id: None,
                workdir: PathBuf::from("."),
                permission_mode: PermissionMode::DangerFullAccess,
            })),
        )
        .await
        .unwrap();

    assert!(s.contains("summary"));
    assert!(s.contains("[image: image/jpeg 4B — scene]"));
}

#[test]
fn anthropic_image_block_serialises_with_nested_source() {
    let block = ToolResultContentBlock::Image {
        source: ImageSource::base64("image/jpeg", "AAAA"),
        alt: Some("page".into()),
    };
    let json_value = serde_json::to_value(&block).unwrap();
    // Anthropic wire shape: {type:"image", source:{type:"base64", media_type, data}, alt}
    assert_eq!(json_value["type"], "image");
    assert_eq!(json_value["source"]["type"], "base64");
    assert_eq!(json_value["source"]["media_type"], "image/jpeg");
    assert_eq!(json_value["source"]["data"], "AAAA");
    assert_eq!(json_value["alt"], "page");
}

#[test]
fn anthropic_image_block_roundtrips_via_serde() {
    let block = ToolResultContentBlock::Image {
        source: ImageSource::base64("image/png", "BBBB"),
        alt: None,
    };
    let json_str = serde_json::to_string(&block).unwrap();
    let back: ToolResultContentBlock = serde_json::from_str(&json_str).unwrap();
    assert_eq!(back, block);
}

#[test]
fn tool_output_to_legacy_string_handles_image_with_caption() {
    let out = ToolOutput::text_then_image("ctx", "image/jpeg", "DATA", Some("alt".into()));
    let s = out.to_legacy_string();
    assert!(s.contains("ctx"));
    assert!(s.contains("[image: image/jpeg 4B — alt]"));
}
