# Architecture

## Crate Dependency Graph

```
┌────────────────────────────────────────────────────────────┐
│                        youtui                              │
│  (35k LOC, 71 files - main TUI application)                │
│                                                            │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ app/ - core application logic                        │  │
│  │  ├── app.rs - main event loop, callback dispatch     │  │
│  │  ├── ui.rs - YoutuiWindow, HelpMenu, component tree  │  │
│  │  ├── server/ - backend tasks, providers, downloader  │  │
│  │  ├── view/ - table/filter/sort system                │  │
│  │  └── component/ - action handler, key router traits  │  │
│  ├── config/ - config.toml parsing + keymap IR          │  │
│  ├── widgets/ - scrolling_list, scrolling_table, tab    │  │
│  ├── youtube_downloader/ - yt-dlp + native downloaders  │  │
│  └── audio-player/                              │  │
└────────┬────────────────────────────────────────────────┘  │
         │ depends on:                                       │
    ┌────┴────┬──────────┬──────────────┬───────────────┐    │
    ▼         ▼          ▼              ▼               │    │
┌────────┐ ┌────────┐ ┌──────────┐ ┌──────────┐ ┌─────────┐ │
│ ytmapi │ │ async  │ │ json     │ │ vi-text  │ │ audio   │ │
│ -rs    │ │-callbkd│ │ -crawler │ │ -editor  │ │-player  │ │
│ 12.8k  │ │ 1.8k   │ │ 1.0k     │ │ 2.3k     │ │ 0.8k    │ │
└────────┘ └────────┘ └──────────┘ └──────────┘ └─────────┘ │
└────────────────────────────────────────────────────────────┘
```

## 3-Layer Callback Architecture

```
┌──────────────┐     AsyncTask<T>     ┌─────────────┐     BackendTask    ┌──────────┐
│   Frontend   │ ──────────────────►  │ TaskManager  │ ────────────────►  │ Backend  │
│  (UI state)  │                      │ (spawn/await)│                    │ (Server) │
│              │ ◄─────────────────── │              │ ◄────────────────  │          │
│  Ratatui TUI │     FrontendEffect   │ AsyncCallback│     Result<T>     │ API/ytdlp│
│  components  │     (state mutation) │    Manager   │                   │  /ffmpeg │
└──────────────┘                      └─────────────┘                    └──────────┘
```

### Flow

1. **Event** arrives (keyboard, media key, IPC)
2. **Frontend** handles it → may spawn a `BackendTask` via `AsyncTask::new_future_try(task, ok_handler, err_handler, metadata)`
3. **TaskManager** sends the `BackendTask` to the **Backend**
4. **Backend** executes the task (API call, download, decode, etc.)
5. **Result** returns to TaskManager → calls `FrontendEffect` handler on frontend state
6. **Frontend** re-renders via `terminal.draw(|f| ...)`

### Key Types

```rust
// A task that runs on the backend
trait BackendTask<S> {
    type Output: Send + 'static;
    type MetadataType: Debug + Send + 'static;
    fn into_future(self, backend: &S) -> impl Future<Output = Self::Output> + Send;
}

// An effect that mutates frontend state when task completes
trait FrontendEffect<Component, Backend, Metadata> {
    fn handle(self, component: &mut Component, backend: &Backend, metadata: Metadata);
}

// Wrapper combining a task + handlers into a spawnable unit
struct AsyncTask<C, S, M> { ... }
```

## Window Context Routing

Youtui has a `WindowContext` enum that controls which component receives keyboard events:

```rust
pub enum WindowContext {
    Browser,        // Search tabs (artist/song/playlist/library)
    Playlist,       // Queue view
    Logs,           // Logger/tracing view
    Lyrics,         // Lyrics popup overlay
    SongInfo,       // Song info popup overlay
    PlaylistSavePopup,   // Save-to-playlist popup
    PlaylistUpdatePopup, // Add-to-playlist popup
}
```

### Context priority (highest to lowest)

1. **Popups** - lyrics, song info, album art, config editor, save/update playlist (full intercept)
2. **Command mode** (`:` prompt) - ViTextEditor captures all keys
3. **Quit confirm** - `y`/`n` only
4. **Current context** - Browser, Playlist, or Logs
5. **Global** - F-keys, volume, seek, toggle browser/queue

### Context switching

- `F1` - toggle YTM search panel (overlays current context)
- `F2` - toggle Browser (saves/restores prev_context)
- `F3` - toggle Playlist (saves/restores prev_context)
- `F11` - toggle Logs
- `Esc`/`q` in popups - close popup, return to underlying context

## Component Trait System

Every UI component implements:

```rust
// Maps keyboard events to actions
trait ActionHandler<A: Action> {
    fn apply_action(&mut self, action: A) -> impl Into<YoutuiEffect<Self>>;
}

// Provides keybinding lookup for a component
trait KeyRouter<A> {
    fn get_active_keybinds(&self, config: &Config) -> impl Iterator<Item = &Keymap<A>>;
    fn get_all_keybinds(&self, config: &Config) -> impl Iterator<Item = &Keymap<A>>;
}

// Describes an action (for help screen display)
trait Action {
    fn context(&self) -> Cow<'_, str>;
    fn describe(&self) -> Cow<'_, str>;
}
```

### Macro

```rust
// Generates the impl_youtui_component!(MyComponent) macro boilerplate:
// - impl ActionHandler<AppAction> (delegates to inner action handler)
// - impl DominantKeyRouter (keybinding priority)
```

## Event Loop

```rust
// app.rs:run()
loop {
    tokio::select! {
        Some(event) = event_handler.next() => {
            self.handle_event(event).await;
        }
        Some(outcome) = task_manager.get_next_response() => {
            self.handle_effect(outcome);
        }
    }
    terminal.draw(|f| draw_app(f, &mut window_state, ...));
}
```

### `handle_event` path

```
Event::Key(k) → YoutuiWindow::handle_key_event(k)
  → keymap lookup (global → context → dominant)
  → action dispatch (AppAction enum)
  → if task needed: AsyncTask::new_future_try(...)
  → if callback: AppCallback handled in app.rs:handle_callback
```

### `handle_effect` path

```
TaskOutcome { result, metadata } → FrontendEffect::handle(state, backend, metadata)
  → state mutation: playlist.add_songs(), set_lyrics(), etc.
  → backend mutation: download triggers, decode triggers
  → next effect may chain: e.g., download complete → decode next
```
