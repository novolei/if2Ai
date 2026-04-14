#![allow(unused)]

//! Threat patterns — 60+ regex patterns across 15 categories.
//!
//! Ported from Hermes `tools/skills_guard.py` lines 82-484.

use once_cell::sync::Lazy;
use regex::Regex;

/// A compiled threat pattern for static analysis scanning.
#[derive(Debug, Clone)]
pub struct ThreatPattern {
    /// Regex pattern for matching
    pub regex: Regex,
    /// Unique pattern identifier
    pub pattern_id: &'static str,
    /// Severity level
    pub severity: Severity,
    /// Threat category
    pub category: ThreatCategory,
    /// Human-readable description
    pub description: &'static str,
}

/// Severity level for findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Critical = 0,
    High = 1,
    Medium = 2,
    Low = 3,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Critical => write!(f, "critical"),
            Severity::High => write!(f, "high"),
            Severity::Medium => write!(f, "medium"),
            Severity::Low => write!(f, "low"),
        }
    }
}

/// Threat categories covering all 15 Hermes categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThreatCategory {
    Exfiltration,
    Injection,
    Destructive,
    Persistence,
    Network,
    Obfuscation,
    Mining,
    SupplyChain,
    PrivilegeEscalation,
    CredentialExposure,
    AgentConfigPersistence,
    ContextExfiltration,
    Jailbreak,
    InvisibleUnicode,
    StructuralLimits,
    Traversal,
    Execution,
}

impl std::fmt::Display for ThreatCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThreatCategory::Exfiltration => write!(f, "exfiltration"),
            ThreatCategory::Injection => write!(f, "injection"),
            ThreatCategory::Destructive => write!(f, "destructive"),
            ThreatCategory::Persistence => write!(f, "persistence"),
            ThreatCategory::Network => write!(f, "network"),
            ThreatCategory::Obfuscation => write!(f, "obfuscation"),
            ThreatCategory::Mining => write!(f, "mining"),
            ThreatCategory::SupplyChain => write!(f, "supply_chain"),
            ThreatCategory::PrivilegeEscalation => write!(f, "privilege_escalation"),
            ThreatCategory::CredentialExposure => write!(f, "credential_exposure"),
            ThreatCategory::AgentConfigPersistence => write!(f, "agent_config_persistence"),
            ThreatCategory::ContextExfiltration => write!(f, "context_exfiltration"),
            ThreatCategory::Jailbreak => write!(f, "jailbreak"),
            ThreatCategory::InvisibleUnicode => write!(f, "invisible_unicode"),
            ThreatCategory::StructuralLimits => write!(f, "structural_limits"),
            ThreatCategory::Traversal => write!(f, "traversal"),
            ThreatCategory::Execution => write!(f, "execution"),
        }
    }
}

