# Development environment snapshot

Dated snapshot of the machine, model serving and coding-agent setup used
to develop emur. Volatile by nature — refresh the snapshot (and its
date) when the environment changes; nothing here is a build requirement
(see README "Development environment" for the stable toolchain facts).
Model IDs/serve configurations are authoritative in `opencode.json`
only. Snapshot date: 2026-10.

## Development machine

- OS: Windows 11 Pro 25H2, OS Build 26200.9711 (the editor itself runs
  on the WSL2 x86_64 kernel under this Windows host).
- Hardware: Ryzen 7 5700X — 8 physical cores / 16 logical threads; main
  memory 64 GB; GPU Radeon RX 9060 XT 16 GB; NVMe SSD WD Black SN770
  1TB.

## Local LLM serving

- Strata, running natively on Windows 11 (not inside WSL2); serves the
  coding model over a local OpenAI-compatible endpoint; context size
  128k tokens.
- Version pinned by commit `477ac13cbe8e582c20484871ce459d20d82df09d`,
  tag `v0.1.45`.

## Coding agent

- OpenCode v1.18.31.
- Model (as configured at snapshot time): Qwen3.8-Flash-Next Coder
  IQ1_M (Strata) — model ID `strata/qwen3.8-flash-next-coder`, served
  by Strata; configured in `opencode.json` under the display name
  "Qwen3.8-Flash-Next Coder (Strata)".
