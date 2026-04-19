//! Multimodal tool output (Phase 7C, slice 7C.2).
//!
//! Before this slice every `ToolHandler` returned `Result<String, ToolError>`.
//! That made it impossible for tools to emit anything non-textual: the
//! `browser` tool's `screenshot` action used to encode a JPEG as
//! `"data:image/jpeg;base64,..."` and dump the raw base64 into the LLM
//! prompt — wasting tens of thousands of tokens while the model received
//! no usable visual signal.
//!
//! This module introduces:
//!
//! - [`ToolResultPart`] — one chunk of a tool result (text or image).
//! - [`ToolOutput`]     — the full handler result, an ordered `Vec<ToolResultPart>`.
//! - Convenience constructors and a [`ToolOutput::to_legacy_string`] fallback
//!   used by code paths that still consume `String` (most of the agent today).
//!
//! Tools opt into multimodal output by populating
//! [`ToolEntry::multimodal_handler`](crate::modules::tools::ToolEntry::multimodal_handler)
//! instead of (or in addition to) the legacy text-only `handler`; existing
//! 30+ tools are unaffected.

use serde::{Deserialize, Serialize};

/// A single chunk inside a [`ToolOutput`].
///
/// Serialised with an explicit `kind` discriminator so the JS frontend (and
/// any future external consumer of `tool_result` events) can branch on the
/// shape without sniffing field presence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolResultPart {
    /// Plain text — the only flavour produced by legacy 7B tools.
    Text {
        /// The textual content (no length limit applied here; `dispatch`
        /// enforces `max_text_bytes`).
        text: String,
    },
    /// Image bytes encoded as base64 (no `data:` URI prefix).  Provider
    /// adapters are responsible for re-shaping this into the vendor's
    /// expected wire format (Anthropic `image.source.base64`, OpenAI
    /// `image_url`, Gemini `inlineData`).
    Image {
        /// MIME type, e.g. `"image/jpeg"` or `"image/png"`.
        mime: String,
        /// Base64-encoded raw bytes.
        data: String,
        /// Optional descriptive caption.  Surfaced as `alt` text and as a
        /// fallback for vision-incapable models.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        alt: Option<String>,
    },
}

impl ToolResultPart {
    /// Approximate byte cost of this part.  For `Text` it is the UTF-8
    /// length of the string; for `Image` the base64-encoded payload length
    /// (which is what hits the LLM context, not the underlying binary).
    #[must_use]
    pub fn byte_size(&self) -> usize {
        match self {
            Self::Text { text } => text.len(),
            Self::Image { data, .. } => data.len(),
        }
    }

    /// `true` when this part is an image (used by the dispatch layer to
    /// route to `max_image_bytes` instead of `max_text_bytes`).
    #[must_use]
    pub const fn is_image(&self) -> bool {
        matches!(self, Self::Image { .. })
    }
}

/// Ordered collection of [`ToolResultPart`]s returned by a tool handler.
///
/// Most tools still return a single `Text` part; only tools that actually
/// produce multimedia (currently just `browser` in the `screenshot` action)
/// emit `Image` parts.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ToolOutput {
    /// Parts in display order — mirrors the user-visible message ordering
    /// produced by Anthropic's content-block array convention.
    pub parts: Vec<ToolResultPart>,
}

impl ToolOutput {
    /// Construct a single-text-part output (the most common case).
    #[must_use]
    pub fn text(s: impl Into<String>) -> Self {
        Self {
            parts: vec![ToolResultPart::Text { text: s.into() }],
        }
    }

