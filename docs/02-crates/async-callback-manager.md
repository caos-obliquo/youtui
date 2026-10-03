# Crate: async-callback-manager

**1,802 LOC, 12 files** - Generic async callback management for UI frameworks.

## Purpose

Decouples task execution (backend) from state mutation (frontend). UI components define what to run and how to handle results without knowing about threading or async runtimes.

## Core Types

```rust
/// A task that encapsulates a future + success/error handlers
/// Parametrized over: Component type C, Backend type S, Metadata type M
pub struct AsyncTask<C, S, M> { ... }

impl<C, S, M> AsyncTask<C, S, M> {
    pub fn new_no_op() -> Self;
    pub fn new_future<F, H>(future: F, ok_handler: H, metadata: M) -> Self
    pub fn new_future_try<F, H, E>(future: F, ok_handler: H, err_handler: E, metadata: M) -> Self;
    pub fn map_frontend<F>(self, f: F) -> AsyncTask<T, S, M>;
    pub fn with_delay(self, delay: Duration) -> Self;
}

/// Manager that runs in background, receives task completions
/// Generic over frontend type, backend type, metadata type
pub struct AsyncCallbackManager<Frntend, Bkend, Md> { ... }

impl<Frntend, Bkend, Md: PartialEq> AsyncCallbackManager<Frntend, Bkend, Md> {
    pub fn new() -> Self;
    pub fn spawn_task(&mut self, backend: &Bkend, task: AsyncTask<Frntend, Bkend, Md>);
    pub async fn get_next_response(&mut self) -> Option<TaskOutcome<Frntend, Bkend, Md>>;
}

/// Result delivered to frontend
pub struct TaskOutcome { ... }
```

## Module Tree

```
src/
├── lib.rs                   - Re-exports
├── adaptors.rs              - Task construction helpers
├── constraint.rs            - Concurrency limiting (semaphore)
├── error.rs                 - Error handling for task execution
├── manager.rs               - AsyncCallbackManager main implementation
├── manager/task_list.rs     - Internal task storage + dispatch
├── panicking_receiver_stream.rs - Stream wrapper for non-panicking receive
├── task.rs                  - AsyncTask struct, AsyncTaskKind enum (Future / Stream / Multi / NoOp)
├── task/dyn_task.rs         - Dynamic dispatch for BackendTask
├── task/dyn_task/handlers.rs- Handler functions
├── task/map.rs              - Frontend type mapping (map_frontend)
└── task/tests.rs            - Unit tests
```

## Constraint System

```rust
// constraint.rs - note: `ConstraitType` spelling (missing "n") is in the source
pub struct Constraint<Cstrnt> {
    pub(crate) constraint_type: ConstraitType<Cstrnt>,
}

impl<Cstrnt> Constraint<Cstrnt> {
    pub fn new_block_same_type() -> Self;
    pub fn new_kill_same_type() -> Self;
    pub fn new_block_matching_metadata(metadata: Cstrnt) -> Self;
}

pub enum ConstraitType<Cstrnt> {
    BlockSameType,
    KillSameType,
    BlockMatchingMetatdata(Cstrnt), // note: `Metatdata` spelling is in the source
}
```

Used to prevent too many concurrent downloads, rate-limited API calls, etc.

## Architecture

```
spawn_task(task, backend) 
  → wrap future + handlers into Task enum
  → check constraint 
  → if allowed: run immediately
  → if blocked: queue until slot available
  → on completion: call ok_handler or err_handler 
  → push TaskOutcome to receiver
```

## Test Count

```bash
cargo test --release -p async-callback-manager
# 14 tests pass (3 lib + 11 integration)
```
