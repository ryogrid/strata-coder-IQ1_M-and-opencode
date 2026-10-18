# AGENT.md

## Build rules

- Fix every issue pointed out by compiler warnings.
- The codebase must stay warning-free: `cargo build` and `cargo test`
  must print zero `warning:` lines.
- Fix warnings by changing the code (e.g. remove unnecessary `mut`,
  write `loop { ... }` instead of `while true { ... }`, drop or use
  unused variables), never by silencing them with `#[allow(...)]`,
  `#![allow(...)]`, or by filtering output.
