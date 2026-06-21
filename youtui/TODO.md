# youtui (caos-obliquo fork)

## Tests
- youtui: 126 pass, 0 fail, 4 ignored
- ViTextEditor: 65/65 pass

## VTE Features (ALL DONE ✅)
- `s`/`S` substitute, `D`/`C`/`Y` synonyms, `visual o` exchange
- `W`/`B`/`E` BIG-word motions, `dW`/`cW`/`dB`/`cB`/`dE`/`cE`
- `i'`/`a'`/`` i` ``/`` a` `` text objects
- Nested-pair depth-counter, `want_col` column preservation
- Surround `cs`/`ds`/`ys`, switch keyword `^A`/`^X`
- proptest invariants (cursor bounds, undo/redo roundtrip)
- UTF-8 char boundary safety

## Lyrics Pipeline
- **Priority 0**: Genius JSON API (`genius.com/api/songs/{id}/lyrics`)
- Priority 1: Musixmatch (free tier may be partial)
- Priority 2: Bandcamp via `bandcamp-lyrics` CLI
- Priority 3: `lyr` CLI
- Annotations rendered in right-side panel when `a` toggled

## PlaylistEditorPopup
- Full vim-driven playlist editing (j/k/gg/G/dd/J/K)
- `:w` save, `:q` quit, `:wq` save+quit, `:q!` force quit
- `:d N` delete, `:m N M` move, `:a URL` add, `:h` help
- `o.E` shortcut for save to existing playlist
- Opens from Browser > Library > Playlists on Enter

## Known Issues
- `o.a` conflict: `browser_artist_songs` uses `o.a` = PlayAlbum (not GoToArtist)
- Native downloader 403 Forbidden (use yt-dlp)
- Crossterm 0.29 `Event::Key` destructure mismatch
- Musixmatch may return partial lyrics (Genius always preferred)
