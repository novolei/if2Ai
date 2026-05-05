use super::{
    collapse_blank_lines, display_context_path, normalize_instruction_content,
    render_instruction_content, render_instruction_files, truncate_instruction_content,
    ContextFile, ProjectContext, SystemPromptBuilder, SYSTEM_PROMPT_DYNAMIC_BOUNDARY,
};
use crate::modules::runtime::config::ConfigLoader;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("runtime-prompt-{nanos}"))
}

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[test]
fn discovers_instruction_files_from_ancestor_chain() {
    let root = temp_dir();
    let nested = root.join("apps").join("api");
    fs::create_dir_all(nested.join(".claw")).expect("nested claw dir");
    fs::write(root.join("CLAW.md"), "root instructions").expect("write root instructions");
    fs::write(root.join("CLAW.local.md"), "local instructions").expect("write local instructions");
    fs::create_dir_all(root.join("apps")).expect("apps dir");
    fs::create_dir_all(root.join("apps").join(".claw")).expect("apps claw dir");
    fs::write(root.join("apps").join("CLAW.md"), "apps instructions")
        .expect("write apps instructions");
    fs::write(
        root.join("apps").join(".claw").join("instructions.md"),
        "apps dot claw instructions",
    )
    .expect("write apps dot claw instructions");
    fs::write(nested.join(".claw").join("CLAW.md"), "nested rules").expect("write nested rules");
    fs::write(
        nested.join(".claw").join("instructions.md"),
        "nested instructions",
    )
    .expect("write nested instructions");

    let context = ProjectContext::discover(&nested, "2026-03-31").expect("context should load");
    let contents = context
        .instruction_files
        .iter()
        .map(|file| file.content.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        contents,
        vec![
            "root instructions",
            "local instructions",
            "apps instructions",
            "apps dot claw instructions",
            "nested rules",
            "nested instructions"
        ]
    );
    fs::remove_dir_all(root).expect("cleanup temp dir");
}

#[test]
fn dedupes_identical_instruction_content_across_scopes() {
    let root = temp_dir();
    let nested = root.join("apps").join("api");
    fs::create_dir_all(&nested).expect("nested dir");
    fs::write(root.join("CLAW.md"), "same rules\n\n").expect("write root");
    fs::write(nested.join("CLAW.md"), "same rules\n").expect("write nested");

    let context = ProjectContext::discover(&nested, "2026-03-31").expect("context should load");
    assert_eq!(context.instruction_files.len(), 1);
    assert_eq!(
        normalize_instruction_content(&context.instruction_files[0].content),
        "same rules"
    );
    fs::remove_dir_all(root).expect("cleanup temp dir");
}

#[test]
fn truncates_large_instruction_content_for_rendering() {
    let rendered = render_instruction_content(&"x".repeat(4500));
    assert!(rendered.contains("[truncated]"));
    assert!(rendered.len() < 4_100);
}

#[test]
fn normalizes_and_collapses_blank_lines() {
    let normalized = normalize_instruction_content("line one\n\n\nline two\n");
    assert_eq!(normalized, "line one\n\nline two");
    assert_eq!(collapse_blank_lines("a\n\n\n\nb\n"), "a\n\nb\n");
}

#[test]
fn displays_context_paths_compactly() {
    assert_eq!(
        display_context_path(Path::new("/tmp/project/.claw/CLAW.md")),
        "CLAW.md"
    );
}

#[test]
fn discover_with_git_includes_status_snapshot() {
    let _guard = env_lock();
    let root = temp_dir();
    fs::create_dir_all(&root).expect("root dir");
    std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(&root)
        .status()
        .expect("git init should run");
    fs::write(root.join("CLAW.md"), "rules").expect("write instructions");
    fs::write(root.join("tracked.txt"), "hello").expect("write tracked file");

    let context =
        ProjectContext::discover_with_git(&root, "2026-03-31").expect("context should load");

    let status = context.git_status.expect("git status should be present");
    assert!(status.contains("## No commits yet on") || status.contains("## "));
    assert!(status.contains("?? CLAW.md"));
    assert!(status.contains("?? tracked.txt"));
    assert!(context.git_diff.is_none());

    fs::remove_dir_all(root).expect("cleanup temp dir");
}

