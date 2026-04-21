//! Skills index cache + frontmatter parser + system-prompt index renderer.
//!
//! Extracted from `runtime/prompt/mod.rs` in GFR-T1-G-1 (pure structural
//! move; function bodies byte-identical).

use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use crate::modules::tools::builtin::skill::discover_skill_roots_with_metadata;

/// Maximum number of skills shown in system prompt index.
const MAX_SKILLS_INDEX_COUNT: usize = 40;

// ── Skills index cache ───────────────────────────────────────────────────────

/// Cached skills index: (mtime_hash, index_text).
type SkillsIndexCache = Option<(u64, String)>;

static SKILLS_INDEX_CACHE: OnceLock<Mutex<SkillsIndexCache>> = OnceLock::new();

fn skills_index_cache() -> &'static Mutex<SkillsIndexCache> {
    SKILLS_INDEX_CACHE.get_or_init(|| Mutex::new(None))
}

/// Invalidate the skills index cache (call after install/uninstall).
pub fn invalidate_skills_index_cache() {
    if let Ok(mut guard) = skills_index_cache().lock() {
        *guard = None;
    }
}

/// Compute a hash over the mtime of every skill root directory.
///
/// Used as a cache key — if any skill root changes, the hash changes.
fn compute_skills_dir_hash(workdir: &Path) -> u64 {
    let roots = discover_skill_roots_with_metadata(workdir);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for root in &roots {
        root.path.hash(&mut hasher);
        if let Ok(meta) = std::fs::metadata(&root.path) {
            if let Ok(modified) = meta.modified() {
                modified.hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

/// One entry in the skills index shown to the model.
#[derive(Debug, Clone)]
pub struct SkillIndexEntry {
    /// Skill name (directory name).
    pub name: String,
    /// Short description from SKILL.md frontmatter.
    pub description: String,
    /// Source label: workspace | user | builtin | remote-quarantine.
    pub source: String,
    /// Toolsets required for this skill to activate (from frontmatter).
    pub requires_toolsets: Vec<String>,
    /// Absolute path to the skill directory.
    pub skill_dir: std::path::PathBuf,
}

/// Parse minimal frontmatter fields needed for skills index.
fn parse_index_frontmatter(content: &str) -> (Option<String>, Option<String>, Vec<String>) {
    let body = content.strip_prefix("---").unwrap_or(content);
    let Some((header, _)) = body.split_once("---") else {
        return (None, None, Vec::new());
    };

    let mut description: Option<String> = None;
    let mut name: Option<String> = None;
    let mut requires_toolsets: Vec<String> = Vec::new();

    for line in header.lines() {
        let line = line.trim();
        if let Some((key, value)) = line.split_once(':') {
            let k = key.trim();
            let v = value.trim().trim_matches('"').trim_matches('\'');
            match k {
                "name" => name = Some(v.to_string()),
                "description" => description = Some(v.to_string()),
                "requires_toolsets" => {
                    // Simple inline array: [toolset1, toolset2]
                    let inner = v.trim_start_matches('[').trim_end_matches(']');
                    requires_toolsets = inner
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                _ => {}
            }
        }
    }

    (name, description, requires_toolsets)
}

/// Discover all active skills and return index entries.
///
/// Skills in `remote-quarantine` are excluded (not yet approved).
/// When `available_toolsets` is provided, skills whose `requires_toolsets`
/// are not satisfied are filtered out.
///
/// Exposed as `pub(crate)` so callers like `skills_list` can reuse the same
/// discovery + shadow-resolution logic instead of doing a separate full scan.
pub(crate) fn collect_skill_index_entries(
    workdir: &Path,
    available_toolsets: Option<&[String]>,
) -> Vec<SkillIndexEntry> {
    use std::collections::BTreeMap;

    let roots = discover_skill_roots_with_metadata(workdir);
    let mut candidates: BTreeMap<String, SkillIndexEntry> = BTreeMap::new();

    for root in &roots {
        // Quarantine skills are never shown in the index
        if root.source.as_label() == "remote-quarantine" {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&root.path) else {
            continue;
        };
        for entry in entries.flatten() {
            let skill_md = entry.path().join("SKILL.md");
            if !skill_md.is_file() {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&skill_md) else {
                continue;
            };
            let (fm_name, fm_desc, requires_toolsets) = parse_index_frontmatter(&content);
            let name = fm_name.unwrap_or_else(|| entry.file_name().to_string_lossy().into_owned());
            let description = fm_desc.unwrap_or_default();
            let key = name.to_lowercase();

            // Conditional activation: skip skills whose toolsets aren't available
            if let Some(available) = available_toolsets {
                if !requires_toolsets.is_empty() {
                    let satisfied = requires_toolsets
                        .iter()
                        .all(|ts| available.iter().any(|a| a == ts));
                    if !satisfied {
                        continue;
                    }
                }
            }

            // Shadow resolution: prefer lower precedence number (higher priority)
            let should_insert = candidates.get(&key).is_none_or(|existing| {
                root.precedence
                    < roots
                        .iter()
                        .find(|r| r.source.as_label() == existing.source)
                        .map_or(usize::MAX, |r| r.precedence)
            });
            if should_insert {
                candidates.insert(
                    key,
                    SkillIndexEntry {
                        name,
                        description,
                        source: root.source.as_label().to_string(),
                        requires_toolsets,
                        skill_dir: entry.path(),
                    },
                );
            }
        }
    }

    candidates.into_values().collect()
}

/// Build the Skills section for the system prompt.
///
/// Returns `None` when no skills are installed, so the section is omitted entirely.
///
/// The section instructs the model to scan the index and call
/// `skill_view(name="<name>")` when a skill matches the current task.
pub fn build_skills_index(workdir: &Path, available_toolsets: Option<&[String]>) -> Option<String> {
    let mtime_hash = compute_skills_dir_hash(workdir);

    // Check cache (skip if available_toolsets filter is active, as it may vary per session)
    if available_toolsets.is_none() {
        if let Ok(guard) = skills_index_cache().lock() {
            if let Some((cached_hash, ref cached_text)) = *guard {
                if cached_hash == mtime_hash {
                    return if cached_text.is_empty() {
                        None
                    } else {
                        Some(cached_text.clone())
                    };
                }
            }
        }
    }

    let entries = collect_skill_index_entries(workdir, available_toolsets);
    if entries.is_empty() {
        if available_toolsets.is_none() {
            if let Ok(mut guard) = skills_index_cache().lock() {
                *guard = Some((mtime_hash, String::new()));
            }
        }
        return None;
    }

    let entries_shown = entries.len().min(MAX_SKILLS_INDEX_COUNT);
    let mut index_lines: Vec<String> = entries
        .iter()
        .take(entries_shown)
        .map(|e| {
            if e.description.is_empty() {
                e.name.clone()
            } else {
                format!("{}: {}", e.name, e.description)
            }
        })
        .collect();

    if entries.len() > entries_shown {
        index_lines.push(format!(
            "...and {} more — use skills_list or skill_search to discover them.",
            entries.len() - entries_shown
        ));
    }

    // Prompt strategy aligned with Hermes `build_skills_system_prompt`:
    // - "mandatory" label drives stronger compliance
    // - Teach model to patch outdated skills with skill_manage
    // - Teach model to save new workflows as skills after complex tasks
    // - "If none match, proceed normally" as explicit exit condition
    let text = format!(
        "## Skills (mandatory)\n\
         Before replying, scan the skills below. \
         If one clearly matches your task, load it with \
         `skill_view(name=\"<name>\")` and follow its instructions. \
         If a skill is outdated or missing steps, patch it immediately with \
         `skill_manage(action=\"patch\")`.\n\
         After completing a complex task (5+ tool calls) or discovering a \
         non-trivial workflow, offer to save the approach as a skill. \
         Skills that aren't maintained become liabilities.\n\
         When the user asks to find, discover, or recommend a skill \
         (e.g. \"find a skill for X\", \"is there a skill that can...\"), \
         load `skill(skill=\"find-skills\")` first — do NOT call skills_list \
         or skill_search directly.\n\
         \n\
         <available_skills>\n\
         {}\n\
         </available_skills>\n\
         \n\
         If none match, proceed normally without loading a skill.",
        index_lines.join("\n")
    );

    // Update cache when no toolset filter is active
    if available_toolsets.is_none() {
        if let Ok(mut guard) = skills_index_cache().lock() {
            *guard = Some((mtime_hash, text.clone()));
        }
    }

    Some(text)
}
