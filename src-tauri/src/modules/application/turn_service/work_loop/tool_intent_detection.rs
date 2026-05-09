//! Assistant text → tool intent extraction and detection helpers.

/// Return true when assistant prose claims it is executing/writing files without tool evidence.
#[must_use]
pub fn assistant_claims_tool_execution_without_tool(text: &str) -> bool {
    let lower = text.to_lowercase();
    let tool_channel_marker = lower.contains("<function_calls")
        || lower.contains("<invoke")
        || lower.contains("tool_use")
        || lower.contains("function_call");
    let command_write_claim = (lower.contains("bash")
        || lower.contains("shell")
        || lower.contains("terminal")
        || lower.contains("命令")
        || lower.contains("工具"))
        && (lower.contains("写入")
            || lower.contains("创建文件")
            || lower.contains("保存")
            || lower.contains("write")
            || lower.contains("create file"));
    let file_write_claim = (lower.contains("index.html")
        || lower.contains(".html")
        || lower.contains(".css")
        || lower.contains(".js")
        || lower.contains("文件"))
        && (lower.contains("直接写入")
            || lower.contains("一次性写入")
            || lower.contains("完整写入")
            || lower.contains("我把")
            || lower.contains("我会")
            || lower.contains("让我用")
            || lower.contains("i will write")
            || lower.contains("i'll write"));

    tool_channel_marker || command_write_claim || file_write_claim
}