#[test]
fn discover_with_git_includes_diff_snapshot_for_tracked_changes() {
    let _guard = env_lock();
    let root = temp_dir();
    fs::create_dir_all(&root).expect("root dir");
    std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(&root)
        .status()
        .expect("git init should run");
    std::process::Command::new("git")
        .args(["config", "user.email", "tests@example.com"])
        .current_dir(&root)
        .status()
        .expect("git config email should run");
    std::process::Command::new("git")
        .args(["config", "user.name", "Runtime Prompt Tests"])
        .current_dir(&root)
        .status()
        .expect("git config name should run");
    fs::write(root.join("tracked.txt"), "hello\n").expect("write tracked file");
    std::process::Command::new("git")
        .args(["add", "tracked.txt"])
        .current_dir(&root)
        .status()
        .expect("git add should run");
    std::process::Command::new("git")
        .args(["commit", "-m", "init", "--quiet"])
        .current_dir(&root)
        .status()
        .expect("git commit should run");
    fs::write(root.join("tracked.txt"), "hello\nworld\n").expect("rewrite tracked file");

    let context =
        ProjectContext::discover_with_git(&root, "2026-03-31").expect("context should load");

    let diff = context.git_diff.expect("git diff should be present");
    assert!(diff.contains("Unstaged changes:"));
    assert!(diff.contains("tracked.txt"));

    fs::remove_dir_all(root).expect("cleanup temp dir");
}

#[test]
fn load_system_prompt_reads_claw_files_and_config() {
    let root = temp_dir();
    fs::create_dir_all(root.join(".claw")).expect("claw dir");
    fs::create_dir_all(root.join(".if2ai")).expect("if2ai config dir");
    fs::write(root.join("CLAW.md"), "Project rules").expect("write instructions");
    fs::write(
        root.join(".if2ai").join("settings.json"),
        r#"{"permissionMode":"acceptEdits"}"#,
    )
    .expect("write settings");

    let _guard = env_lock();
    let previous = std::env::current_dir().expect("cwd");
    let original_home = std::env::var("HOME").ok();
    let original_if2ai_home = std::env::var("IF2AI_CONFIG_HOME").ok();
    std::env::set_var("HOME", &root);
    std::env::set_var("IF2AI_CONFIG_HOME", root.join("missing-home"));
    std::env::set_current_dir(&root).expect("change cwd");
    let prompt = super::load_system_prompt(&root, "2026-03-31", "linux", "6.8")
        .expect("system prompt should load")
        .join(
            "

",
        );
    std::env::set_current_dir(previous).expect("restore cwd");
    if let Some(value) = original_home {
        std::env::set_var("HOME", value);
    } else {
        std::env::remove_var("HOME");
    }
    if let Some(value) = original_if2ai_home {
        std::env::set_var("IF2AI_CONFIG_HOME", value);
    } else {
        std::env::remove_var("IF2AI_CONFIG_HOME");
    }

    assert!(prompt.contains("Project rules"));
    assert!(prompt.contains("permissionMode"));
    fs::remove_dir_all(root).expect("cleanup temp dir");
}

#[test]
fn renders_claw_code_style_sections_with_project_context() {
    let root = temp_dir();
    fs::create_dir_all(root.join(".claw")).expect("claw dir");
    fs::create_dir_all(root.join(".if2ai")).expect("if2ai config dir");
    fs::write(root.join("CLAW.md"), "Project rules").expect("write CLAW.md");
    fs::write(
        root.join(".if2ai").join("settings.json"),
        r#"{"permissionMode":"acceptEdits"}"#,
    )
    .expect("write settings");

    let project_context =
        ProjectContext::discover(&root, "2026-03-31").expect("context should load");
    let config = ConfigLoader::new(&root, root.join("missing-home"))
        .load()
        .expect("config should load");
    let prompt = SystemPromptBuilder::new()
        .with_output_style("Concise", "Prefer short answers.")
        .with_os("linux", "6.8")
        .with_project_context(project_context)
        .with_runtime_config(config)
        .render();

    assert!(prompt.contains("# System"));
    assert!(prompt.contains("# Project context"));
    assert!(prompt.contains("# Claw instructions"));
    assert!(prompt.contains("Project rules"));
    assert!(prompt.contains("permissionMode"));
    assert!(prompt.contains(SYSTEM_PROMPT_DYNAMIC_BOUNDARY));

    fs::remove_dir_all(root).expect("cleanup temp dir");
}