    /// Construct a single-image-part output.
    #[allow(dead_code)]
    #[must_use]
    pub fn image(mime: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            parts: vec![ToolResultPart::Image {
                mime: mime.into(),
                data: data.into(),
                alt: None,
            }],
        }
    }

    /// Construct a single-image-part output with an alt-text caption.
    ///
    /// The caption is used both as accessibility text and as a textual
    /// fallback served to providers without vision support.
    #[allow(dead_code)]
    #[must_use]
    pub fn image_with_alt(
        mime: impl Into<String>,
        data: impl Into<String>,
        alt: impl Into<String>,
    ) -> Self {
        Self {
            parts: vec![ToolResultPart::Image {
                mime: mime.into(),
                data: data.into(),
                alt: Some(alt.into()),
            }],
        }
    }

    /// Construct a `Text` + `Image` two-part output.  Convenience helper
    /// used by `browser_tool::screenshot` so the LLM gets a textual context
    /// line ("Screenshot at <url>") alongside the visual content.
    #[must_use]
    pub fn text_then_image(
        text: impl Into<String>,
        mime: impl Into<String>,
        data: impl Into<String>,
        alt: Option<String>,
    ) -> Self {
        Self {
            parts: vec![
                ToolResultPart::Text { text: text.into() },
                ToolResultPart::Image {
                    mime: mime.into(),
                    data: data.into(),
                    alt,
                },
            ],
        }
    }

    /// `true` when the output carries no parts at all.
    #[allow(dead_code)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// `true` when at least one part is an image.  Used by the agent loop
    /// (slice 7C.3+) to decide whether to emit a multimodal
    /// `ToolResultContentBlock` when forwarding the result to the LLM.
    #[allow(dead_code)]
    #[must_use]
    pub fn has_image(&self) -> bool {
        self.parts.iter().any(ToolResultPart::is_image)
    }

    /// Sum of `byte_size` over `parts`.  Reserved for future telemetry
    /// callers; the dispatch layer enforces caps via the per-modality
    /// `text_byte_size` / `image_byte_size` instead so that a mixed output
    /// is not rejected just because the *sum* trips a single limit.
    #[allow(dead_code)]
    #[must_use]
    pub fn byte_size(&self) -> usize {
        self.parts.iter().map(ToolResultPart::byte_size).sum()
    }

    /// Sum of `byte_size` over `Text` parts only.
    #[must_use]
    pub fn text_byte_size(&self) -> usize {
        self.parts
            .iter()
            .filter(|p| !p.is_image())
            .map(ToolResultPart::byte_size)
            .sum()
    }

    /// Sum of `byte_size` over `Image` parts only (in base64 bytes).
    #[must_use]
    pub fn image_byte_size(&self) -> usize {
        self.parts
            .iter()
            .filter(|p| p.is_image())
            .map(ToolResultPart::byte_size)
            .sum()
    }

    /// Project the output into a single string, suitable for legacy code
    /// paths that still consume `Result<String, ToolError>`.
    ///
    /// Image parts collapse to `[image: <mime> <bytes>B <alt?>]` placeholders
    /// so the result remains useful for logging / persistence even when no
    /// vision-capable provider is in the loop.
    #[must_use]
    pub fn to_legacy_string(&self) -> String {
        let mut out = String::new();
        for (i, part) in self.parts.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            match part {
                ToolResultPart::Text { text } => out.push_str(text),
                ToolResultPart::Image { mime, data, alt } => {
                    let bytes = data.len();
                    match alt {
                        Some(caption) if !caption.is_empty() => {
                            out.push_str(&format!("[image: {mime} {bytes}B — {caption}]"));
                        }
                        _ => out.push_str(&format!("[image: {mime} {bytes}B]")),
                    }
                }
            }
        }
        out
    }
}

impl From<String> for ToolOutput {
    fn from(s: String) -> Self {
        Self::text(s)
    }
}

impl From<&str> for ToolOutput {
    fn from(s: &str) -> Self {
        Self::text(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_constructor_produces_single_text_part() {
        let out = ToolOutput::text("hello");
        assert_eq!(out.parts.len(), 1);
        assert!(matches!(&out.parts[0], ToolResultPart::Text { text } if text == "hello"));
        assert!(!out.has_image());
    }

    #[test]
    fn image_constructor_produces_single_image_part() {
        let out = ToolOutput::image("image/jpeg", "BASE64DATA");
        assert!(out.has_image());
        assert_eq!(out.image_byte_size(), "BASE64DATA".len());
        assert_eq!(out.text_byte_size(), 0);
    }

    #[test]
    fn text_then_image_keeps_order() {
        let out =
            ToolOutput::text_then_image("caption", "image/png", "DATA", Some("alt".to_string()));
        assert_eq!(out.parts.len(), 2);
        assert!(matches!(&out.parts[0], ToolResultPart::Text { .. }));
        assert!(matches!(&out.parts[1], ToolResultPart::Image { .. }));
    }

    #[test]
    fn to_legacy_string_collapses_image_with_caption() {
        let out =
            ToolOutput::text_then_image("ctx", "image/jpeg", "x".repeat(100), Some("alt".into()));
        let s = out.to_legacy_string();
        assert!(s.starts_with("ctx\n"));
        assert!(s.contains("[image: image/jpeg 100B — alt]"));
    }

    #[test]
    fn to_legacy_string_collapses_image_without_caption() {
        let out = ToolOutput::image("image/png", "yy");
        assert_eq!(out.to_legacy_string(), "[image: image/png 2B]");
    }

    #[test]
    fn byte_size_sums_text_and_image() {
        let out = ToolOutput::text_then_image("hi", "image/jpeg", "AAAA", None);
        assert_eq!(out.text_byte_size(), 2);
        assert_eq!(out.image_byte_size(), 4);
        assert_eq!(out.byte_size(), 6);
    }

    #[test]
    fn empty_output_to_legacy_string_is_empty() {
        let out = ToolOutput::default();
        assert!(out.is_empty());
        assert_eq!(out.to_legacy_string(), "");
    }

    #[test]
    fn from_string_lifts_into_text_part() {
        let out: ToolOutput = "hello".to_string().into();
        assert_eq!(out.parts.len(), 1);
    }

    #[test]
    fn from_str_lifts_into_text_part() {
        let out: ToolOutput = "hello".into();
        assert_eq!(out.parts.len(), 1);
    }

    #[test]
    fn serde_roundtrip_text() {
        let out = ToolOutput::text("hi");
        let json = serde_json::to_string(&out).unwrap();
        let back: ToolOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(back, out);
    }

    #[test]
    fn serde_roundtrip_image_with_alt() {
        let out = ToolOutput::image_with_alt("image/png", "AAAA", "screen");
        let json = serde_json::to_string(&out).unwrap();
        assert!(json.contains("\"kind\":\"image\""));
        assert!(json.contains("\"alt\":\"screen\""));
        let back: ToolOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(back, out);
    }
}
