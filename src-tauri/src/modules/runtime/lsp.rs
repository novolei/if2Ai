#![allow(dead_code)]

//! LSP module stub - migrated from /rust/crates/lsp
//!
//! This is a stub implementation to satisfy compilation.
//! Full LSP integration will be completed in a future slice.

#[derive(Debug, Clone, Default)]
pub struct LspContextEnrichment {
    sections: Vec<String>,
}

impl LspContextEnrichment {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    #[must_use]
    pub fn render_prompt_section(&self) -> String {
        self.sections.join("\n")
    }
}

#[derive(Debug, Clone)]
pub struct LspServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug)]
pub enum LspError {
    NotFound,
    ConnectionFailed(String),
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct LspManager {
    servers: Vec<LspServerConfig>,
}

impl LspManager {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone)]
pub struct FileDiagnostics {
    pub file_path: String,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: DiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

#[derive(Debug, Clone)]
pub struct SymbolLocation {
    pub name: String,
    pub kind: SymbolKind,
    pub location: Range,
}

#[derive(Debug, Clone, Copy)]
pub enum SymbolKind {
    File,
    Module,
    Function,
    Variable,
}

#[derive(Debug, Clone)]
pub struct WorkspaceDiagnostics {
    pub files: Vec<FileDiagnostics>,
}
