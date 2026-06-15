# `rev` — The Time-Traveler Runtime

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)]()
[![Platform Support](https://img.shields.io/badge/platform-Linux%20%7C%20Windows-blue.svg)]()

> **Tagline:** Every execution crash comes with a full flight recorder.  
> **One-liner pitch:** Prefix any command with `rev` (or initialize automatic project tracking) and get instant, zero-setup time-travel debugging.

`rev` is a language-agnostic runtime execution recorder and replayer. It wraps interpreter-based program execution, silently records non-deterministic inputs and memory state deltas during the run, and—on crash or explicit checkpoint—drops the developer into an interactive terminal-based TUI where they can scrub backward and forward through the program's full execution history.

---

## Key Features

- 🛠️ **Zero-Friction UX:** Wrap commands using `rev python script.py` or use `rev --init` to automatically record all project executions under the hood.
- ⚡ **Native Windows Debugger Engine:** Uses native low-level Windows APIs (`CreateProcessW`, `DebugActiveProcess`, and structured exception handler loops) to intercept execution events without external heavy dependencies.
- 🐧 **Linux ptrace Backend:** Full syscall interception and instruction tracking under Linux.
- 🌳 **SQLite Merkle DAG State storage:** Computes differential pages on memory step boundaries using virtual memory tracking (`VirtualQueryEx` on Windows, `/proc/<pid>/maps` soft-dirty page bits on Linux) and builds a cryptographic Merkle DAG to store history with minimal disk footprint.
- 🖥️ **Interactive TUI:** Step back and forth through steps, scrub execution timelines, inspect variable values, and view detailed syscall reports.
- 📦 **Export Engine:** Export complete program states at any step to JSON files for external analysis.

---

## Installation & Setup

Ensure you have Rust and Cargo installed, then build the workspace:
```bash
cargo build --release
```

Add the target executable `target/release/rev-cli` to your system path or alias it as `rev`.

---

## Usage Guide

### 1. Manual Recording (Prefix Mode)
Prefix your standard execution command with `rev`:
```bash
rev python main.py
```
If your script crashes, the interactive TUI will open automatically. To record a run without opening the TUI, use `--no-tui`:
```bash
rev --no-tui python main.py
```

### 2. Automatic Project Tracking (Like Git)
No need to prefix every command! Initialize `rev` in your project root:
```bash
rev --init
```
This automatically:
- **Shims local virtual environments** (`.venv`, `venv`, `env`): Replaces the virtual environment's `python.exe` with a `rev` wrapper that automatically records runs and forwards execution to the original python binary.
- **Rewrites `package.json` scripts**: Appends `rev` to any Node/Python scripts so that `npm start` or `npm run dev` automatically records executions.

To disable automatic tracking and restore all files back to their original state:
```bash
rev --uninit
```

### 3. Replaying Existing Traces
Launch the interactive timeline scrubber directly on a saved trace:
```bash
rev --replay ~/.rev/traces/12345_1781526766.rev-trace
```

### 4. Exporting Program State
Export the complete program state (variables, call stack, events) at any step to standard output:
```bash
rev --export ~/.rev/traces/12345_1781526766.rev-trace <STEP_ID>
```

---

## Project Architecture

```
├── core/             # Shared types, error handling, and serialization constants
├── interceptor/      # Low-level platform event listeners (ptrace on Linux, Win32 Debugger on Windows)
├── recorder/         # Trace compressor and file writer/reader engines (LZ4 + CRC32)
├── delta/            # Memory page tracking (VirtualQueryEx / clear_refs) and Merkle DB storage
├── replay/           # Reconstructs states by fast-forwarding headless interpreters
├── tui/              # Interactive Crossterm + Ratatui UI panels and timeline scrubber
└── cli/              # Entrypoint, argument parsing, shim handlers, and orchestrator
```

---

## Platform Support Matrix

| Feature | Linux | Windows |
| :--- | :---: | :---: |
| **Syscall Interception** | Full (`ptrace`) | Full (SEH Breakpoint Hooking) |
| **Memory Tracking** | Yes (Soft-Dirty bits) | Yes (VirtualQueryEx cache) |
| **State Reconstruction** | Full | Preview (Metadata only) |
| **TUI & Scrubbing** | Yes | Yes |
| **Venv & Script Shimming** | Yes | Yes |

---

## Development & Test

To run all unit and integration tests across the workspace:
```bash
cargo test --workspace
```

To run clippy lint checks:
```bash
cargo clippy --workspace --all-targets -- -D warnings
```

---

## License

Distributed under the MIT License. See [LICENSE](LICENSE) for more information.
