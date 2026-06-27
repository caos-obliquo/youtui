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
pub struct AsyncCallbackManager { ... }

impl AsyncCallbackManager {
    pub fn new() -> Self;
    pub fn spawn_task<S, C, M>(&mut self, backend: &S, task: AsyncTask<C, S, M>);
    pub fn get_next_response(&mut self) -> Option<TaskOutcome>;
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
├── task.rs                  - Task enum (Future / BackendTask variants)
├── task/dyn_task.rs         - Dynamic dispatch for BackendTask
├── task/dyn_task/handlers.rs- Handler functions
├── task/map.rs              - Frontend type mapping (map_frontend)
└── task/tests.rs            - Unit tests
```

## Constraint System

```rust
pub enum Constraint {
    Unlimited,         // No limit
    Max(u32),          // Max N concurrent tasks of this type
    Sequential,        // One at a time (FIFO queue)
    Ordered,           // Sequential + preserve order
}
```

Used to prevent too many concurrent downloads, rate-limited API calls (MusicBrainz: 1 req/s), etc.

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
