# `rev` — Time-Traveler Runtime: Complete Implementation Plan

> **Tagline:** Every crash comes with a full flight recorder.
> **One-line pitch:** Prefix any command with `rev` and get instant, zero-setup time-travel debugging.

---

## Table of Contents

1. [Project Overview](#1-project-overview)
2. [Core Philosophy & Principles](#2-core-philosophy--principles)
3. [System Architecture](#3-system-architecture)
4. [Repository Structure](#4-repository-structure)
5. [Module Specifications](#5-module-specifications)
   - 5.1 [Interceptor Layer](#51-interceptor-layer)
   - 5.2 [Trace Recorder](#52-trace-recorder)
   - 5.3 [State Delta Engine](#53-state-delta-engine)
   - 5.4 [Replay Engine](#54-replay-engine)
   - 5.5 [TUI Interface](#55-tui-interface)
   - 5.6 [CLI Entry Point](#56-cli-entry-point)
6. [Data Formats & Schemas](#6-data-formats--schemas)
7. [Language Runtime Support](#7-language-runtime-support)
8. [Implementation Phases](#8-implementation-phases)
9. [Testing Strategy](#9-testing-strategy)
10. [Error Handling Guidelines](#10-error-handling-guidelines)
11. [Performance Constraints](#11-performance-constraints)
12. [Developer Guidelines & DRY Rules](#12-developer-guidelines--dry-rules)
13. [Glossary](#13-glossary)
14. [Known Landmines & Required Fixes](#14-known-landmines--required-fixes)

---

## 1. Project Overview

### What `rev` Is

`rev` is a **language-agnostic runtime execution recorder and replayer**. It wraps any interpreter-based program execution, silently records every non-deterministic input and memory state delta during the run, and — on crash or explicit checkpoint — drops the developer into an interactive TUI where they can scrub backward and forward through the program's full execution history.

### What `rev` Is NOT

- Not a traditional debugger (no breakpoints, no step-through during normal execution)
- Not an AI assistant or log analyzer
- Not a replacement for the developer's existing shell, IDE, or language toolchain
- Not a rewrite of the program's runtime

### The Core User Contract

```
Normal run:   python main.py        →  program runs, crashes, you see a stack trace
With rev:     rev python main.py    →  program runs, crashes, TUI opens with full history
```

Zero configuration. Zero changed habits. One word prefix.

---

## 2. Core Philosophy & Principles

### 2.1 Zero Friction Above All

Every design decision must be evaluated against: *"Does this require the developer to change how they work?"* If yes, the design is wrong. Integration must feel like the tool was always there.

### 2.2 The Flight Recorder Model

`rev` operates like an airplane's black box. It runs silently in the background at all times. The developer never thinks about it until something goes wrong. When something goes wrong, everything is already recorded.

### 2.3 DRY (Don't Repeat Yourself)

- Every concept has **one canonical implementation**. No logic is duplicated across modules.
- All shared types, constants, and utilities live in a dedicated `core/` package imported everywhere else.
- If you find yourself writing similar logic in two places, that logic belongs in `core/`.

### 2.4 Separation of Concerns

Each module does exactly one thing:

| Module | Responsibility |
|---|---|
| `interceptor` | Captures syscalls from the running process |
| `recorder` | Serializes captured data into `.rev-trace` files |
| `delta` | Computes and stores memory state diffs |
| `replay` | Reconstructs any historical state from trace + deltas |
| `tui` | Renders the interactive interface for the developer |
| `cli` | Entry point, argument parsing, orchestration only |

No module reaches into another module's internals. All communication happens through defined interfaces.

### 2.5 Correctness Over Performance

During the recording phase, correctness (capturing every relevant event) is the priority. Performance optimizations are additive and never compromise the accuracy of the trace.

### 2.6 Explicit Over Implicit

All configuration is explicit. No magic defaults that differ by environment. All paths, formats, and behaviors are logged at startup in verbose mode.

---

## 3. System Architecture

### 3.1 High-Level Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        DEVELOPER ZONE                           │
│                                                                 │
│   $ rev python main.py                                          │
│         │                                                       │
│         ▼                                                       │
│   ┌─────────────┐                                               │
│   │  CLI Entry  │  Parses args, detects runtime, boots daemon  │
│   └──────┬──────┘                                               │
│          │                                                      │
└──────────┼──────────────────────────────────────────────────────┘
           │
┌──────────┼──────────────────────────────────────────────────────┐
│          ▼              RECORDING LAYER                         │
│   ┌──────────────┐    ┌──────────────┐    ┌──────────────────┐  │
│   │  Interceptor │───▶│    Recorder  │───▶│  Delta Engine    │  │
│   │  (ptrace /   │    │  (serializes │    │  (memory page    │  │
│   │   eBPF /     │    │   events to  │    │   dirty tracking │  │
│   │   dtrace)    │    │   .rev-trace)│    │   + Merkle DAG)  │  │
│   └──────────────┘    └──────────────┘    └──────────────────┘  │
│          │                                        │             │
└──────────┼────────────────────────────────────────┼─────────────┘
           │                                        │
┌──────────┼────────────────────────────────────────┼─────────────┐
│          ▼              REPLAY LAYER               ▼             │
│   ┌─────────────────────────────────────────────────────────┐   │
│   │                    Replay Engine                        │   │
│   │  Spawns headless interpreter → feeds recorded inputs    │   │
│   │  → fast-forwards to target state via delta tree         │   │
│   └───────────────────────────┬─────────────────────────────┘   │
│                               │                                  │
└───────────────────────────────┼──────────────────────────────────┘
                                │
┌───────────────────────────────┼──────────────────────────────────┐
│                               ▼         TUI LAYER                │
│   ┌─────────────────────────────────────────────────────────┐   │
│   │                   TUI Interface                         │   │
│   │   Timeline scrubber │ State viewer │ Variable inspector │   │
│   └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 Data Flow

```
Process runs
    │
    ├─── Syscall fires (read/write/time/random/network)
    │         │
    │         ├─── Interceptor captures raw bytes + timestamp + syscall ID
    │         │
    │         └─── Recorder appends event to .rev-trace (append-only)
    │
    ├─── Memory mutates (after each logical "step")
    │         │
    │         └─── Delta Engine records dirty pages → computes diff
    │                   │
    │                   └─── Stores delta node in Merkle DAG
    │
    └─── Process exits / crashes / hits checkpoint
              │
              └─── TUI launches
                        │
                        └─── User scrubs to step N
                                  │
                                  └─── Replay Engine reconstructs state N
```

### 3.3 OS-Level Strategy by Platform

| Platform | Primary Mechanism | Fallback |
|---|---|---|
| Linux | `ptrace` + eBPF probes | `strace` wrapper |
| macOS | `dtrace` | `lldb` scripting API |
| Windows | ETW (Event Tracing for Windows) | Debugger API |

> **Implementation note for agents:** Start with Linux/ptrace only. macOS and Windows support are Phase 2+. Abstract all platform-specific code behind a `Platform` interface in `interceptor/platform/` so they can be added without touching core logic.

---

## 4. Repository Structure

```
rev/
├── README.md
├── LICENSE
├── .github/
│   └── workflows/
│       ├── ci.yml               # Run tests on every PR
│       └── release.yml          # Build binaries on tag
│
├── docs/
│   ├── architecture.md          # This document (condensed)
│   ├── trace-format.md          # .rev-trace file format spec
│   ├── contributing.md          # Contributor guide
│   └── language-support.md      # Per-language integration notes
│
├── core/                        # Shared types, constants, utilities — imported by all modules
│   ├── types.rs                 # All shared structs and enums (Event, Delta, State, etc.)
│   ├── error.rs                 # Unified error types
│   ├── constants.rs             # Magic numbers, limits, defaults
│   └── utils.rs                 # Pure utility functions (hashing, compression, formatting)
│
├── interceptor/                 # Syscall capture layer
│   ├── mod.rs
│   ├── platform/
│   │   ├── linux.rs             # ptrace + eBPF implementation
│   │   ├── macos.rs             # dtrace implementation (Phase 2)
│   │   └── windows.rs           # ETW implementation (Phase 2)
│   ├── syscall_map.rs           # Syscall ID → semantic meaning mapping
│   └── filter.rs                # Which syscalls to capture vs ignore
│
├── recorder/                    # Trace serialization
│   ├── mod.rs
│   ├── writer.rs                # Append-only .rev-trace writer
│   ├── compressor.rs            # LZ4 compression wrapper
│   └── schema.rs                # Trace file schema versioning
│
├── delta/                       # Memory state diffing
│   ├── mod.rs
│   ├── page_tracker.rs          # Dirty page detection
│   ├── filter.rs                # Memory region whitelist (user heap vs interpreter internals)
│   ├── merkle.rs                # Merkle DAG construction + traversal
│   └── snapshot.rs              # Full snapshot at checkpoints
│
├── replay/                      # State reconstruction
│   ├── mod.rs
│   ├── engine.rs                # Core replay orchestration
│   ├── headless.rs              # Headless interpreter spawner
│   └── fast_forward.rs          # Delta tree traversal for O(log n) seek
│
├── tui/                         # Terminal user interface
│   ├── mod.rs
│   ├── app.rs                   # TUI application state machine
│   ├── timeline.rs              # Timeline scrubber component
│   ├── inspector.rs             # Variable/state inspector panel
│   ├── layout.rs                # Panel layout management
│   └── keybinds.rs              # All keyboard mappings in one place
│
├── cli/                         # Entry point and argument parsing
│   ├── main.rs
│   ├── args.rs                  # Argument definitions (clap)
│   ├── runtime_detect.rs        # Detect python/node/ruby from command
│   ├── orchestrator.rs          # Wires all modules together
│   └── shim/
│       └── alias.sh             # Shell hook: prompts "run rev !! ?" after a bare crash
│
├── tests/
│   ├── unit/                    # Unit tests per module (mirroring src structure)
│   ├── integration/             # End-to-end test programs
│   │   ├── fixtures/            # Sample programs that crash in known ways
│   │   └── scenarios/           # Test scenarios with expected outcomes
│   └── benchmarks/              # Performance regression tests
│
└── Cargo.toml                   # Workspace root
```

---

## 5. Module Specifications

---

### 5.1 Interceptor Layer

**Location:** `interceptor/`
**Language:** Rust (with unsafe blocks for syscall-level work)
**Purpose:** Attach to the child process and capture every non-deterministic input before it reaches the program.

#### What to Capture

Non-determinism in a running process comes from exactly these syscall categories:

| Category | Syscalls (Linux) | Why It Matters |
|---|---|---|
| Time | `clock_gettime`, `gettimeofday` | `datetime.now()`, timestamps |
| Randomness | `getrandom`, `/dev/urandom` reads | `random.random()`, UUID generation |
| Network I/O | `read`/`recv` on sockets | HTTP responses, DB query results |
| File I/O | `read` on file descriptors | Config files, user input files |
| Environment | `getenv` (via memory) | Env var reads |
| Process | `getpid`, `fork` return values | Process identity |

#### What NOT to Capture

- Write syscalls (output, not input — doesn't affect determinism)
- Memory allocation syscalls (`mmap`, `brk`) — tracked separately by the delta engine
- CPU instructions — too granular, captured implicitly by state deltas

#### Interface Contract

```rust
// interceptor/mod.rs

pub trait Interceptor: Send {
    /// Attach to a running process by PID
    fn attach(&mut self, pid: u32) -> Result<(), RevError>;

    /// Block until the next capturable event occurs, then return it
    fn next_event(&mut self) -> Result<SyscallEvent, RevError>;

    /// Detach cleanly without killing the process
    fn detach(&mut self) -> Result<(), RevError>;
}

// The single event type returned by every interceptor implementation
pub struct SyscallEvent {
    pub id: u64,                  // monotonically increasing sequence number
    pub timestamp_ns: u64,        // nanoseconds since process start
    pub syscall: SyscallKind,     // enum of captured syscall categories
    pub return_bytes: Vec<u8>,    // the exact bytes returned to the process
    pub fd: Option<i32>,          // file descriptor if applicable
}

pub enum SyscallKind {
    TimeRead,
    RandomRead,
    NetworkRead { socket_addr: Option<String> },
    FileRead { path: Option<String> },
    EnvRead { key: String },
    ProcessId,
}
```

#### Implementation Notes for Agents

- The `Interceptor` trait must be implemented separately per platform in `interceptor/platform/`. The `cli/runtime_detect.rs` selects the correct implementation at startup.
- On Linux, use `ptrace(PTRACE_SYSCALL, ...)` to stop on every syscall entry/exit. On exit, read the return value from registers. Capture the return bytes before `PTRACE_CONT`.
- **Critical:** The child process is paused while the interceptor reads the syscall return value. Resume it immediately — minimize latency here. Any processing beyond reading registers must be deferred to the Recorder.
- `syscall_map.rs` contains a static mapping from raw syscall numbers to `SyscallKind`. This mapping is the single source of truth — never hardcode syscall numbers elsewhere.
- `filter.rs` decides which syscalls to intercept. Default filter: capture the categories in the table above, ignore everything else. This is configurable via `RevConfig`.

---

### 5.2 Trace Recorder

**Location:** `recorder/`
**Language:** Rust
**Purpose:** Take `SyscallEvent` objects from the interceptor and write them to a `.rev-trace` file efficiently and durably.

#### Design Goals

- **Append-only:** Never modify existing records. Corruption resistance.
- **Compressed:** LZ4 compression per chunk. Network and file I/O payloads are highly compressible.
- **Streamable:** Can be read back event-by-event without loading the whole file.
- **Versioned:** Schema version in the file header so old traces can always be replayed.

#### Trace File Structure

```
[HEADER: 64 bytes]
  magic:    8 bytes  →  "REVTRACE"
  version:  2 bytes  →  schema version (u16)
  runtime:  16 bytes →  null-padded runtime name ("python3.11\0...")
  pid:      4 bytes  →  original process PID
  start_ts: 8 bytes  →  process start unix timestamp (ns)
  reserved: 26 bytes →  zeroed, for future use

[CHUNKS: variable]
  Each chunk:
    chunk_len:     4 bytes  → compressed chunk size in bytes (u32)
    event_count:   2 bytes  → number of events in this chunk (u16)
    checksum:      4 bytes  → CRC32 of compressed data
    data:          N bytes  → LZ4-compressed sequence of SyscallEvent records
```

#### Interface Contract

```rust
// recorder/mod.rs

pub struct Recorder {
    writer: BufWriter<File>,
    chunk_buffer: Vec<SyscallEvent>,
    chunk_size: usize,       // flush every N events (default: from constants.rs)
    bytes_written: u64,
}

impl Recorder {
    pub fn new(trace_path: &Path, config: &RecorderConfig) -> Result<Self, RevError>;

    /// Buffer an event. Flushes chunk to disk when chunk_size is reached.
    pub fn record(&mut self, event: SyscallEvent) -> Result<(), RevError>;

    /// Force-flush remaining buffered events and write EOF marker.
    pub fn finalize(&mut self) -> Result<TraceStats, RevError>;
}

pub struct TraceStats {
    pub total_events: u64,
    pub bytes_written: u64,
    pub compression_ratio: f32,
    pub duration_ms: u64,
}
```

#### Implementation Notes for Agents

- `constants.rs` defines `DEFAULT_CHUNK_SIZE = 256` (events per chunk). Do not hardcode this anywhere else.
- `compressor.rs` is a thin wrapper around the `lz4_flex` crate. It exposes `compress(bytes) -> Vec<u8>` and `decompress(bytes, original_len) -> Vec<u8>`. All compression calls go through this wrapper — never call `lz4_flex` directly from other modules.
- `schema.rs` defines the binary encoding of `SyscallEvent` using `bincode`. Schema version is bumped whenever the encoding changes. The replay engine checks schema version before reading any trace.
- The trace file is created in `~/.rev/traces/<pid>_<timestamp>.rev-trace` by default. The path is configurable.
- On process crash, **do not call `finalize()` inside a signal handler**. See Landmine 3 in Section 14 for the correct atomic flag pattern.

---

### 5.3 State Delta Engine

**Location:** `delta/`
**Language:** Rust
**Purpose:** Track what changed in the process's memory between logical steps, building a queryable history of states.

#### The Problem

Full memory snapshots of a Python or Node process can be hundreds of megabytes. Snapshotting every instruction is impossible. The solution is **dirty page tracking**: only record memory pages that actually changed since the last snapshot.

#### Logical Step Definition

A "step" in `rev` is not a CPU instruction — it is a **semantic boundary**:
- One syscall event = one step boundary
- An explicit `rev.checkpoint()` call in user code = one step boundary
- Process exit/crash = final step

This keeps the step count human-scale (hundreds to thousands for a typical program run, not millions).

#### Merkle DAG Structure

```
State 0 (full snapshot at process start)
    │
    ├── Delta 1 (pages changed after step 1)
    │       │
    │       ├── Delta 2 (pages changed after step 2)
    │       │
    │       └── Delta 2b (branching — only for replay, not recording)
    │
    └── ...

Each delta node:
    hash:       32 bytes  → Blake3 hash of this node's content
    parent:     32 bytes  → hash of parent node
    step_id:    8 bytes   → matches SyscallEvent.id at this boundary
    pages:      [PageDiff] → list of changed pages
```

#### Interface Contract

```rust
// delta/mod.rs

pub struct DeltaEngine {
    merkle: MerkleDAG,
    page_tracker: PageTracker,
    current_step: u64,
}

impl DeltaEngine {
    pub fn new(pid: u32) -> Result<Self, RevError>;

    /// Called at each step boundary. Computes dirty pages, creates delta node.
    pub fn commit_step(&mut self, step_id: u64) -> Result<DeltaHash, RevError>;

    /// Retrieve the full memory state at any historical step.
    pub fn state_at(&self, step_id: u64) -> Result<MemoryState, RevError>;

    /// Returns ordered list of all step IDs recorded so far.
    pub fn steps(&self) -> &[u64];
}

pub struct PageDiff {
    pub address: u64,      // page-aligned memory address
    pub before: Vec<u8>,   // page content before this step (for reverse replay)
    pub after: Vec<u8>,    // page content after this step
}
```

#### Implementation Notes for Agents

- `page_tracker.rs` uses Linux's `userfaultfd` or `/proc/<pid>/pagemap` to detect which pages were written since the last checkpoint. Read the pagemap, compare dirty bits, reset them, record changed pages. **Before tracking, parse `/proc/<pid>/maps` to build a whitelist of user-heap memory regions.** Only track pages that fall within those regions. Interpreter-internal segments (the CPython or V8 binary's own text/BSS/data segments) must be excluded — managed runtimes constantly dirty these pages through garbage collection and reference counting, which would otherwise flood the delta log with noise unrelated to the user's program state. See Landmine 1 in Section 14.
- `merkle.rs` stores the DAG in a local SQLite database at `~/.rev/deltas/<pid>.db`. Each row is a delta node. Blake3 is used for hashing (`blake3` crate).
- Page size is always `4096` bytes on x86_64. Define this as `PAGE_SIZE` in `constants.rs`.
- `snapshot.rs` handles full memory snapshots at step 0 and at every `SNAPSHOT_INTERVAL` steps (default: `100`, defined in `constants.rs`). Full snapshots enable O(log n) seek during replay — you never need to replay from step 0, only from the nearest snapshot.

---

### 5.4 Replay Engine

**Location:** `replay/`
**Language:** Rust
**Purpose:** Given a target step N, reconstruct the exact memory state and variable values of the process at that point.

#### Replay Algorithm

```
1. Load .rev-trace file for this process run
2. Find nearest full snapshot at or before step N  →  snapshot_step S
3. Spawn headless interpreter (same binary as original run)
4. Feed interpreter all recorded syscall return values from step S → step N
   (instead of making real syscalls, return the recorded bytes)
5. Apply memory delta nodes from S → N on top of base snapshot
6. Extract variable state from interpreter's memory using runtime-specific introspection
7. Return MemoryState to TUI
```

#### Interface Contract

```rust
// replay/mod.rs

pub struct ReplayEngine {
    trace: TraceReader,
    delta: DeltaEngine,
    runtime: Box<dyn RuntimeIntrospector>,
}

impl ReplayEngine {
    pub fn new(trace_path: &Path, runtime: Box<dyn RuntimeIntrospector>) -> Result<Self, RevError>;

    /// Reconstruct state at given step. Returns variable values and call stack.
    pub fn state_at(&mut self, step: u64) -> Result<ProgramState, RevError>;

    /// Total number of steps recorded.
    pub fn step_count(&self) -> u64;

    /// Human-readable summary of what happened at a given step.
    pub fn event_summary(&self, step: u64) -> Option<String>;
}

pub struct ProgramState {
    pub step: u64,
    pub timestamp_ns: u64,
    pub variables: Vec<Variable>,       // local variables in scope at this step
    pub call_stack: Vec<StackFrame>,    // call stack
    pub last_event: SyscallEvent,       // what triggered this step boundary
}

pub struct Variable {
    pub name: String,
    pub type_name: String,
    pub value: serde_json::Value,       // JSON representation of value
    pub is_changed: bool,               // did this variable change at this step?
}
```

#### Interface for Runtime Introspection

Each supported language implements this trait:

```rust
// replay/headless.rs

pub trait RuntimeIntrospector: Send {
    /// Name of the runtime this handles (e.g., "python3")
    fn runtime_name(&self) -> &str;

    /// Spawn the interpreter in replay mode with syscall interception
    fn spawn_headless(&self, program: &Path, args: &[String]) -> Result<HeadlessProcess, RevError>;

    /// Extract variable values from the running headless process at current state
    fn extract_variables(&self, proc: &HeadlessProcess) -> Result<Vec<Variable>, RevError>;

    /// Extract the current call stack
    fn extract_call_stack(&self, proc: &HeadlessProcess) -> Result<Vec<StackFrame>, RevError>;
}
```

#### Implementation Notes for Agents

- `engine.rs` owns the orchestration. It never directly touches ptrace or memory — it calls `delta.state_at()` and `headless.extract_variables()`.
- `fast_forward.rs` implements the seek algorithm: given a target step, find the nearest snapshot, apply deltas in order. This must never replay from step 0 if a closer snapshot exists.
- The headless interpreter is spawned with all network access blocked (via seccomp on Linux) to guarantee it only uses recorded inputs. This prevents the replay from making real network calls.
- `state_at()` must complete in under 500ms for a smooth TUI experience. Profile and optimize the delta application path if this is not met.

---

### 5.5 TUI Interface

**Location:** `tui/`
**Language:** Rust (`ratatui` crate)
**Purpose:** Render the interactive time-travel interface inside the developer's existing terminal.

#### Layout

```
┌─────────────────────────────────────────────────────────────────┐
│  rev — flight recorder  │  main.py  │  step 47 / 203           │
├──────────────────────────────────────┬──────────────────────────┤
│                                      │  VARIABLES               │
│  CALL STACK                          │  ─────────────           │
│  ─────────────                       │  user_id    = 42         │
│  ▶ process_payment (payment.py:88)   │  order      = {...}  ●   │
│    handle_request (server.py:134)    │  response   = None       │
│    main (main.py:12)                 │  retries    = 3      ●   │
│                                      │                          │
├──────────────────────────────────────┴──────────────────────────┤
│  LAST EVENT                                                      │
│  NetworkRead ← api.stripe.com:443  (234 bytes)                  │
│  Returned: {"error": "card_declined", "code": "insufficient_…"} │
├─────────────────────────────────────────────────────────────────┤
│  TIMELINE                                                        │
│  ●──●──●──●──●──◆──●──●──●──●──●──◆──●──●──●──✕               │
│  0  10 20 30 40 50 60 70 80 90 100 110 120 130 140 (CRASH)     │
│                              ▲                                   │
│                           step 47                               │
├─────────────────────────────────────────────────────────────────┤
│  ← prev   → next   ⇐ jump back 10   ⇒ jump fwd 10   q quit    │
└─────────────────────────────────────────────────────────────────┘

● = variable changed at this step
◆ = checkpoint
✕ = crash point
```

#### Keybindings (`tui/keybinds.rs` — single source of truth)

| Key | Action |
|---|---|
| `←` / `h` | Previous step |
| `→` / `l` | Next step |
| `Shift+←` / `H` | Jump back 10 steps |
| `Shift+→` / `L` | Jump forward 10 steps |
| `g` | Go to step 0 |
| `G` | Go to last step (crash) |
| `/` | Search events by keyword |
| `v` | Toggle variable inspector expand |
| `e` | Show full event payload |
| `s` | Export current state to JSON |
| `q` / `Esc` | Quit TUI |

#### Interface Contract

```rust
// tui/app.rs

pub struct TuiApp {
    replay: ReplayEngine,
    current_step: u64,
    total_steps: u64,
    current_state: Option<ProgramState>,
    layout: Layout,
    mode: AppMode,
}

pub enum AppMode {
    Normal,
    Search(String),        // user is typing a search query
    EventExpanded,         // full event payload view
    ExportConfirm,
}

impl TuiApp {
    pub fn new(replay: ReplayEngine) -> Self;

    /// Main loop: render → handle input → replay → re-render
    pub fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<(), RevError>;

    // Private: called on any step change
    fn seek_to(&mut self, step: u64);
    fn render(&self, frame: &mut Frame);
    fn handle_key(&mut self, key: KeyEvent) -> AppAction;
}
```

#### Implementation Notes for Agents

- `layout.rs` computes panel sizes from terminal dimensions. All layout math is here — no hardcoded dimensions in render functions.
- `timeline.rs` renders the step dots. It samples steps to fit the terminal width — if there are 1000 steps and 80 columns, it renders every ~12th step as a dot. The current step is always shown with a caret underneath.
- `inspector.rs` renders the variable panel. Variables changed at the current step are marked with `●` (defined as `CHANGED_MARKER` in `constants.rs`).
- Changed variables are highlighted using the terminal's default "yellow" color — never hardcode hex colors. Use `ratatui`'s `Color::Yellow` so it respects the user's terminal theme.
- The TUI must render in under 16ms per frame (60fps target). `seek_to()` is async; the TUI shows a loading indicator if replay takes more than 100ms.

---

### 5.6 CLI Entry Point

**Location:** `cli/`
**Language:** Rust (`clap` crate)
**Purpose:** Parse arguments, detect the runtime, wire all modules together, and launch the child process.

#### Command Interface

```
USAGE:
    rev [OPTIONS] <RUNTIME> [ARGS...]

ARGS:
    <RUNTIME>    The interpreter and program to run (e.g., "python main.py")
    [ARGS...]    Arguments passed through to the program

OPTIONS:
    -o, --output <PATH>       Where to save the .rev-trace file [default: ~/.rev/traces/]
    -s, --step-size <N>       Snapshot interval in steps [default: 100]
    -v, --verbose             Print recording stats during execution
    --no-tui                  Record only, don't open TUI on crash
    --replay <TRACE>          Open TUI directly on an existing .rev-trace file
    --export <TRACE> <STEP>   Export state at step N to stdout as JSON (no TUI)
    -h, --help                Show help
    -V, --version             Show version

EXAMPLES:
    rev python main.py
    rev node server.js --port 3000
    rev ruby app.rb
    rev --replay ~/.rev/traces/12345_1720000000.rev-trace
    rev --export trace.rev-trace 47
```

#### Runtime Detection (`cli/runtime_detect.rs`)

```rust
// Detects which RuntimeIntrospector to use from the command
pub fn detect_runtime(command: &[String]) -> Result<Box<dyn RuntimeIntrospector>, RevError> {
    let binary = command.first().ok_or(RevError::NoCommand)?;
    match binary.as_str() {
        "python" | "python3" | s if s.starts_with("python") => Ok(Box::new(PythonIntrospector::new())),
        "node" | "nodejs"                                    => Ok(Box::new(NodeIntrospector::new())),
        "ruby"                                               => Ok(Box::new(RubyIntrospector::new())),
        other => Err(RevError::UnsupportedRuntime(other.to_string())),
    }
}
```

#### Orchestration Flow (`cli/orchestrator.rs`)

```
1. Parse args
2. Detect runtime → get RuntimeIntrospector
3. Create Recorder (opens .rev-trace file)
4. Create DeltaEngine (attaches to process memory)
5. Fork child process
6. Attach Interceptor to child PID
7. Loop:
   a. interceptor.next_event() → SyscallEvent
   b. recorder.record(event)
   c. delta.commit_step(event.id)
   d. Resume child process
8. On child exit/crash:
   a. recorder.finalize()
   b. If exit was crash OR --tui flag:
      → create ReplayEngine
      → launch TuiApp
```

#### Shell Crash Hook (`cli/shim/alias.sh`)

This file is appended to the user's `~/.bashrc` or `~/.zshrc` during `rev` installation. It wraps the shell's `command_not_found` / exit-code trap to silently detect when a bare (non-`rev`) command exits with a non-zero code and prompt the user once:

```bash
# Installed by rev — do not edit manually
__rev_trap() {
  local exit_code=$?
  local last_cmd=$(fc -ln -1 | xargs)
  if [[ $exit_code -ne 0 && "$last_cmd" != rev* ]]; then
    printf "\n\033[33mrev:\033[0m command failed (exit %d). Run \033[1mrev !!\033[0m to time-travel? [y/N] " "$exit_code"
    read -r reply
    if [[ "$reply" =~ ^[Yy]$ ]]; then
      eval "rev $last_cmd"
    fi
  fi
}
trap '__rev_trap' DEBUG
```

**Rules for the shim:**
- It must never run automatically — always prompt first. The developer stays in control.
- It fires only on non-zero exit codes from non-`rev` commands. It never double-wraps.
- The prompt is a single line, dismissable with Enter. No blocking, no verbose output.
- Installation is idempotent: the installer checks for the `# Installed by rev` marker before appending.
- Uninstallation (`rev --uninstall-shim`) removes the block cleanly.
- The shim is entirely optional. Users who don't want it can skip it during install with `--no-shim`.

---

## 6. Data Formats & Schemas

### 6.1 `.rev-trace` File (Binary)

Defined completely in `recorder/schema.rs`. See Section 5.2 for structure. Schema version must be incremented on any breaking change.

### 6.2 State Export Format (JSON)

When a user presses `s` in the TUI or uses `--export`, the state is written as:

```json
{
  "rev_version": "0.1.0",
  "trace_file": "/home/user/.rev/traces/12345_1720000000.rev-trace",
  "runtime": "python3.11",
  "step": 47,
  "total_steps": 203,
  "timestamp_ns": 1720000001234567890,
  "last_event": {
    "kind": "NetworkRead",
    "socket_addr": "api.stripe.com:443",
    "bytes_read": 234,
    "payload_preview": "{\"error\": \"card_declined\"..."
  },
  "call_stack": [
    { "function": "process_payment", "file": "payment.py", "line": 88 },
    { "function": "handle_request",  "file": "server.py",  "line": 134 },
    { "function": "main",            "file": "main.py",    "line": 12  }
  ],
  "variables": [
    { "name": "user_id",  "type": "int",    "value": 42,     "changed": false },
    { "name": "order",    "type": "dict",   "value": {...},  "changed": true  },
    { "name": "response", "type": "None",   "value": null,   "changed": false },
    { "name": "retries",  "type": "int",    "value": 3,      "changed": true  }
  ]
}
```

### 6.3 Configuration File (`~/.rev/config.toml`)

```toml
[defaults]
trace_dir = "~/.rev/traces"
snapshot_interval = 100      # steps between full memory snapshots
chunk_size = 256             # events per compressed chunk in trace file
max_trace_age_days = 7       # auto-delete traces older than this

[tui]
jump_size = 10               # steps to jump with Shift+arrow

[runtime.python]
executable = "python3"       # override default python binary

[runtime.node]
executable = "node"
```

---

## 7. Language Runtime Support

### 7.1 Phase 1 — Python

**Introspection method:** `libpython.so` symbol table via `process_vm_readv`

When the headless Python process is paused for replay, the introspector reads variable names, types, and values from the CPython interpreter's memory. **Do not use hardcoded `PyFrameObject` struct field offsets.** The internal memory layout of `PyFrameObject` changed significantly between Python 3.10, 3.11, and 3.12 (3.11 introduced a completely overhauled frame representation and internal bytecode caching). Hardcoded offsets will silently produce garbage values or segfault on any version mismatch.

**Correct approach:** At startup, the `PythonIntrospector` reads the exported symbol table from the `libpython.so` loaded in the target process (visible via `/proc/<pid>/maps` + `dlopen`/`dlsym` on the mapped library path). This gives the correct, version-specific offsets for the running interpreter's actual struct layout. Use `process_vm_readv` to read memory cross-process without ptrace overhead. See Landmine 2 in Section 14.

**Key challenge:** Python's garbage collector moves objects between GC cycles. Always validate pointer integrity using the `ob_refcnt` field of `PyObject` before dereferencing. A zero or negative refcount means the object has been collected and the pointer is stale.

### 7.2 Phase 2 — Node.js

**Introspection method:** V8 inspector protocol

Node has a built-in inspector (`--inspect` flag). The headless Node process is started with the inspector enabled on a local port. The introspector connects to it and uses the Chrome DevTools Protocol to extract variable state. This is significantly easier than CPython because V8 exposes a clean API.

### 7.3 Phase 3 — Ruby

**Introspection method:** `ObjectSpace` API via pipe

A small shim (`rev_shim.rb`) is injected into the Ruby process at load time via `-r rev_shim`. The shim registers a hook that writes variable state to a local pipe when signaled by the introspector. This is the least invasive approach for MRI Ruby.

### Adding a New Runtime

1. Create `replay/runtimes/<name>.rs` implementing `RuntimeIntrospector`
2. Add detection case to `cli/runtime_detect.rs`
3. Add configuration section to `config.toml` schema in `core/constants.rs`
4. Add integration test fixture in `tests/integration/fixtures/<name>/`
5. Document in `docs/language-support.md`

---

## 8. Implementation Phases

### Phase 0 — Foundation

**Goal:** Compile-able project skeleton with all interfaces defined but not implemented.

- [ ] Initialize Rust workspace with all crates (`cargo new --lib` for each module)
- [ ] Define all types in `core/types.rs` — `SyscallEvent`, `ProgramState`, `Variable`, `StackFrame`, `Delta`, `RevError`
- [ ] Define all error variants in `core/error.rs` with descriptive messages
- [ ] Define all constants in `core/constants.rs`
- [ ] Stub all `trait` interfaces with `todo!()` implementations
- [ ] Set up CI (GitHub Actions): `cargo build`, `cargo test`, `cargo clippy -- -D warnings`
- [ ] Write `README.md` with install instructions and quick-start example

**Exit criteria:** `cargo build` succeeds. All tests fail with `not yet implemented` (not compile errors).

---

### Phase 1 — Record

**Goal:** `rev python main.py` records a trace file. No TUI yet.

- [ ] Implement `interceptor/platform/linux.rs` using `ptrace`
- [ ] Implement `syscall_map.rs` for the 6 captured syscall categories
- [ ] Implement `recorder/writer.rs` with append-only writes
- [ ] Implement `recorder/compressor.rs` using `lz4_flex`
- [ ] Implement `delta/page_tracker.rs` using `/proc/<pid>/pagemap`
- [ ] Implement `delta/merkle.rs` with SQLite backend
- [ ] Implement `cli/args.rs` with `clap`
- [ ] Implement `cli/runtime_detect.rs` for python/node/ruby
- [ ] Implement `cli/orchestrator.rs` — full recording loop
- [ ] Integration test: run a crashing Python script, verify `.rev-trace` file is created with correct event count

**Exit criteria:** `rev python tests/fixtures/crash.py` produces a valid `.rev-trace` file. `--verbose` output shows event counts and compression ratio.

---

### Phase 2 — Replay

**Goal:** Given a `.rev-trace`, reconstruct program state at any step.

- [ ] Implement `replay/engine.rs` — core orchestration
- [ ] Implement `replay/headless.rs` — headless Python spawner with syscall interception
- [ ] Implement `replay/fast_forward.rs` — snapshot-based O(log n) seek
- [ ] Implement `replay/runtimes/python.rs` — CPython frame introspection
- [ ] CLI: `rev --export trace.rev-trace 47` prints JSON state to stdout
- [ ] Integration test: run `crash.py`, export state at step N, verify variable values match expected values from the fixture's known state

**Exit criteria:** `rev --export` produces correct variable values for 3+ different Python test programs.

---

### Phase 3 — TUI

**Goal:** Full interactive time-travel interface.

- [ ] Implement `tui/layout.rs` — responsive panel sizing
- [ ] Implement `tui/timeline.rs` — step dots with sampling
- [ ] Implement `tui/inspector.rs` — variable panel with change indicators
- [ ] Implement `tui/app.rs` — main loop, input handling, state machine
- [ ] Implement `tui/keybinds.rs` — all keybindings as constants
- [ ] Connect TUI to replay engine: step changes trigger `replay.state_at()`
- [ ] Loading indicator for replay operations > 100ms
- [ ] Integration test: launch TUI against a known trace, simulate keystrokes, assert rendered output

**Exit criteria:** `rev python crash.py` opens TUI on crash. All keybindings work. Timeline renders correctly for traces with 10, 100, and 1000 steps.

---

### Phase 4 — Polish & Distribution

**Goal:** Installable, documented, and performant.

- [ ] Performance profiling: `replay.state_at()` must complete in < 500ms for traces with 10k steps
- [ ] Node.js runtime support (Phase 2 runtime)
- [ ] `brew install rev` formula (macOS) and `apt` package (Linux)
- [ ] Shell completion scripts for bash/zsh/fish
- [ ] `docs/` fully written
- [ ] Record a 2-minute demo video showing a real bug being debugged with `rev`

---

## 9. Testing Strategy

### 9.1 Unit Tests

Every public function in every module has a unit test. Tests live in `tests/unit/` mirroring the source structure.

**Naming convention:** `test_<function_name>_<scenario>`

```rust
// Example
#[test]
fn test_detect_runtime_python() { ... }

#[test]
fn test_detect_runtime_unknown_returns_error() { ... }

#[test]
fn test_recorder_chunk_flushes_at_chunk_size() { ... }
```

### 9.2 Integration Test Fixtures

`tests/integration/fixtures/` contains small programs designed to crash in specific, known ways:

| Fixture | Language | What it tests |
|---|---|---|
| `division_by_zero.py` | Python | Exception with local variables |
| `infinite_loop.py` | Python | Long-running trace with many steps |
| `network_crash.py` | Python | Crash after HTTP request |
| `env_missing.py` | Python | Crash reading missing env var |
| `multi_function.py` | Python | Deep call stack |

Each fixture has a companion `expected_state_step_N.json` file. The integration test runs the fixture with `rev`, exports state at step N, and asserts equality.

### 9.3 Performance Benchmarks

`tests/benchmarks/` uses Criterion.rs to benchmark:

- `recorder.record()` throughput (target: > 100k events/sec)
- `delta.commit_step()` latency (target: < 1ms per step)
- `replay.state_at()` latency (target: < 500ms for 10k-step trace)
- TUI render frame time (target: < 16ms)

Benchmarks run in CI on every PR. A regression of > 10% on any benchmark fails the CI.

---

## 10. Error Handling Guidelines

### 10.1 Error Enum

All errors are variants of `RevError` defined in `core/error.rs`. Never use `unwrap()` or `expect()` outside of test code.

```rust
// core/error.rs

#[derive(Debug, thiserror::Error)]
pub enum RevError {
    #[error("No command provided. Usage: rev <runtime> [args...]")]
    NoCommand,

    #[error("Unsupported runtime: '{0}'. Supported: python, node, ruby")]
    UnsupportedRuntime(String),

    #[error("Failed to attach to process {pid}: {reason}")]
    AttachFailed { pid: u32, reason: String },

    #[error("Trace file corrupted at offset {offset}: {reason}")]
    TraceCorrupted { offset: u64, reason: String },

    #[error("Trace schema version {found} is newer than supported {supported}")]
    SchemaMismatch { found: u16, supported: u16 },

    #[error("Replay failed at step {step}: {reason}")]
    ReplayFailed { step: u64, reason: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
}
```

### 10.2 Rules

- Use `?` for propagation everywhere. Never swallow errors silently.
- User-facing errors (printed to stderr before exit) must be human-readable. Use the `#[error("...")]` message directly.
- Internal errors (logged in verbose mode) include full context: file, function, step ID.
- On crash during recording (the recorder itself crashes, not the target process), write a partial trace with an error marker at the end. Always give the user *something*.

---

## 11. Performance Constraints

| Operation | Target | Hard Limit |
|---|---|---|
| `interceptor.next_event()` latency | < 0.1ms | < 1ms |
| `recorder.record()` throughput | > 100k/sec | — |
| `delta.commit_step()` latency | < 1ms | < 5ms |
| Overall overhead on target process | < 5% slowdown | < 20% |
| `replay.state_at()` for any step | < 500ms | < 2s |
| TUI frame render time | < 16ms | < 50ms |

**Overhead measurement:** Run `python -c "import time; [time.sleep(0) for _ in range(100000)]"` with and without `rev`. Compute wall-clock ratio.

---

## 12. Developer Guidelines & DRY Rules

### 12.1 The Single Source of Truth Rules

| Thing | Where it lives | Never define it elsewhere |
|---|---|---|
| All shared types | `core/types.rs` | — |
| All error variants | `core/error.rs` | — |
| All magic numbers | `core/constants.rs` | — |
| All keybindings | `tui/keybinds.rs` | — |
| Syscall → Kind mapping | `interceptor/syscall_map.rs` | — |
| Runtime detection logic | `cli/runtime_detect.rs` | — |
| Compression calls | `recorder/compressor.rs` | — |

### 12.2 Code Review Checklist

Before any PR is merged, verify:

- [ ] No `unwrap()` or `expect()` outside `tests/`
- [ ] No magic numbers outside `constants.rs`
- [ ] No duplicated logic across modules
- [ ] Every new public function has a unit test
- [ ] Every new module has a doc comment explaining its single responsibility
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo fmt` has been run
- [ ] Performance benchmarks have not regressed > 10%

### 12.3 Commit Message Format

```
<module>: <what changed> (<why>)

Examples:
  interceptor: capture getrandom syscall (needed for uuid tracking)
  tui: fix timeline sampling for < 10 steps (was panicking on short traces)
  core: add SchemaMismatch error variant (needed for schema versioning)
```

### 12.4 Adding a New Syscall Category

1. Add variant to `SyscallKind` enum in `core/types.rs`
2. Add mapping entry in `interceptor/syscall_map.rs`
3. Add filter rule in `interceptor/filter.rs`
4. Add JSON serialization in `core/types.rs`
5. Add unit test for the new variant
6. Update `docs/architecture.md` syscall table

**Do not** add syscall logic anywhere else. The mapping is in `syscall_map.rs` and the rest follows automatically.

---

## 14. Known Landmines & Required Fixes

These are confirmed low-level engineering pitfalls that **will** be encountered during implementation. Each has a mandated fix. Do not attempt an alternative approach without strong justification documented in an ADR (Architecture Decision Record).

---

### Landmine 1 — Interpreter GC Noise in Dirty Page Tracking

**Affects:** `delta/page_tracker.rs`, Phase 1

**The trap:** Managed runtimes like CPython and V8 run garbage collection and update object reference counts continuously in the background. Even when the user's script variables are completely idle, the interpreter will dirty hundreds of memory pages per second through internal bookkeeping. Using raw `/proc/<pid>/pagemap` dirty-bit tracking without filtering will cause the delta engine to record thousands of irrelevant page diffs per step, bloating trace files and producing meaningless state history.

**The fix:** Before beginning page tracking, parse `/proc/<pid>/maps` to classify every mapped memory region. Build a whitelist containing only regions tagged as user heap (typically `[heap]` and anonymous `rw-p` mappings created by the user program). Exclude all regions that correspond to the interpreter binary itself, its shared libraries (`libpython.so`, `libv8.so`, `libc.so`, etc.), and their BSS/data segments. Implement this classification in `delta/filter.rs`. The `PageTracker` must consult `filter.rs` before recording any page diff.

```
/proc/<pid>/maps entries to EXCLUDE:
  /usr/lib/python3.x/...      ← interpreter binary segments
  /usr/lib/x86_64-linux-gnu/libc.so.*
  /usr/lib/x86_64-linux-gnu/libpython*.so.*
  [vvar], [vdso], [vsyscall]  ← kernel virtual pages

/proc/<pid>/maps entries to INCLUDE:
  [heap]                      ← user heap
  anonymous rw-p with no path ← dynamically allocated user memory
```

---

### Landmine 2 — CPython Frame Layout Version Mismatch

**Affects:** `replay/runtimes/python.rs`, Phase 2

**The trap:** Reading `PyFrameObject` fields using hardcoded C struct byte offsets will produce silently wrong variable values or segfaults whenever the Python minor version differs from what the offsets were written for. Python 3.11 completely overhauled the internal frame representation (introducing `_PyInterpreterFrame` as a separate concept) and added internal bytecode caching. Offsets valid for 3.10 are wrong for 3.11, and 3.11 offsets are wrong for 3.12.

**The fix:** At startup, `PythonIntrospector::new()` must:

1. Identify the exact `libpython.so` path from `/proc/<pid>/maps`
2. Read that library's ELF symbol table to locate the addresses of key exported symbols (`PyInterpreterState_Head`, `_PyRuntime`, etc.)
3. Use those live symbol addresses — not hardcoded offsets — to navigate the frame structure

Use `process_vm_readv` for cross-process memory reads (faster than ptrace, no process pause required for reads). Never use hardcoded struct offsets. If symbol resolution fails for an unsupported Python version, `PythonIntrospector` must return `RevError::UnsupportedRuntime` with the exact version string rather than proceeding with wrong data.

---

### Landmine 3 — Signal Handler File I/O Deadlock

**Affects:** `recorder/writer.rs`, `cli/orchestrator.rs`, Phase 1

**The trap:** The original design called for invoking `recorder.finalize()` inside the SIGCHLD/SIGSEGV signal handler that fires when the child process crashes. This is incorrect. POSIX signal handlers have severe restrictions: calling `malloc`, acquiring a `Mutex`, or performing buffered file I/O (`BufWriter` flush) inside a signal handler risks immediate deadlock if the signal interrupts an already-locked allocator or I/O operation. The program will hang silently with the trace file incomplete.

**The fix:** The signal handler must do exactly one thing — set an `AtomicBool` flag:

```rust
// In cli/orchestrator.rs
static CHILD_CRASHED: AtomicBool = AtomicBool::new(false);

extern "C" fn handle_sigchld(_: libc::c_int) {
    // Only async-signal-safe operations allowed here
    CHILD_CRASHED.store(true, Ordering::SeqCst);
}
```

The main recording loop checks this flag on every iteration:

```rust
loop {
    if CHILD_CRASHED.load(Ordering::SeqCst) {
        break; // exit loop cleanly
    }
    let event = interceptor.next_event()?;
    recorder.record(event)?;
    delta.commit_step(event.id)?;
}

// finalize() runs here in normal user space — no signal restrictions
recorder.finalize()?;
launch_tui_if_needed();
```

This guarantees `finalize()` always runs in unrestricted user-space context with full access to allocators, mutexes, and file I/O.

---

## 13. Glossary

| Term | Definition |
|---|---|
| **Step** | A semantic execution boundary in `rev`. Triggered by a syscall event or explicit checkpoint. One step ≈ one "thing that happened" (a network call, a file read, etc.) |
| **Trace** | A `.rev-trace` file. The complete recording of all syscall events for one process run. |
| **Delta** | A record of which memory pages changed between two consecutive steps. |
| **Snapshot** | A full copy of process memory at a given step. Taken every `SNAPSHOT_INTERVAL` steps. |
| **Merkle DAG** | The tree structure linking all delta nodes. Each node hashes its content + its parent, enabling integrity verification. |
| **Headless process** | A copy of the target interpreter spawned during replay. It receives recorded syscall return values instead of making real syscalls. |
| **Introspector** | The per-runtime component that reads variable values and call stacks from the headless process. |
| **Non-deterministic input** | Any value the program receives from outside itself: time, randomness, network data, file contents, environment variables. These are what `rev` records. |
| **Flight recorder mode** | The default behavior: silent recording in the background, TUI only opens on crash. |
| **Step boundary** | The moment `rev` pauses the child process to take a delta snapshot and record events. |

---

*This document is the ground truth for the `rev` implementation. All architectural decisions are made here first. If code disagrees with this document, the document wins — update the code, not the document, unless a deliberate architectural decision has been made and the document is updated accordingly.*