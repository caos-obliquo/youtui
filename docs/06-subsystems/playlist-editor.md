# Subsystem: PlaylistEditor

Full-screen vim-driven playlist editor popup. Opens from Browser > Library > Playlists on Enter.

## File

`app/ui/playlist/playlist_editor_popup.rs` (~320 lines)

## Struct

```rust
pub struct PlaylistEditorPopup {
    pub playlist_id: PlaylistID<'static>,
    pub playlist_title: String,
    pub tracks: Vec<ListSong>,
    pub cursor: usize,
    pub scroll_offset: usize,
    pub command_mode: bool,
    pub command_editor: ViTextEditor,
    pub modified: bool,
}
```

## Vim Motions

| Key | Action |
|-----|--------|
| `j`/`k` | Move cursor up/down |
| `gg`/`G` | First/last track |
| `dd` | Delete track at cursor |
| `J`/`K` | Move track down/up (swap) |
| `u` | Undo (placeholder, no-op) |
| `:` | Enter command mode |
| `Esc` | Exit command mode / close editor |
| `q` | Close editor (no confirm) |
| `o` | Open context menu |
| `E` | Save to existing playlist |

## Command Mode (`:`)

| Command | Action |
|---------|--------|
| `:w` | Save to existing playlist (opens PlaylistUpdatePopup) |
| `:q` | Quit editor |
| `:wq` | Save + quit |
| `:q!` | Force quit (no confirm) |
| `:d N` or `:delete N` | Delete track at position N |
| `:m N M` or `:move N M` | Move track from N to M |
| `:a URL` or `:add URL` | Add video to playlist (URL, no-op placeholder) |
| `:h` or `:help` | Show command help |

## Save Flow

`:w` or `o.E` → collects video_ids from current tracks
→ `AppCallback::OpenPlaylistUpdatePopup(video_ids)`
→ Opens existing PlaylistUpdatePopup with user's playlists
→ User selects target playlist
→ `RemovePlaylistItems` + `AddSongsToPlaylist` API calls
→ Popup closes, editor view restored

## Integration

- Wired in `app/ui/browser/library.rs` — `ActivateSelected` on Playlists category
- Routed in `app/ui.rs` — `playlist_editor_popup` field + event interception
- Drawn in `app/ui/draw.rs` — popup render after config editor
- Callback in `app.rs` — `OpenPlaylistEditor` creates the popup
- No custom keybindings — popup handles keys directly (like ConfigEditorPopup)
