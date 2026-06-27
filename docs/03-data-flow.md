# Data Flow

Complete trace of how a keypress becomes a UI update.

## 1. Event Reception

```
Crossterm event stream → EventHandler::next() → Youtui::handle_event()
```

`app.rs` main loop uses `tokio::select!` to multiplex between keyboard events and task completions:

```rust
loop {
    tokio::select! {
        Some(event) = self.event_handler.next() => {
            self.handle_event(event).await;
        }
        Some(outcome) = self.task_manager.get_next_response() => {
            self.handle_effect(outcome);
        }
    }
    self.terminal.draw(|f| draw_app(f, &mut self.window_state, ...));
}
```

## 2. Key Event Routing

`handle_event` → `YoutuiWindow::handle_key_event(k)` (app/ui.rs:707)

Priority order:

```
1. Config editor popup active?
   → route to config_editor_popup.handle_key(k)

2. Quit confirm screen?
   → y/n keys only

3. Command mode active?
   → route to command_editor.handle_key(k)
   → Enter submits text as command or URL

4. Lyrics popup active?
   → route to lyrics_popup.handle_key(k)
   → may return AppCallback (ClosePopup, SeekBack, SeekForward, SeekTo)

5. Album art popup active?
   → route to album_art_popup.handle_key(k)

6. Song info popup active?
   → route to song_info_popup.handle_key(k)

7. Playlist save popup?
   → route to popup.handle_key(k)

8. Playlist update popup?
   → route to popup.handle_key(k)

9. Count prefix detected (digit key)?
   → accumulate pending_count, return

10. Keymap lookup:
    global_handle_key_stack_with_count() or global_handle_key_stack()
    → match against active keymaps
    → resolve to AppAction variant
```

## 3. Action Dispatch

`global_handle_key_stack` (app/ui.rs:919) resolves the key stack against keymaps in priority order:

```
Dominant keybinds > active context keybinds > global keybinds
```

Each keymap context:
- `Global` - F1/F2/F3/F7/F11, volume, seek, play/pause, quit
- `Playlist` - j/k/d/y/V, shuffle, repeat, delete, etc.
- `Browser` - tab switching, navigation
- `BrowserSongs` - song list actions
- `BrowserArtists` - artist list actions
- `BrowserPlaylists` - playlist list actions
- `Filter` - local filter mode
- `Sort` - sort mode
- `Help` - help screen
- `TextEntry` - ViTextEditor keybindings
- `List` - up/down/page navigation
- `Log` - log viewer
- `PlaylistSavePopup` - save playlist popup
- `PlaylistUpdatePopup` - update playlist popup

## 4. Task Spawning

Actions that need backend work spawn `AsyncTask`:

```rust
// In action handler:
match action {
    PlaylistAction::ViewLyrics { artist, title } => {
        let task = AsyncTask::new_future_try(
            GetLyrics(artist, title, genius_token),
            HandleLyricsOk,
            HandleLyricsErr,
            None,
        ).map_frontend(|this: &mut Playlist| this);
        return (task, None);  // task + optional AppCallback
    }
}
```

The task is returned from the handler and spawned in `app.rs`:

```rust
AppAction::Playlist(PlaylistAction::ViewLyrics { ... }) => {
    let (effect, callback) = self.playlist.handle_view_lyrics(...);
    if callback.is_some() { self.handle_callback(callback.unwrap()); }
    // effect contains the GetLyrics AsyncTask
    self.task_manager.spawn_task(&self.server, effect);
}
```

## 5. Effect Handling

When a task completes:

```rust
fn handle_effect(&mut self, outcome: TaskOutcome) {
    // outcome contains:
    // - result: TaskResult (Ok/Err)
    // - handler: FrontendEffect fn pointer
    // - metadata: TaskMetadata (for matching)
    
    // Call the handler on the component:
    handler.handle(&mut self.window_state.playlist, &self.server, metadata);
}
```

The handler mutates UI state:

```rust
impl FrontendEffect<Playlist, ArcServer, TaskMetadata> for HandleLyricsOk {
    fn handle(self, playlist: &mut Playlist, _backend: &ArcServer, _meta: TaskMetadata) {
        if let Some(popup) = &mut playlist.lyrics_popup {
            popup.set_lyrics(self.0);  // state mutation
        }
    }
}
```

## 6. Re-render

After every event or effect:

```rust
self.terminal.draw(|f| {
    draw_app(f, &mut self.window_state, &self.terminal_image_capabilities);
});
```

The `draw_app` function (app/ui/draw.rs) renders:

1. Window content based on `WindowContext` (Browser/Playlist/Logs)
2. Help screen if `help.shown`
3. Key pending popup if in key sequence
4. Active popups (save, update, lyrics, album art, config editor)
5. Quit confirm overlay
6. Command mode `:` prompt
7. Header (mode indicator, song info, progress)
8. Footer (keybinding hints, context menu)

## 7. Complete Example: Play a Song

```
User presses Enter on song in Browser
  → Event::Key(Enter) → handle_key_event
  → keymap resolves: BrowserSongsAction::PlaySong
  → action handler: AddSongsToPlaylistAndPlay(vec![song])
  → returns (no_op_task, Some(AppCallback::AddSongsToPlaylistAndPlay))
  → handle_callback:
      playlist.add_song_to_playlist(song)
      playlist.play_song(id)
      → spawns DecodeSong task
      → spawns GetLyrics task
  → DecodeSong completes → player starts playback
  → GetLyrics completes → lyrics_popup.set_lyrics(text)
  → render: header shows new song, lyrics popup shows text
```

## 8. Complete Example: Seek with `]` in Lyrics

```
User presses ] in lyrics popup
  → lyrics_popup::handle_key → returns AppCallback::SeekForward
  → handle_callback:
      playlist.handle_seek(5s, SeekDirection::Forward)
      → spawns Seek task on audio sink
  → audio adjusts playback position
  → auto-advance logic checks if near track end
```
