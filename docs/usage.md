# `rev` Time-Travel Debugging Guide

This document describes the command-line usage, keybindings, and internal design of `rev`.

---

## 1. Command-Line Interface

`rev` wraps command execution to record performance and memory traces, or lets you replay existing traces.

### Commands & Flags

```bash
# 1. Record process execution (starts TUI automatically on crash)
rev python my_script.py

# 2. Record process execution, but suppress opening the TUI on crash
rev --no-tui python my_script.py

# 3. Print verbose recording stats (total events, compression ratios, duration)
rev -v python my_script.py

# 4. Replay an existing trace file in the interactive TUI
rev --replay ~/.rev/traces/12345_162383848.rev-trace

# 5. Export program state at a specific step to stdout as formatted JSON
rev --export ~/.rev/traces/12345_162383848.rev-trace 42
```

---

## 2. Interactive TUI Interface

When the TUI launches (either directly via `--replay` or automatically after a crash), the terminal screen is split into five distinct panels:

1. **Call Stack (Left)**: Displays the call frames active in the current execution step, highlighting function names and file locations.
2. **Event Details (Middle)**: Shows detailed syscall/checkpoint parameters, timestamps, file descriptors, and a hex/ASCII preview of return byte payloads.
3. **Variable Inspector (Right)**: Shows local variables in scope. Variables that changed in the current step are highlighted with a yellow bullet (`●`).
4. **Timeline Scrubber (Bottom)**: Draws a timeline representing all execution steps. Current position is indicated with a highlighted caret (`▲`). High step counts are sampled down dynamically to fit the terminal size.
5. **Footer / Status Bar**: Summarizes active keybindings and displays temporary notifications (e.g., JSON export confirmations).

### Keyboard Navigation Reference

| Key / Shortcut | Mode | Description |
|---|---|---|
| `q` or `Esc` | Normal | Exits the TUI and restores original terminal state cleanly. |
| `h` or `Left Arrow` | Normal | Steps backward one instruction step. |
| `l` or `Right Arrow` | Normal | Steps forward one instruction step. |
| `k` or `Up Arrow` | Normal | Jumps backward 10 instruction steps. |
| `j` or `Down Arrow` | Normal | Jumps forward 10 instruction steps. |
| `/` | Normal | Opens **Search Mode**. |
| `d` | Normal | Toggles **Expanded Event Details** pop-up (useful for viewing large return payloads). |
| `Ctrl + E` | Normal | Triggers **Export State** confirmation dialog. |
| `y` / `n` | Export | Confirms or cancels state serialization. |
| `Enter` | Search | Confirms search query, seeking to the next matching syscall. |
| `Esc` | Search / Popups | Cancels the popup or search field and returns to Normal Mode. |

---

## 3. Architecture & Storage

`rev` records deterministic execution states by tracking non-deterministic inputs and memory page deltas.

### Memory Snapshotting and Delta Tracking
Memory tracking is managed by the `delta` crate:
1. **Snapshots**: At step 0 and at regular intervals (default: every 100 steps), `rev` dumps full snapshots of dirty memory pages.
2. **Page Diffs**: For intermediate steps, `rev` stores only the address and modified content (`after` bytes) of modified memory pages.
3. **Reconstruction**: When the developer seeks to step `N`, the `delta` engine finds the nearest snapshot `S <= N`, loads it, and applies the subsequent forward diffs up to `N` in memory.
4. **Database Storage**: Page snapshots and diffs are organized in a **SQLite Merkle DAG** stored locally under `~/.rev/deltas/<pid>.db`. Merkle hashes prevent tampering and ensure database integrity.

### Syscall Interception
On Linux, `rev` uses `ptrace(PTRACE_SYSCALL)` to intercept the entry and exit of non-deterministic syscalls:
- Time readings (`time`, `clock_gettime`)
- Entropy collection (`getrandom`, `/dev/urandom` reads)
- File reads (`read`, `pread`)
- Socket reads (`recv`, `recvfrom`)
- Environment variable reads

During replay, the replayer spawns a headless copy of the target interpreter in suspended mode, fast-forwards it, traps its syscall exits, and overrides their return values with the recorded values, reconstructing the original memory state.
