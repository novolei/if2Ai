# Coding Style and Lint Contract

> **本文件是 If2Ai Rust 代码的完整规范**。executor 每次启动时必须读取本文件。  
> 所有规则均为强制约束，违反任何规则将导致 REVIEW_FAIL。

## What We Borrow From `claw-code`

- Keep the system layered: runtime, service, and app should stay separate unless a slice is intentionally cross-cutting.
- Keep generated bootstrap and harness files aligned with actual workflows.
- Update docs and tests together when behavior changes.
- Prefer small, reviewable slices over broad, entangled changes.
- Treat workspace lint settings as a hard contract, not a suggestion.

## What We Borrow From `zeroclaw`

- Keep the CLI entrypoint explicit and well documented.
- Keep subcommands discoverable through clear help text.
- Use module boundaries to keep features organized and easy to navigate.
- Prefer a thin binary entrypoint that delegates to bounded modules.
- Use descriptive command docs and examples so the tool is readable without context.

## Code Shape Rules

- One file should own one bounded responsibility.
- Prefer small helper functions over long nested blocks.
- Keep `main.rs` or bin entrypoints as orchestration, not business logic.
- Put shared behavior in modules with intentional exports.
- Prefer `pub(crate)` by default and expose `pub` only when the boundary is real.
- Use enums and newtypes when they make invalid states impossible.
- Use `Result` for fallible paths; avoid `unwrap()` and `expect()` outside tests.
- Add doc comments for public APIs and module-level summaries for important boundaries.

## Modularization Rules

- Split by bounded context, not by incidental file size.
- Keep service contracts separate from app rendering and runtime execution.
- If a module starts accumulating unrelated responsibilities, split it before adding more behavior.
- Do not create a new module just to hide code; create it to express an ownership boundary.
- Re-export only what the next layer truly needs.

## Lint Contract

These commands are mandatory before a slice is considered complete:

- `cargo fmt --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`

Additional expectations:

- obey the workspace lint contract in `rust/Cargo.toml`
- do not add new `allow(...)` annotations unless the slice has a clear reason
- if a lint must be suppressed, document why in the code or the slice record
- use clippy warnings as a signal to simplify or split modules

## Review Contract

- If a change adds a large file, ask whether it should be split.
- If a change crosses app and service boundaries, verify that the contract is still one slice.
- If documentation lags behind code, pause and update the docs before widening scope.

## Completion Contract

- Do not call a slice done until formatting, linting, tests, and docs are updated.
- Do not move to the next slice until the current slice is committed.
- Keep the detailed Rust rules in [rust-code-style-contract.md](./rust-code-style-contract.md) aligned with the active workspace shape.
