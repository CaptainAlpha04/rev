# `rev` — Time-Traveler Runtime

> **Tagline:** Every crash comes with a full flight recorder.
> **One-line pitch:** Prefix any command with `rev` and get instant, zero-setup time-travel debugging.

`rev` is a language-agnostic runtime execution recorder and replayer. It wraps any interpreter-based program execution, silently records every non-deterministic input and memory state delta during the run, and — on crash or explicit checkpoint — drops the developer into an interactive TUI where they can scrub backward and forward through the program's full execution history.

## Quick Start

```bash
# Run your program normally, but with the rev prefix
rev python main.py
```

If the program crashes, `rev` will automatically intercept the exit and open an interactive TUI showing:
- A step-by-step history of executed non-deterministic events (syscalls, randomness, network, etc.)
- A variable state inspector at each historical execution boundary
- The complete call stack corresponding to each step

## Project Structure

- **core**: Shared types, error handling, constants, and utilities.
- **interceptor**: Syscall interception layer (ptrace / ETW).
- **recorder**: Serialization of captured events to compressed trace files.
- **delta**: Memory dirty page tracking and state storage engine.
- **replay**: Reconstructs execution state by fast-forwarding headless interpreters.
- **tui**: Interactive terminal user interface.
- **cli**: Command-line orchestration entrypoint.

## Development

To build the workspace:
```bash
cargo build
```

To run all unit tests:
```bash
cargo test
```
