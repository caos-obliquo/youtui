# Subsystem: Notes Popup

## What

A vim-driven text editor popup for storing frequently-visited URLs, song links, album IDs, and personal notes. Accessible from anywhere in the app via `:notes` command.

## Why

The user frequently visits specific music URLs (albums, playlists, songs) that are hard to discover through YTM search alone - niche artists, algorithm-unfriendly genres, specific album IDs. Instead of retyping URLs into the `:` command each time, Notes provides a persistent scratchpad with vim navigation and URL-opening on Enter.

Built suckless: plain text file, no database, no JSON, no serialization - just `~/.config/youtui/notes.txt` read and written as-is.

## File Format

```
~/.config/youtui/notes.txt - plain UTF-8 text, one line per entry
```

> **Cross-Platform:** Config path resolved via `directories` crate (`ProjectDirs::config_local_dir()`) - `~/.config/youtui/` on Linux, `~/Library/Application Support/com.nick42.youtui/` on macOS. Temp files use `std::env::temp_dir()`.

Lines starting with `http://` or `https://` are URLs - pressing Enter on them opens the URL in yt-dlp. Lines without URL prefix are plain text notes (descriptions, metadata, reminders).

Example:
```
# My saved URLs
https://music.youtube.com/watch?v=EqAtk5D1R1Y
My notes about this song
https://genius.com/Queen-bohemian-rhapsody-lyrics
```

## Keybindings

| Key | Mode | Action |
|-----|------|--------|
| `Esc` | Normal | Close popup |
| `Esc` | Insert | Switch to Normal mode |
| `i` | Normal | Enter Insert mode at cursor |
| `a` | Normal | Enter Insert mode after cursor |
| `I` | Normal | Enter Insert mode at line start |
| `A` | Normal | Enter Insert mode at line end |
| `o` | Normal | Open new line below, Insert mode |
| `O` | Normal | Open new line above, Insert mode |
| `:` | Normal | Enter Command mode |
| `Enter` | Normal on URL | Open URL at cursor line |
| `Enter` | Insert | Newline |
| `j`/`k` | Any | Move cursor down/up |
| `h`/`l` | Any | Move cursor left/right |
| `gg`/`G` | Normal | Jump to first/last line |
| `w`/`b`/`e` | Normal | Word motions |
| `0`/`$` | Normal | Line start/end |
| `dd` | Normal | Delete line |
| `yy` | Normal | Yank line to clipboard |
| `p`/`P` | Normal | Paste below/above |
| `u`/`C-r` | Normal | Undo/redo |
| `V`/`v` | Normal | Enter Visual line/char mode |
| `d`/`y` | Visual | Delete/yank selection |

### Command Mode (`:`) keybindings

| Command | Action |
|---------|--------|
| `:w` | Save notes to file |
| `:wq` | Save and quit |
| `:q` | Quit without saving |
| `:q!` | Force quit |

## Architecture Decisions

### Why plain text instead of JSON?

- **Editability**: User can `vi ~/.config/youtui/notes.txt` outside youtui
- **Reliability**: No JSON parse errors, no schema drift, no file corruption from half-writes
- **Suckless**: Text is the universal interface. JSON adds a dependency without benefit
- **Version control**: Plain text diffs cleanly in git

### Why no hardware cursor (unlike ConfigEditorPopup)?

The Notes popup uses the `▎` character from `ViTextEditor::render_simple()` as cursor indicator - same behavior as F1 search box. ConfigEditorPopup additionally calls `Frame::set_cursor_position()` to position the hardware cursor, but this caused visual mismatch when both the `▎` character and the hardware cursor appeared at slightly different positions. Matching the search box behavior (cursor character only) proved more consistent and less error-prone across terminals.

### Why Esc behavior differs from vim?

Standard vim: Esc in insert mode exits to Normal mode, cursor stays in place.

Previous VTE behavior (now fixed): `cursor -= 1` moved cursor back one character on Esc. This was a bug inherited from vi-text-editor's original implementation. Removed to match standard vim behavior.

### Why no cursor style switching (blinking bar for insert, block for normal)?

`SetCursorStyle` from crossterm worked when called directly via `execute!` to stdout, but ratatui's `terminal.draw()` flush would override the style. Setting it inside the draw closure wasn't possible because `Frame` doesn't expose cursor style control. The `▎` character provides visual mode indication via its position and the mode char in the title bar (`[I]`/`[N]`).

## File Lifecycle

```
1. User types `:notes` in command mode
2. ui.rs command handler reads ~/.config/youtui/notes.txt
3. NotesPopup created with file content
4. User edits via vim motions
5. :w → std::fs::write overwrites file atomically
6. Next `:notes` reads the updated file
```

The file persists across sessions and reboots - stored on disk, not in memory.

## Integration Points

| Point | File | Purpose |
|-------|------|---------|
| Event routing | `ui.rs:673-682` | NotesPopup intercepts events before normal dispatch |
| Draw | `draw.rs:98-100` | NotesPopup rendered over `window_chunk` |
| Close handler | `ui.rs:1490-1506` | Clears popup + restores context |
| Command handler | `ui.rs:721-732` | `:notes` command reads file and creates popup |

## Tests

Visual testing only (no unit tests):
- Open `:notes` - popup displays with file content
- Press `i` - cursor changes to insert position, title shows `[I]`
- Press `Esc` - exits to Normal mode, title shows `[N]`
- Press `:` then `:w` - saves, stays open
- Press `:` then `:q` - quits
- Enter on URL line - opens in yt-dlp
