# Subsystem: Playlist Editor

Full-screen vim-driven playlist editor popup. Opens from Browser > Library > Playlists on Enter (`o.e` in tracks view, or Enter on playlist in list view).

## Files

- `app/ui/playlist/playlist_editor_popup.rs` (~748 lines) — editor UI + keybindings
- `app/ui/playlist/effect_handlers_playlist.rs` — overwrite save chain handlers
- `app.rs` — `OpenPlaylistEditor`, `OverwritePlaylistTracks` callbacks

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
    pub undo_stack: Vec<Vec<ListSong>>,    // 100-level undo
    pub yank_buffer: Vec<ListSong>,         // yanked lines
    pub delete_mode: bool,                  // d operator waiting for motion
    pub yank_mode: bool,                    // y operator waiting for motion
    pub visual_mode: bool,                  // visual line selection
    pub visual_start: usize,                // visual selection anchor
}
```

## Motions

| Key | Action |
|-----|--------|
| `j`/`k` | Move down/up (with `Nj`/`Nk` count prefix) |
| `g`/`gg` | Go to first line (or `Ng` to line N) |
| `G` | Go to last line (or `NG` to line N) |

## Delete (`d` operator)

| Key | Action |
|-----|--------|
| `dd`/`Ndd` | Delete N lines |
| `dN`+`j` | Delete N lines down |
| `dN`+`k` | Delete N lines up |
| `dg` | Delete to top |
| `dG`/`D` | Delete to end |

## Yank (`y` operator)

| Key | Action |
|-----|--------|
| `yy`/`Nyy` | Yank N lines |
| `yj` | Yank line below |
| `yk` | Yank line above |
| `ygg` | Yank to top |
| `yG` | Yank to end |
| `Y` | Yank current line (`yy`) |

## Paste

| Key | Action |
|-----|--------|
| `p` | Paste below cursor |
| `P` | Paste above cursor |

## Visual Mode

| Key | Action |
|-----|--------|
| `V` | Toggle visual line selection |
| `j`/`k` | Extend selection |
| `d`/`x` | Delete selection |
| `y` | Yank selection |
| `p`/`P` | Paste over selection |

## Undo/Redo

| Key | Action |
|-----|--------|
| `u` | Undo (100-level stack) |
| `C-r` | Redo (unbound yet) |

## Insert/Reorder

| Key | Action |
|-----|--------|
| `o`/`O` | Insert blank line below/above |
| `J`/`K` | Move line down/up (swap, with undo) |

## Command Mode (`:`)

| Command | Action |
|---------|--------|
| `:w` | Save (overwrite) |
| `:wq` | Save + quit |
| `:q` | Quit (warns if modified) |
| `:q!` | Force quit (no confirm) |
| `:d N` | Delete track at position N |
| `:m N M` | Move track from N to M |
| `:rename` | Rename playlist |
| `:privacy` | Set privacy status |
| `:rate` | Rate playlist |

Other: `q`/`Esc` close, `E` save to existing playlist.

## Capacity Bar

Shown at top: `Tracks: N/5000 [■■■■] [□□□□] [□□□□] [□□□□]` (4 blocks × 1250). Updates live on insert/delete.

## Save Flow (Overwrite)

### Editor save (`:w` or `:wq`)

1. Collects all current video_ids from editor tracks
2. Dispatches `AppCallback::OverwritePlaylistTracks(playlist_id, video_ids)`
3. App closes popup, spawns chain:
   - `GetPlaylistTracks` → fetches current remote tracks
   - `HandleOverwriteGetTracks` → extracts `set_video_id` from remote tracks
   - `RemovePlaylistItems` → removes all remote tracks
   - `HandleOverwriteRemoveDone` → spawns `AddSongsToPlaylist` with new IDs
   - `HandleAddSongsOk` → done
4. Playlist editor popup dismissed, library playlists marked for refresh

### Existing playlist save (`E` key)

Opens `PlaylistUpdatePopup` with track IDs. User selects target playlist.
- `[Append]` mode: just adds tracks (no removal)
- `[Replace]` mode: uses same overwrite chain as editor (fetch → remove → add)

## Architecture

- `save_state()` pushes full track snapshot to `undo_stack` before every mutation
- `yank_buffer: Vec<ListSong>` stores copied lines
- `delete_mode`/`yank_mode` are operator-mode flags (like vim's d/y waiting for motion)
- `visual_mode` + `visual_start` for visual line selection
- Selection color: cyan background

## Integration

- Wired in `app/ui/browser/library.rs` — `OpenPlaylistEditor` handler
- Routed in `youtui/src/app/ui.rs` — `playlist_editor_popup` field + event interception
- Drawn in `app/ui/draw.rs` — popup render
- Callback in `app.rs` — `OpenPlaylistEditor` creates the popup, `OverwritePlaylistTracks` triggers save chain
