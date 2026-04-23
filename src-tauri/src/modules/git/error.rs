//! Structured error type for the git module.
//!
//! Mirrors the failure modes that the `/rust` reference implementation
//! collapsed into `io::Error::other(...)` strings, but keeps them as
//! distinct variants so callers (Tauri commands, slash dispatch, prompt
//! injection) can match precisely without parsing strings.

use std::fmt;
use std::io;
use std::string::FromUtf8Error;

/// All failure modes surfaced by the git module.
#[derive(Debug)]
pub(crate) enum GitError {
    /// Underlying IO failure when spawning a child process or reading
    /// its output.
    Io(io::Error),
    /// `git` / `gh` produced non-UTF-8 stdout where text was required.
    Utf8(FromUtf8Error),
    /// The child process exited with a non-zero status code.
    ///
    /// `exit_code` is `None` for signal-terminated children (Unix) or
    /// when the platform cannot report a code; callers that need to
    /// branch on `gh`'s exit-code semantics (1 = not found, 128 =
    /// unauthenticated, 2 = usage error) should match on `Some(_)`.
    NonZeroExit {
        program: String,
        args: Vec<String>,
        exit_code: Option<i32>,
        stderr: String,
        stdout: String,
    },
    /// A required external binary (`git`, `gh`) was not found on `PATH`.
    MissingBinary(&'static str),
    /// The supplied working directory is not inside a git repository.
    NotARepository,
    /// The working tree is in a state with no checked-out branch
    /// (e.g. detached HEAD); the repository itself is valid.  Distinct
    /// from [`GitError::NotARepository`] so callers can differentiate.
    NoBranch,
    /// A worktree was requested at a path that already exists.
    WorktreeAlreadyExists(std::path::PathBuf),
    /// A `commit` (or `commit-push-pr`) was requested but the working
    /// tree has no changes to record.  Distinct from
    /// [`GitError::NonZeroExit`] so the slash layer can render this as
    /// an idempotent "skipped" outcome rather than a failure.
    NoWorkspaceChanges,
    /// `commit` requested but no message was supplied.
    CommitMessageRequired,
    /// Caller supplied an empty / whitespace-only commit message.
    EmptyCommitMessage,
    /// A required input field (e.g. PR/Issue title) was missing or
    /// blank.  Field name is captured so the caller can render a
    /// targeted usage hint without parsing the error string.
    MissingRequired(&'static str),
    /// Output parsing (e.g. `gh pr view --json url`) failed.
    Parse(String),
    /// Catch-all for internal control-plane failures that aren't a
    /// subprocess exit (e.g. a `tokio::task::spawn_blocking` panic).
    /// Should be rare; exists so callers don't have to reach for
    /// `Parse` as a generic dumping ground.
    Internal(String),
}

impl GitError {
    /// Build a [`GitError::NonZeroExit`] from a [`std::process::Output`].
    #[must_use]
    pub fn from_output(program: &str, args: &[&str], output: &std::process::Output) -> Self {
        Self::NonZeroExit {
            program: program.to_string(),
            args: args.iter().map(|a| (*a).to_string()).collect(),
            exit_code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        }
    }
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "git io error: {error}"),
            Self::Utf8(error) => write!(f, "git output not valid UTF-8: {error}"),
            Self::NonZeroExit {
                program,
                args,
                exit_code: _,
                stderr,
                stdout,
            } => {
                let detail = if stderr.is_empty() { stdout } else { stderr };
                if detail.is_empty() {
                    write!(f, "{program} {} failed", args.join(" "))
                } else {
                    write!(f, "{program} {} failed: {detail}", args.join(" "))
                }
            }
            Self::MissingBinary(name) => write!(f, "required binary not found on PATH: {name}"),
            Self::NotARepository => write!(f, "not a git repository"),
            Self::NoBranch => write!(f, "no branch is currently checked out (detached HEAD)"),
            Self::WorktreeAlreadyExists(path) => {
                write!(f, "worktree target already exists: {}", path.display())
            }
            Self::NoWorkspaceChanges => write!(f, "no workspace changes to commit"),
            Self::CommitMessageRequired => write!(f, "commit message is required"),
            Self::EmptyCommitMessage => write!(f, "commit message is empty"),
            Self::MissingRequired(field) => write!(f, "missing required input: {field}"),
            Self::Parse(detail) => write!(f, "git output parse error: {detail}"),
            Self::Internal(detail) => write!(f, "git internal error: {detail}"),
        }
    }
}

impl std::error::Error for GitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Utf8(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for GitError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<FromUtf8Error> for GitError {
    fn from(error: FromUtf8Error) -> Self {
        Self::Utf8(error)
    }
}

impl From<GitError> for String {
    fn from(error: GitError) -> Self {
        error.to_string()
    }
}

/// Result alias used throughout the git module.
pub(crate) type GitResult<T> = Result<T, GitError>;