#[test]
fn truncates_instruction_content_to_budget() {
    let content = "x".repeat(5_000);
    let rendered = truncate_instruction_content(&content, 4_000);
    assert!(rendered.contains("[truncated]"));
    assert!(rendered.chars().count() <= 4_000 + "\n\n[truncated]".chars().count());
}

#[test]
fn discovers_dot_claw_instructions_markdown() {
    let root = temp_dir();
    let nested = root.join("apps").join("api");
    fs::create_dir_all(nested.join(".claw")).expect("nested claw dir");
    fs::write(
        nested.join(".claw").join("instructions.md"),
        "instruction markdown",
    )
    .expect("write instructions.md");

    let context = ProjectContext::discover(&nested, "2026-03-31").expect("context should load");
    assert!(context
        .instruction_files
        .iter()
        .any(|file| file.path.ends_with(".claw/instructions.md")));
    assert!(render_instruction_files(&context.instruction_files).contains("instruction markdown"));

    fs::remove_dir_all(root).expect("cleanup temp dir");
}

#[test]
fn renders_instruction_file_metadata() {
    let rendered = render_instruction_files(&[ContextFile {
        path: PathBuf::from("/tmp/project/CLAW.md"),
        content: "Project rules".to_string(),
    }]);
    assert!(rendered.contains("# Claw instructions"));
    assert!(rendered.contains("scope: /tmp/project"));
    assert!(rendered.contains("Project rules"));
}

/// Phase 8A.11 — confirm `with_memory_injection` appends pinned +
/// compiled + rules sections after the dynamic boundary marker.
#[test]
fn builder_appends_pinned_section_after_boundary() {
    use crate::modules::memory::MemoryInjection;
    let injection = MemoryInjection {
        pinned_section: Some("## Pinned memory\n\n- alpha\n".to_string()),
        compiled_section: Some("## Compiled memory\n\nbody\n".to_string()),
        rules_section: "## Memory usage rules\n\n- rule one\n".to_string(),
        procedural_section: None,
        total_tokens_estimate: 10,
        ..Default::default()
    };
    let sections = SystemPromptBuilder::new()
        .with_memory_injection(injection)
        .build();
    let boundary_idx = sections
        .iter()
        .position(|s| s == SYSTEM_PROMPT_DYNAMIC_BOUNDARY)
        .expect("boundary present");
    let pinned_idx = sections
        .iter()
        .position(|s| s.contains("## Pinned memory"))
        .expect("pinned section present");
    let compiled_idx = sections
        .iter()
        .position(|s| s.contains("## Compiled memory"))
        .expect("compiled section present");
    let rules_idx = sections
        .iter()
        .position(|s| s.contains("## Memory usage rules"))
        .expect("rules section present");
    assert!(pinned_idx > boundary_idx);
    assert!(compiled_idx > pinned_idx);
    assert!(rules_idx > compiled_idx);
}

/// Phase 8A.11 — without `with_memory_injection`, the build output
/// must contain none of the memory section headers.
#[test]
fn builder_omits_memory_section_when_none() {
    let rendered = SystemPromptBuilder::new().render();
    assert!(!rendered.contains("## Pinned memory"));
    assert!(!rendered.contains("## 置顶记忆"));
    assert!(!rendered.contains("## Compiled memory"));
    assert!(!rendered.contains("## Memory usage rules"));
}

/// Phase 7C, slice 7C.4 — when `with_tool_routing_guide` carries a
/// `Some`, the rendered prompt must include the routing block AND
/// place it BEFORE the dynamic boundary so it sits in the high-attention
/// "static-ish" zone shared with the skills index.
#[test]
fn builder_inserts_tool_routing_guide_before_boundary() {
    let guide = "# Web Access Tool Routing\nesc-test-marker\n".to_string();
    let rendered = SystemPromptBuilder::new()
        .with_tool_routing_guide(Some(guide))
        .render();
    let guide_idx = rendered
        .find("esc-test-marker")
        .expect("routing block should appear");
    let boundary_idx = rendered
        .find(SYSTEM_PROMPT_DYNAMIC_BOUNDARY)
        .expect("boundary should appear");
    assert!(
        guide_idx < boundary_idx,
        "routing guide must come BEFORE the dynamic boundary"
    );
}

/// When no routing guide is supplied the prompt must not gain any
/// stray "Web Access Tool Routing" header.
#[test]
fn builder_omits_routing_guide_when_none() {
    let rendered = SystemPromptBuilder::new().render();
    assert!(!rendered.contains("# Web Access Tool Routing"));
}