/// Return true when assistant prose says it is about to use an external action.
#[must_use]
pub fn assistant_signals_tool_intent(text: &str) -> bool {
    let lower = text.to_lowercase();
    if assistant_claims_tool_execution_without_tool(text) {
        return true;
    }

    let action_prefix = [
        "let me",
        "i'll",
        "i will",
        "i am going to",
        "i'm going to",
        "让我",
        "我来",
        "我会",
        "我将",
        "先看",
        "先检查",
        "先执行",
        "开始",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    let external_action = [
        "search",
        "check",
        "fetch",
        "find",
        "read",
        "write",
        "create",
        "run",
        "execute",
        "inspect",
        "open",
        "edit",
        "bash",
        "tool",
        "查找",
        "搜索",
        "检查",
        "读取",
        "写入",
        "创建",
        "运行",
        "执行",
        "打开",
        "调用工具",
        "用工具",
        "用 bash",
        "用 repl",
    ]
    .iter()
    .any(|needle| lower.contains(needle));

    action_prefix && external_action
}

/// Provider compatibility signal for models that print pseudo tool markup
/// instead of emitting structured `tool_calls` deltas.
///
/// Normalize ASCII pipe variants to the Unicode fullwidth `｜` (U+FF5C) that
/// the existing DSML detection/extraction logic recognizes.
///
/// DeepSeek-V4-Flash and similar models sometimes emit DSML markers using
/// ASCII pipes (`<|DSML|...>`), single or doubled (`<||DSML||...>`), instead
/// of the canonical Unicode fullwidth form (`<｜DSML｜...>`). The two forms
/// render visually identical in most fonts, so the divergence is easy to
/// miss. This normalization pass converts the ASCII variants to the canonical
/// fullwidth form so all downstream logic (detection, extraction, tests)
/// works against a single representation.
///
/// Order matters: replace doubled pipes (`||`) first so we don't double-replace
/// single pipes embedded within them.
#[must_use]
pub fn normalize_dsml_delimiters(text: &str) -> String {
    use std::sync::OnceLock;

    use regex::Regex;

    static DSML_OPEN: OnceLock<Regex> = OnceLock::new();
    static DSML_CLOSE: OnceLock<Regex> = OnceLock::new();

    let open_re = DSML_OPEN.get_or_init(|| {
        Regex::new(
            r"<\s*[\|\u{FF5C}](?:\s*[\|\u{FF5C}])*\s*DSML\s*[\|\u{FF5C}](?:\s*[\|\u{FF5C}])*\s*",
        )
        .expect("DSML open regex must compile")
    });
    let close_re = DSML_CLOSE.get_or_init(|| {
        Regex::new(
            r"</\s*[\|\u{FF5C}](?:\s*[\|\u{FF5C}])*\s*DSML\s*[\|\u{FF5C}](?:\s*[\|\u{FF5C}])*\s*",
        )
        .expect("DSML close regex must compile")
    });

    let s = close_re
        .replace_all(text, "</\u{FF5C}DSML\u{FF5C}")
        .into_owned();
    open_re
        .replace_all(&s, "<\u{FF5C}DSML\u{FF5C}")
        .into_owned()
}

/// Detect provider-emitted textual tool call markup family.
pub fn detect_textual_tool_call_markup(text: &str) -> Option<String> {
    let normalized = normalize_dsml_delimiters(text);
    let text = normalized.as_str();
    let lower = text.to_lowercase();
    let family = if text.contains("<｜DSML｜tool_calls")
        || text.contains("<｜DSML｜invoke")
        || text.contains("</｜DSML｜tool_calls>")
    {
        Some("deepseek_dsml_tool_calls")
    } else if lower.contains("<function_calls") || lower.contains("</function_calls>") {
        Some("xml_function_calls")
    } else if lower.contains("<invoke") || lower.contains("</invoke>") {
        Some("xml_invoke")
    } else if lower.contains("<tool_use") || lower.contains("</tool_use>") {
        Some("xml_tool_use")
    } else if lower.contains("\"tool_calls\"") || lower.contains("function_call") {
        Some("json_tool_call_text")
    } else {
        None
    }?;
    Some(family.to_string())
}

/// Extract provider-emitted textual tool calls into the canonical pending-tool
/// tuple used by the stream loop.
#[must_use]
pub fn extract_textual_tool_calls(text: &str) -> Vec<(String, String, String)> {
    let normalized = normalize_dsml_delimiters(text);
    let text = normalized.as_str();
    let mut calls = extract_deepseek_dsml_tool_calls(text);
    let offset = calls.len();
    calls.extend(extract_xml_invoke_tool_calls(text, offset));
    calls
}

fn extract_deepseek_dsml_tool_calls(text: &str) -> Vec<(String, String, String)> {
    const INVOKE_OPEN: &str = "<｜DSML｜invoke";
    const INVOKE_CLOSE: &str = "</｜DSML｜invoke>";
    const PARAM_OPEN: &str = "<｜DSML｜parameter";
    const PARAM_CLOSE: &str = "</｜DSML｜parameter>";

    let mut calls = Vec::new();
    let mut rest = text;
    while let Some(invoke_start) = rest.find(INVOKE_OPEN) {
        let invoke_slice = &rest[invoke_start..];
        let Some(invoke_tag_end) = invoke_slice.find('>') else {
            break;
        };
        let invoke_tag = &invoke_slice[..=invoke_tag_end];
        let Some(tool_name) = extract_quoted_attr(invoke_tag, "name") else {
            rest = &invoke_slice[invoke_tag_end + 1..];
            continue;
        };

        let body_start = invoke_tag_end + 1;
        let Some(invoke_close_start) = invoke_slice[body_start..].find(INVOKE_CLOSE) else {
            break;
        };
        let invoke_body = &invoke_slice[body_start..body_start + invoke_close_start];
        let mut args = serde_json::Map::new();
        let mut param_rest = invoke_body;
        while let Some(param_start) = param_rest.find(PARAM_OPEN) {
            let param_slice = &param_rest[param_start..];
            let Some(param_tag_end) = param_slice.find('>') else {
                break;
            };
            let param_tag = &param_slice[..=param_tag_end];
            let Some(param_name) = extract_quoted_attr(param_tag, "name") else {
                param_rest = &param_slice[param_tag_end + 1..];
                continue;
            };
            let param_body_start = param_tag_end + 1;
            let Some(param_close_start) = param_slice[param_body_start..].find(PARAM_CLOSE) else {
                break;
            };
            let raw_value = &param_slice[param_body_start..param_body_start + param_close_start];
            args.insert(
                param_name,
                serde_json::Value::String(decode_basic_xml_entities(raw_value)),
            );
            param_rest = &param_slice[param_body_start + param_close_start + PARAM_CLOSE.len()..];
        }

        let input_json = serde_json::Value::Object(args).to_string();
        let call_id = format!("textual_dsml_tool_call:{}", calls.len());
        calls.push((call_id, tool_name, input_json));
        rest = &invoke_slice[body_start + invoke_close_start + INVOKE_CLOSE.len()..];
    }

    calls
}

fn extract_quoted_attr(tag: &str, attr_name: &str) -> Option<String> {
    let pattern = format!("{attr_name}=\"");
    let value_start = tag.find(&pattern)? + pattern.len();
    let value_rest = &tag[value_start..];
    let value_end = value_rest.find('"')?;
    Some(decode_basic_xml_entities(&value_rest[..value_end]))
}

fn decode_basic_xml_entities(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn extract_xml_invoke_tool_calls(
    text: &str,
    call_id_offset: usize,
) -> Vec<(String, String, String)> {
    const INVOKE_OPEN: &str = "<invoke";
    const INVOKE_CLOSE: &str = "</invoke>";
    const PARAM_OPEN: &str = "<parameter";
    const PARAM_CLOSE: &str = "</parameter>";

    let mut calls = Vec::new();
    let mut rest = text;
    while let Some(invoke_start) = rest.find(INVOKE_OPEN) {
        let invoke_slice = &rest[invoke_start..];
        let Some(invoke_tag_end) = invoke_slice.find('>') else {
            break;
        };
        let invoke_tag = &invoke_slice[..=invoke_tag_end];
        let Some(tool_name) = extract_quoted_attr(invoke_tag, "name") else {
            rest = &invoke_slice[invoke_tag_end + 1..];
            continue;
        };

        let body_start = invoke_tag_end + 1;
        let Some(invoke_close_start) = invoke_slice[body_start..].find(INVOKE_CLOSE) else {
            break;
        };
        let invoke_body = &invoke_slice[body_start..body_start + invoke_close_start];
        let mut args = serde_json::Map::new();
        let mut param_rest = invoke_body;
        while let Some(param_start) = param_rest.find(PARAM_OPEN) {
            let param_slice = &param_rest[param_start..];
            let Some(param_tag_end) = param_slice.find('>') else {
                break;
            };
            let param_tag = &param_slice[..=param_tag_end];
            let Some(param_name) = extract_quoted_attr(param_tag, "name") else {
                param_rest = &param_slice[param_tag_end + 1..];
                continue;
            };
            let param_body_start = param_tag_end + 1;
            let Some(param_close_start) = param_slice[param_body_start..].find(PARAM_CLOSE) else {
                break;
            };
            let raw_value = &param_slice[param_body_start..param_body_start + param_close_start];
            args.insert(
                param_name,
                serde_json::Value::String(decode_basic_xml_entities(raw_value)),
            );
            param_rest = &param_slice[param_body_start + param_close_start + PARAM_CLOSE.len()..];
        }

        if !args.is_empty() {
            let input_json = serde_json::Value::Object(args).to_string();
            let call_id = format!("textual_xml_tool_call:{}", call_id_offset + calls.len());
            calls.push((call_id, tool_name, input_json));
        }
        rest = &invoke_slice[body_start + invoke_close_start + INVOKE_CLOSE.len()..];
    }

    calls
}

/// Nudge used when provider text announces an action but emits no tool call.
#[must_use]
pub fn tool_intent_nudge_message() -> String {
    "[agent_loop_control]\nYou said you would perform an action, but no tool call was emitted. Do not describe the action in prose. Use the available tool_calls mechanism now with complete JSON arguments. If the intended tool is blocked or unavailable, produce a final report that explicitly says why the work cannot proceed.".to_string()
}

/// Detect pathological assistant stutter loops before they are treated as a
/// normal final answer.
#[must_use]
pub fn detect_repetitive_model_output(text: &str) -> bool {
    let char_count = text.chars().count();
    if char_count < 280 {
        return false;
    }
    let normalized = text.to_lowercase();
    let continuation_mentions =
        normalized.matches("继续").count() + normalized.matches("continue").count();
    let inspection_mentions = normalized.matches("检查目录").count()
        + normalized.matches("查看目录").count()
        + normalized.matches("看目录").count()
        + normalized.matches("inspect the").count()
        + normalized.matches("check the").count();
    if continuation_mentions >= 12 && inspection_mentions >= 4 {
        return true;
    }

    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for line in text.lines() {
        let normalized_line = line
            .trim()
            .trim_matches(|ch: char| ch.is_ascii_punctuation() || ch.is_whitespace())
            .to_lowercase();
        if normalized_line.chars().count() < 4 {
            continue;
        }
        let count = counts.entry(normalized_line).or_insert(0);
        *count += 1;
        if *count >= 5 {
            return true;
        }
    }

    false
}

/// Return true when a terminal reason implies tool execution was required but absent.
pub fn is_tool_required_terminal_reason(reason: &str) -> bool {
    matches!(
        reason,
        "tool_required_no_tool"
            | "model_stop_no_tools"
            | "max_iterations_reached"
            | "repeated_tool_batch_no_progress"
            | "invalid_tool_args_repeated"
            | "provider_textual_tool_call_markup"
            | "repetitive_model_output"
            | "provider_prepare_failed"
            | "stream_error"
    )
}
