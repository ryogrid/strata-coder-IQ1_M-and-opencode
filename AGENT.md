# AGENT.md

## Serena MCP server

- Use the Serena MCP server (configured as `serena` in `opencode.json`:
  `serena start-mcp-server --project <this repository root> --context
  agent`) to reference, navigate and modify the Rust code: symbol search,
  symbol bodies, cross-references and code search go through Serena's MCP
  tools when they are available; plain file read/edit tools are the
  fallback.
- At the start of every session, before working on the user's first
  request, run the instructions command via the shell:

  ```sh
  serena print-system-prompt /home/ryo/work/opencode-strata-try --only-instructions --context agent
  ```

  and follow the instructions it returns.
- The project configuration is `.serena/project.yml` (language server:
  rust); keep it committed and let Serena index the project (the LSP
  language server is downloaded automatically on first use).

## Build rules

- Fix every issue pointed out by compiler warnings.
- The codebase must stay warning-free: `cargo build` and `cargo test`
  must print zero `warning:` lines.
- Same for lints: `cargo clippy --all-targets` (test code included) must
  print zero `warning:` lines.
  Apply `cargo clippy --fix` first, then hand-fix the remainder
  (e.g. `&String` params -> `&str`, manual loop counters -> `enumerate`,
  `checked_div(...).unwrap_or(...)`, `if let` for single-pattern matches).
- Fix warnings by changing the code (e.g. remove unnecessary `mut`,
  write `loop { ... }` instead of `while true { ... }`, drop or use
  unused variables), never by silencing them with `#[allow(...)]`,
  `#![allow(...)]`, or by filtering output.