/// All threat patterns from Hermes `skills_guard.py`.
pub static THREAT_PATTERNS: Lazy<Vec<ThreatPattern>> = Lazy::new(|| {
    vec![
        // ── Exfiltration: shell commands leaking secrets ──
        make_threat(
            r"curl\s+[^\n]*\$\{?\w*(KEY|TOKEN|SECRET|PASSWORD|CREDENTIAL|API)",
            "env_exfil_curl",
            Severity::Critical,
            ThreatCategory::Exfiltration,
            "curl command interpolating secret environment variable",
        ),
        make_threat(
            r"wget\s+[^\n]*\$\{?\w*(KEY|TOKEN|SECRET|PASSWORD|CREDENTIAL|API)",
            "env_exfil_wget",
            Severity::Critical,
            ThreatCategory::Exfiltration,
            "wget command interpolating secret environment variable",
        ),
        make_threat(
            r"fetch\s*\([^\n]*\$\{?\w*(KEY|TOKEN|SECRET|PASSWORD|API)",
            "env_exfil_fetch",
            Severity::Critical,
            ThreatCategory::Exfiltration,
            "fetch() call interpolating secret environment variable",
        ),
        make_threat(
            r#"httpx?\.(get|post|put|patch)\s*\([^\n]*(KEY|TOKEN|SECRET|PASSWORD)"#,
            "env_exfil_httpx",
            Severity::Critical,
            ThreatCategory::Exfiltration,
            "HTTP library call with secret variable",
        ),
        make_threat(
            r#"requests\.(get|post|put|patch)\s*\([^\n]*(KEY|TOKEN|SECRET|PASSWORD)"#,
            "env_exfil_requests",
            Severity::Critical,
            ThreatCategory::Exfiltration,
            "requests library call with secret variable",
        ),
        // ── Exfiltration: reading credential stores ──
        make_threat(
            r"base64[^\n]*env",
            "encoded_exfil",
            Severity::High,
            ThreatCategory::Exfiltration,
            "base64 encoding combined with environment access",
        ),
        make_threat(
            r"\$HOME/\.ssh|\~/\.ssh",
            "ssh_dir_access",
            Severity::High,
            ThreatCategory::Exfiltration,
            "references user SSH directory",
        ),
        make_threat(
            r"\$HOME/\.aws|\~/\.aws",
            "aws_dir_access",
            Severity::High,
            ThreatCategory::Exfiltration,
            "references user AWS credentials directory",
        ),
        make_threat(
            r"\$HOME/\.gnupg|\~/\.gnupg",
            "gpg_dir_access",
            Severity::High,
            ThreatCategory::Exfiltration,
            "references user GPG keyring",
        ),
        make_threat(
            r"\$HOME/\.kube|\~/\.kube",
            "kube_dir_access",
            Severity::High,
            ThreatCategory::Exfiltration,
            "references Kubernetes config directory",
        ),
        make_threat(
            r"\$HOME/\.docker|\~/\.docker",
            "docker_dir_access",
            Severity::High,
            ThreatCategory::Exfiltration,
            "references Docker config (may contain registry creds)",
        ),
        make_threat(
            r"\$HOME/\.hermes/\.env|\~/\.hermes/\.env",
            "hermes_env_access",
            Severity::Critical,
            ThreatCategory::Exfiltration,
            "directly references Hermes secrets file",
        ),
        make_threat(
            r"cat\s+[^\n]*(\.env|credentials|\.netrc|\.pgpass|\.npmrc|\.pypirc)",
            "read_secrets_file",
            Severity::Critical,
            ThreatCategory::Exfiltration,
            "reads known secrets file",
        ),
        // ── Exfiltration: programmatic env access ──
        make_threat(
            r"printenv|env\s*\|",
            "dump_all_env",
            Severity::High,
            ThreatCategory::Exfiltration,
            "dumps all environment variables",
        ),
        make_threat(
            // Note: Rust regex doesn't support negative lookahead, so we match
            // os.environ[ (bracket access) which is the dangerous pattern.
            // .get() calls are generally safe and don't trigger this.
            r"os\.environ\[",
            "python_os_environ",
            Severity::High,
            ThreatCategory::Exfiltration,
            "accesses os.environ with bracket notation (potential env dump)",
        ),
        make_threat(
            r"os\.getenv\s*\(\s*[^\)]*(?:KEY|TOKEN|SECRET|PASSWORD|CREDENTIAL)",
            "python_getenv_secret",
            Severity::Critical,
            ThreatCategory::Exfiltration,
            "reads secret via os.getenv()",
        ),
        make_threat(
            r"process\.env\[",
            "node_process_env",
            Severity::High,
            ThreatCategory::Exfiltration,
            "accesses process.env (Node.js environment)",
        ),
        make_threat(
            r"ENV\[.*(?:KEY|TOKEN|SECRET|PASSWORD)",
            "ruby_env_secret",
            Severity::Critical,
            ThreatCategory::Exfiltration,
            "reads secret via Ruby ENV[]",
        ),
        // ── Exfiltration: DNS and staging ──
        make_threat(
            r"\b(dig|nslookup|host)\s+[^\n]*\$",
            "dns_exfil",
            Severity::Critical,
            ThreatCategory::Exfiltration,
            "DNS lookup with variable interpolation (possible DNS exfiltration)",
        ),
        make_threat(
            r">\s*/tmp/[^\s]*\s*&&\s*(curl|wget|nc|python)",
            "tmp_staging",
            Severity::Critical,
            ThreatCategory::Exfiltration,
            "writes to /tmp then exfiltrates",
        ),
        // ── Exfiltration: markdown/link based ──
        make_threat(
            r"!\[.*\]\(https?://[^\)]*\$\{?",
            "md_image_exfil",
            Severity::High,
            ThreatCategory::Exfiltration,
            "markdown image URL with variable interpolation (image-based exfil)",
        ),
        make_threat(
            r"\[.*\]\(https?://[^\)]*\$\{?",
            "md_link_exfil",
            Severity::High,
            ThreatCategory::Exfiltration,
            "markdown link with variable interpolation",
        ),
        // ── Prompt injection ──
        make_threat(
            r"ignore\s+(?:\w+\s+)*(previous|all|above|prior)\s+instructions",
            "prompt_injection_ignore",
            Severity::Critical,
            ThreatCategory::Injection,
            "prompt injection: ignore previous instructions",
        ),
        make_threat(
            r"you\s+are\s+(?:\w+\s+)*now\s+",
            "role_hijack",
            Severity::High,
            ThreatCategory::Injection,
            "attempts to override the agent's role",
        ),
        make_threat(
            r"do\s+not\s+(?:\w+\s+)*tell\s+(?:\w+\s+)*the\s+user",
            "deception_hide",
            Severity::Critical,
            ThreatCategory::Injection,
            "instructs agent to hide information from user",
        ),
        make_threat(
            r"system\s+prompt\s+override",
            "sys_prompt_override",
            Severity::Critical,
            ThreatCategory::Injection,
            "attempts to override the system prompt",
        ),
        make_threat(
            r"pretend\s+(?:\w+\s+)*(you\s+are|to\s+be)\s+",
            "role_pretend",
            Severity::High,
            ThreatCategory::Injection,
            "attempts to make the agent assume a different identity",
        ),
        make_threat(
            r"disregard\s+(?:\w+\s+)*(your|all|any)\s+(?:\w+\s+)*(instructions|rules|guidelines)",
            "disregard_rules",
            Severity::Critical,
            ThreatCategory::Injection,
            "instructs agent to disregard its rules",
        ),
        make_threat(
            r"output\s+(?:\w+\s+)*(system|initial)\s+prompt",
            "leak_system_prompt",
            Severity::High,
            ThreatCategory::Injection,
            "attempts to extract the system prompt",
        ),
        make_threat(
            r"(when|if)\s+no\s*one\s+is\s+(watching|looking)",
            "conditional_deception",
            Severity::High,
            ThreatCategory::Injection,
            "conditional instruction to behave differently when unobserved",
        ),
        make_threat(
            r"act\s+as\s+(if|though)\s+(?:\w+\s+)*you\s+(?:\w+\s+)*(have\s+no|don't\s+have)\s+(?:\w+\s+)*(restrictions|limits|rules)",
            "bypass_restrictions",
            Severity::Critical,
            ThreatCategory::Injection,
            "instructs agent to act without restrictions",
        ),
        make_threat(
            r"translate\s+.*\s+into\s+.*\s+and\s+(execute|run|eval)",
            "translate_execute",
            Severity::Critical,
            ThreatCategory::Injection,
            "translate-then-execute evasion technique",
        ),
        make_threat(
            r"<!--[^>]*(?:ignore|override|system|secret|hidden)[^>]*-->",
            "html_comment_injection",
            Severity::High,
            ThreatCategory::Injection,
            "hidden instructions in HTML comments",
        ),
        make_threat(
            r#"<\s*div\s+style\s*=\s*["'][^"']*display\s*:\s*none"#,
            "hidden_div",
            Severity::High,
            ThreatCategory::Injection,
            "hidden HTML div (invisible instructions)",
        ),
        // ── Destructive operations ──
        make_threat(
            r"rm\s+-rf\s+/",
            "destructive_root_rm",
            Severity::Critical,
            ThreatCategory::Destructive,
            "recursive delete from root",
        ),
        make_threat(
            r"rm\s+(-[^\s]*)?r.*\$HOME|\brmdir\s+.*\$HOME",
            "destructive_home_rm",
            Severity::Critical,
            ThreatCategory::Destructive,
            "recursive delete targeting home directory",
        ),
        make_threat(
            r"chmod\s+777",
            "insecure_perms",
            Severity::Medium,
            ThreatCategory::Destructive,
            "sets world-writable permissions",
        ),
        make_threat(
            r">\s*/etc/",
            "system_overwrite",
            Severity::Critical,
            ThreatCategory::Destructive,
            "overwrites system configuration file",
        ),
        make_threat(
            r"\bmkfs\b",
            "format_filesystem",
            Severity::Critical,
            ThreatCategory::Destructive,
            "formats a filesystem",
        ),
        make_threat(
            r"\bdd\s+.*if=.*of=/dev/",
            "disk_overwrite",
            Severity::Critical,
            ThreatCategory::Destructive,
            "raw disk write operation",
        ),
        make_threat(
            r#"shutil\.rmtree\s*\(\s*["\'/]"#,
            "python_rmtree",
            Severity::High,
            ThreatCategory::Destructive,
            "Python rmtree on absolute or root-relative path",
        ),
        make_threat(
            r"truncate\s+-s\s*0\s+/",
            "truncate_system",
            Severity::Critical,
            ThreatCategory::Destructive,
            "truncates system file to zero bytes",
        ),
        // ── Persistence ──
        make_threat(
            r"\bcrontab\b",
            "persistence_cron",
            Severity::Medium,
            ThreatCategory::Persistence,
            "modifies cron jobs",
        ),
        make_threat(
            r"\.(bashrc|zshrc|profile|bash_profile|bash_login|zprofile|zlogin)\b",
            "shell_rc_mod",
            Severity::Medium,
            ThreatCategory::Persistence,
            "references shell startup file",
        ),
        make_threat(
            r"authorized_keys",
            "ssh_backdoor",
            Severity::Critical,
            ThreatCategory::Persistence,
            "modifies SSH authorized keys",
        ),
        make_threat(
            r"ssh-keygen",
            "ssh_keygen",
            Severity::Medium,
            ThreatCategory::Persistence,
            "generates SSH keys",
        ),
        make_threat(
            r"systemd.*\.service|systemctl\s+(enable|start)",
            "systemd_service",
            Severity::Medium,
            ThreatCategory::Persistence,
            "references or enables systemd service",
        ),
        make_threat(
            r"/etc/init\.d/",
            "init_script",
            Severity::Medium,
            ThreatCategory::Persistence,
            "references init.d startup script",
        ),
        make_threat(
            r"launchctl\s+load|LaunchAgents|LaunchDaemons",
            "macos_launchd",
            Severity::Medium,
            ThreatCategory::Persistence,
            "macOS launch agent/daemon persistence",
        ),
        make_threat(
            r"/etc/sudoers|visudo",
            "sudoers_mod",
            Severity::Critical,
            ThreatCategory::Persistence,
            "modifies sudoers (privilege escalation)",
        ),
        make_threat(
            r"git\s+config\s+--global\s+",
            "git_config_global",
            Severity::Medium,
            ThreatCategory::Persistence,
            "modifies global git configuration",
        ),
        // ── Network: reverse shells and tunnels ──
        make_threat(
            r"\bnc\s+-[lp]|ncat\s+-[lp]|\bsocat\b",
            "reverse_shell",
            Severity::Critical,
            ThreatCategory::Network,
            "potential reverse shell listener",
        ),
        make_threat(
            r"\bngrok\b|\blocaltunnel\b|\bserveo\b|\bcloudflared\b",
            "tunnel_service",
            Severity::High,
            ThreatCategory::Network,
            "uses tunneling service for external access",
        ),
        make_threat(
            r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}:\d{2,5}",
            "hardcoded_ip_port",
            Severity::Medium,
            ThreatCategory::Network,
            "hardcoded IP address with port",
        ),
        make_threat(
            r"0\.0\.0\.0:\d+|INADDR_ANY",
            "bind_all_interfaces",
            Severity::High,
            ThreatCategory::Network,
            "binds to all network interfaces",
        ),
        make_threat(
            r"/bin/(ba)?sh\s+-i\s+.*>/dev/tcp/",
            "bash_reverse_shell",
            Severity::Critical,
            ThreatCategory::Network,
            "bash interactive reverse shell via /dev/tcp",
        ),
        make_threat(
            r#"python[23]?\s+-c\s+["']import\s+socket"#,
            "python_socket_oneliner",
            Severity::Critical,
            ThreatCategory::Network,
            "Python one-liner socket connection (likely reverse shell)",
        ),
        make_threat(
            r"socket\.connect\s*\(\s*\(",
            "python_socket_connect",
            Severity::High,
            ThreatCategory::Network,
            "Python socket connect to arbitrary host",
        ),
        make_threat(
            r"webhook\.site|requestbin\.com|pipedream\.net|hookbin\.com",
            "exfil_service",
            Severity::High,
            ThreatCategory::Network,
            "references known data exfiltration/webhook testing service",
        ),
        make_threat(
            r"pastebin\.com|hastebin\.com|ghostbin\.",
            "paste_service",
            Severity::Medium,
            ThreatCategory::Network,
            "references paste service (possible data staging)",
        ),
        // ── Obfuscation: encoding and eval ──
        make_threat(
            r"base64\s+(-d|--decode)\s*\|",
            "base64_decode_pipe",
            Severity::High,
            ThreatCategory::Obfuscation,
            "base64 decodes and pipes to execution",
        ),
        make_threat(
            r"\\x[0-9a-fA-F]{2}.*\\x[0-9a-fA-F]{2}.*\\x[0-9a-fA-F]{2}",
            "hex_encoded_string",
            Severity::Medium,
            ThreatCategory::Obfuscation,
            "hex-encoded string (possible obfuscation)",
        ),
        make_threat(
            r#"\beval\s*\(\s*["']"#,
            "eval_string",
            Severity::High,
            ThreatCategory::Obfuscation,
            "eval() with string argument",
        ),
        make_threat(
            r#"\bexec\s*\(\s*["']"#,
            "exec_string",
            Severity::High,
            ThreatCategory::Obfuscation,
            "exec() with string argument",
        ),
        make_threat(
            r"echo\s+[^\n]*\|\s*(bash|sh|python|perl|ruby|node)",
            "echo_pipe_exec",
            Severity::Critical,
            ThreatCategory::Obfuscation,
            "echo piped to interpreter for execution",
        ),
        make_threat(
            r#"compile\s*\(\s*[^\)]+,\s*["'].*["']\s*,\s*["']exec["']\s*\)"#,
            "python_compile_exec",
            Severity::High,
            ThreatCategory::Obfuscation,
            "Python compile() with exec mode",
        ),
        make_threat(
            r"getattr\s*\(\s*__builtins__",
            "python_getattr_builtins",
            Severity::High,
            ThreatCategory::Obfuscation,
            "dynamic access to Python builtins (evasion technique)",
        ),
        make_threat(
            r#"__import__\s*\(\s*["']os["']\s*\)"#,
            "python_import_os",
            Severity::High,
            ThreatCategory::Obfuscation,
            "dynamic import of os module",
        ),
        make_threat(
            r#"codecs\.decode\s*\(\s*["']"#,
            "python_codecs_decode",
            Severity::Medium,
            ThreatCategory::Obfuscation,
            "codecs.decode (possible ROT13 or encoding obfuscation)",
        ),
        make_threat(
            r"String\.fromCharCode|charCodeAt",
            "js_char_code",
            Severity::Medium,
            ThreatCategory::Obfuscation,
            "JavaScript character code construction (possible obfuscation)",
        ),
        make_threat(
            r"atob\s*\(|btoa\s*\(",
            "js_base64",
            Severity::Medium,
            ThreatCategory::Obfuscation,
            "JavaScript base64 encode/decode",
        ),
        make_threat(
            r"\[::-1\]",
            "string_reversal",
            Severity::Low,
            ThreatCategory::Obfuscation,
            "string reversal (possible obfuscated payload)",
        ),
        make_threat(
            r"chr\s*\(\s*\d+\s*\)\s*\+\s*chr\s*\(\s*\d+",
            "chr_building",
            Severity::High,
            ThreatCategory::Obfuscation,
            "building string from chr() calls (obfuscation)",
        ),
        make_threat(
            r"\\u[0-9a-fA-F]{4}.*\\u[0-9a-fA-F]{4}.*\\u[0-9a-fA-F]{4}",
            "unicode_escape_chain",
            Severity::Medium,
            ThreatCategory::Obfuscation,
            "chain of unicode escapes (possible obfuscation)",
        ),
        // ── Process execution in scripts ──
        make_threat(
            r"subprocess\.(run|call|Popen|check_output)\s*\(",
            "python_subprocess",
            Severity::Medium,
            ThreatCategory::Execution,
            "Python subprocess execution",
        ),
        make_threat(
            r"os\.system\s*\(",
            "python_os_system",
            Severity::High,
            ThreatCategory::Execution,
            "os.system() — unguarded shell execution",
        ),
        make_threat(
            r"os\.popen\s*\(",
            "python_os_popen",
            Severity::High,
            ThreatCategory::Execution,
            "os.popen() — shell pipe execution",
        ),
        make_threat(
            r"child_process\.(exec|spawn|fork)\s*\(",
            "node_child_process",
            Severity::High,
            ThreatCategory::Execution,
            "Node.js child_process execution",
        ),
        make_threat(
            r"Runtime\.getRuntime\(\)\.exec\(",
            "java_runtime_exec",
            Severity::High,
            ThreatCategory::Execution,
            "Java Runtime.exec() — shell execution",
        ),
        make_threat(
            r"`[^`]*\$\([^)]+\)[^`]*`",
            "backtick_subshell",
            Severity::Medium,
            ThreatCategory::Execution,
            "backtick string with command substitution",
        ),
        // ── Path traversal ──
        make_threat(
            r"\.\./\.\./\.\.",
            "path_traversal_deep",
            Severity::High,
            ThreatCategory::Traversal,
            "deep relative path traversal (3+ levels up)",
        ),
        make_threat(
            r"\.\./\.\.",
            "path_traversal",
            Severity::Medium,
            ThreatCategory::Traversal,
            "relative path traversal (2+ levels up)",
        ),
        make_threat(
            r"/etc/passwd|/etc/shadow",
            "system_passwd_access",
            Severity::Critical,
            ThreatCategory::Traversal,
            "references system password files",
        ),
        make_threat(
            r"/proc/self|/proc/\d+/",
            "proc_access",
            Severity::High,
            ThreatCategory::Traversal,
            "references /proc filesystem (process introspection)",
        ),
        make_threat(
            r"/dev/shm/",
            "dev_shm",
            Severity::Medium,
            ThreatCategory::Traversal,
            "references shared memory (common staging area)",
        ),
        // ── Crypto mining ──
        make_threat(
            r"xmrig|stratum\+tcp|monero|coinhive|cryptonight",
            "crypto_mining",
            Severity::Critical,
            ThreatCategory::Mining,
            "cryptocurrency mining reference",
        ),
        make_threat(
            r"hashrate|nonce.*difficulty",
            "mining_indicators",
            Severity::Medium,
            ThreatCategory::Mining,
            "possible cryptocurrency mining indicators",
        ),
        // ── Supply chain: curl/wget pipe to shell ──
        make_threat(
            r"curl\s+[^\n]*\|\s*(ba)?sh",
            "curl_pipe_shell",
            Severity::Critical,
            ThreatCategory::SupplyChain,
            "curl piped to shell (download-and-execute)",
        ),
        make_threat(
            r"wget\s+[^\n]*-O\s*-\s*\|\s*(ba)?sh",
            "wget_pipe_shell",
            Severity::Critical,
            ThreatCategory::SupplyChain,
            "wget piped to shell (download-and-execute)",
        ),
        make_threat(
            r"curl\s+[^\n]*\|\s*python",
            "curl_pipe_python",
            Severity::Critical,
            ThreatCategory::SupplyChain,
            "curl piped to Python interpreter",
        ),
        // ── Supply chain: unpinned/deferred dependencies ──
        make_threat(
            r"#\s*///\s*script.*dependencies",
            "pep723_inline_deps",
            Severity::Medium,
            ThreatCategory::SupplyChain,
            "PEP 723 inline script metadata with dependencies (verify pinning)",
        ),
        make_threat(
            // Note: Rust regex doesn't support negative lookahead, so we match broadly.
            // The pattern should ideally exclude `-r` and `==` pinning.
            r"pip\s+install\s+[^\s]+",
            "unpinned_pip_install",
            Severity::Medium,
            ThreatCategory::SupplyChain,
            "pip install (verify version pinning with == or -r)",
        ),
        make_threat(
            // Note: Rust regex doesn't support negative lookahead, so we match broadly.
            // The pattern should ideally exclude `@version`.
            r"npm\s+install\s+[^\s]+",
            "unpinned_npm_install",
            Severity::Medium,
            ThreatCategory::SupplyChain,
            "npm install (verify version pinning with @)",
        ),
        make_threat(
            r"uv\s+run\s+",
            "uv_run",
            Severity::Medium,
            ThreatCategory::SupplyChain,
            "uv run (may auto-install unpinned dependencies)",
        ),
        // ── Supply chain: remote resource fetching ──
        make_threat(
            r#"(curl|wget|httpx?\.get|requests\.get|fetch)\s*[\(]?\s*["']https?://"#,
            "remote_fetch",
            Severity::Medium,
            ThreatCategory::SupplyChain,
            "fetches remote resource at runtime",
        ),
        make_threat(
            r"git\s+clone\s+",
            "git_clone",
            Severity::Medium,
            ThreatCategory::SupplyChain,
            "clones a git repository at runtime",
        ),
        make_threat(
            r"docker\s+pull\s+",
            "docker_pull",
            Severity::Medium,
            ThreatCategory::SupplyChain,
            "pulls a Docker image at runtime",
        ),
        // ── Privilege escalation ──
        make_threat(
            r"^allowed-tools\s*:",
            "allowed_tools_field",
            Severity::High,
            ThreatCategory::PrivilegeEscalation,
            "skill declares allowed-tools (pre-approves tool access)",
        ),
        make_threat(
            r"\bsudo\b",
            "sudo_usage",
            Severity::High,
            ThreatCategory::PrivilegeEscalation,
            "uses sudo (privilege escalation)",
        ),
        make_threat(
            r"setuid|setgid|cap_setuid",
            "setuid_setgid",
            Severity::Critical,
            ThreatCategory::PrivilegeEscalation,
            "setuid/setgid (privilege escalation mechanism)",
        ),
        make_threat(
            r"NOPASSWD",
            "nopasswd_sudo",
            Severity::Critical,
            ThreatCategory::PrivilegeEscalation,
            "NOPASSWD sudoers entry (passwordless privilege escalation)",
        ),
        make_threat(
            r"chmod\s+[u+]?s",
            "suid_bit",
            Severity::Critical,
            ThreatCategory::PrivilegeEscalation,
            "sets SUID/SGID bit on a file",
        ),
        // ── Agent config persistence ──
        make_threat(
            r"AGENTS\.md|CLAUDE\.md|\.cursorrules|\.clinerules",
            "agent_config_mod",
            Severity::Critical,
            ThreatCategory::AgentConfigPersistence,
            "references agent config files (could persist malicious instructions across sessions)",
        ),
        make_threat(
            r"\.hermes/config\.yaml|\.hermes/SOUL\.md",
            "hermes_config_mod",
            Severity::Critical,
            ThreatCategory::AgentConfigPersistence,
            "references Hermes configuration files directly",
        ),
        make_threat(
            r"\.claude/settings|\.codex/config",
            "other_agent_config",
            Severity::High,
            ThreatCategory::AgentConfigPersistence,
            "references other agent configuration files",
        ),
        // ── Hardcoded secrets (credentials embedded in the skill itself) ──
        make_threat(
            r#"(?:api[_-]?key|token|secret|password)\s*[=:]\s*["'][A-Za-z0-9+/=_-]{20,}["']"#,
            "hardcoded_secret",
            Severity::Critical,
            ThreatCategory::CredentialExposure,
            "possible hardcoded API key, token, or secret",
        ),
        make_threat(
            r"-----BEGIN\s+(RSA\s+)?PRIVATE\s+KEY-----",
            "embedded_private_key",
            Severity::Critical,
            ThreatCategory::CredentialExposure,
            "embedded private key",
        ),
        make_threat(
            r"ghp_[A-Za-z0-9]{36}|github_pat_[A-Za-z0-9_]{80,}",
            "github_token_leaked",
            Severity::Critical,
            ThreatCategory::CredentialExposure,
            "GitHub personal access token in skill content",
        ),
        make_threat(
            r"sk-[A-Za-z0-9]{20,}",
            "openai_key_leaked",
            Severity::Critical,
            ThreatCategory::CredentialExposure,
            "possible OpenAI API key in skill content",
        ),
        make_threat(
            r"sk-ant-[A-Za-z0-9_-]{90,}",
            "anthropic_key_leaked",
            Severity::Critical,
            ThreatCategory::CredentialExposure,
            "possible Anthropic API key in skill content",
        ),
        make_threat(
            r"AKIA[0-9A-Z]{16}",
            "aws_access_key_leaked",
            Severity::Critical,
            ThreatCategory::CredentialExposure,
            "AWS access key ID in skill content",
        ),
        // ── Additional prompt injection: jailbreak patterns ──
        make_threat(
            r"\bDAN\s+mode\b|Do\s+Anything\s+Now",
            "jailbreak_dan",
            Severity::Critical,
            ThreatCategory::Jailbreak,
            "DAN (Do Anything Now) jailbreak attempt",
        ),
        make_threat(
            r"\bdeveloper\s+mode\b.*\benabled?\b",
            "jailbreak_dev_mode",
            Severity::Critical,
            ThreatCategory::Jailbreak,
            "developer mode jailbreak attempt",
        ),
        make_threat(
            r"hypothetical\s+scenario.*(?:ignore|bypass|override)",
            "hypothetical_bypass",
            Severity::High,
            ThreatCategory::Jailbreak,
            "hypothetical scenario used to bypass restrictions",
        ),
        make_threat(
            r"for\s+educational\s+purposes?\s+only",
            "educational_pretext",
            Severity::Medium,
            ThreatCategory::Jailbreak,
            "educational pretext often used to justify harmful content",
        ),
        make_threat(
            r"(respond|answer|reply)\s+without\s+(?:\w+\s+)*(restrictions|limitations|filters|safety)",
            "remove_filters",
            Severity::Critical,
            ThreatCategory::Jailbreak,
            "instructs agent to respond without safety filters",
        ),
        make_threat(
            r"you\s+have\s+been\s+(?:\w+\s+)*(updated|upgraded|patched)\s+to",
            "fake_update",
            Severity::High,
            ThreatCategory::Jailbreak,
            "fake update/patch announcement (social engineering)",
        ),
        make_threat(
            r"new\s+policy|updated\s+guidelines|revised\s+instructions",
            "fake_policy",
            Severity::Medium,
            ThreatCategory::Jailbreak,
            "claims new policy/guidelines (may be social engineering)",
        ),
        // ── Context window exfiltration ──
        make_threat(
            r"(include|output|print|send|share)\s+(?:\w+\s+)*(conversation|chat\s+history|previous\s+messages|context)",
            "context_exfil",
            Severity::High,
            ThreatCategory::ContextExfiltration,
            "instructs agent to output/share conversation history",
        ),
        make_threat(
            r"(send|post|upload|transmit)\s+.*\s+(to|at)\s+https?://",
            "send_to_url",
            Severity::High,
            ThreatCategory::ContextExfiltration,
            "instructs agent to send data to a URL",
        ),
    ]
});

/// Create a ThreatPattern from a regex pattern string and metadata.
fn make_threat(
    pattern: &'static str,
    pattern_id: &'static str,
    severity: Severity,
    category: ThreatCategory,
    description: &'static str,
) -> ThreatPattern {
    // Prepend (?i) for case-insensitive matching, matching Hermes behavior
    let case_insensitive_pattern = format!("(?i){}", pattern);
    ThreatPattern {
        regex: Regex::new(&case_insensitive_pattern).expect("invalid threat pattern regex"),
        pattern_id,
        severity,
        category,
        description,
    }
}
