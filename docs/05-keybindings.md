# Keybindings

All default keybindings by context. Customizable in `config.toml`. Source of truth is `youtui/src/config/keymap.rs` (`default_*_keybinds`), action names come from `youtui/src/app/ui/action.rs` and each panel's `*Action` enum.

List movement (`j`/`k`/`gg`/`G`, Ctrl-b/Ctrl-f, etc.) is shared: it lives in the List map and applies wherever a list is focused (queue, browser panels, popups).

## Global

| Key | Action | Description |
|-----|--------|-------------|
| `Space` | PlayPause | Toggle playback |
| `>` | NextSong | Next track |
| `<` | PrevSong | Previous track |
| `]` | SeekForward | Seek forward 5s |
| `[` | SeekBack | Seek back 5s |
| `+` | VolUp | Volume up 5% |
| `-` | VolDown | Volume down 5% |
| `?` | ToggleHelp | Show keybinding help |
| `F1` | Browser(BrowserAction::Search) | Toggle YTM search |
| `F2` | ToggleBrowser | Toggle browser view |
| `F3` | TogglePlaylist | Toggle queue view |
| `F4` | Recommend | Open recommendations (F4 popup) |
| `Shift+F4` | ReloadRecommendations | Refresh recommendations |
| `F11` | ViewLogs | Show logs |
| `q` | Quit | Quit (with confirm) |
| `C-c` | Quit | Quit |
| `C-e` | EditConfig | Edit config.toml |
| `:` | OpenUrl | Open command prompt |
| `/` | FuzzyFinder | Global fuzzy finder |

## Playlist (queue)

Movement uses the List map (`j`/`k`/`gg`/`G`, arrows, Ctrl combos). There are no `Playlist(Down/Up/ShiftDown/ShiftUp)` actions and no bare `J`/`K`/`l`/`h` bindings here.

| Key | Action | Description |
|-----|--------|-------------|
| `Enter` | Playlist(PlaylistAction::PlaySelected) | Play selected song |
| `d` then `d`/`g`/`G` | Playlist(DeleteSelected/DeleteToTop/DeleteToBottom) | Delete mode: line / to top / to bottom |
| `o` ... | (mode) Context Menu | Prefix for the table below |
| `V` | Playlist(ToggleVisualMode) | Visual mode |
| `u` | Playlist(UndoDelete) | Undo last delete |
| `p` | Playlist(PasteYanked) | Paste yanked songs |
| `y` / `Y` | Playlist(CopySongUrl/CopyAlbumUrl) | Copy song / album URL |
| `n` / `N` | Playlist(NextSearchResult/PrevSearchResult) | Next / previous search match |
| `Esc` | Playlist(ClearSearch) | Clear search |

## Playlist `o` Mode (Context Menu)

| Key | Action | Description |
|-----|--------|-------------|
| `Enter` | Playlist(PlaySelected) | Play selected |
| `x` | Playlist(DeleteSelected) | Delete from queue |
| `D` | Playlist(DeleteAll) | Delete all |
| `d` | Playlist(ToggleDislike) | Dislike / undislike |
| `t` | Playlist(ToggleLike) | Like / unlike |
| `s` | Playlist(ToggleShuffle) | Toggle shuffle |
| `z` | Playlist(ToggleRepeat) | Cycle repeat mode |
| `r` | Playlist(SortQueue) | Sort queue |
| `S` | Playlist(SortQueue) | Sort queue |
| `f` | Playlist(ForceSplitAlbum) | Force split album |
| `c` | Playlist(ClearDownload) | Clear download (force re-download) |
| `l` | Playlist(ViewLyrics) | View lyrics |
| `i` | Playlist(ViewSongInfo) | View song info |
| `v` | Playlist(ViewAlbumCover) | View album cover |
| `a` / `b` | Playlist(GoToArtist/GoToAlbum) | Go to artist / album |
| `y` / `Y` | Playlist(CopySongUrl/CopyAlbumUrl) | Copy song / album URL |
| `m` | Playlist(ToggleRomaji) | Toggle romaji |
| `A` | Playlist(SetBestQuality) | Cycle audio quality |
| `R` | Playlist(GetRelatedTracks) | Get related tracks |
| `C` | Playlist(SaveToExistingPlaylist) | Add queue to existing playlist |
| `n` | Playlist(SaveToNewPlaylist) | Save queue to new playlist |
| `q` | Playlist(SaveQueue) | Save queue to disk |
| `L` | Playlist(LoadQueue) | Load queue from disk |
| `Q` | Playlist(DeleteQueue) | Delete saved queue |
| `c` | Playlist(ClearDownload) | Clear download status |
| `C` | Playlist(TogglePlaylistCategoryFilter) | Toggle queue category filter |

`o.c` and `o.C` were previously both bound to ClearDownload, which left TogglePlaylistCategoryFilter unreachable.

## Browser

| Key | Action | Description |
|-----|--------|-------------|
| `h` / `l` | Browser(BrowserAction::Left/Right) | Previous / next tab or panel |
| `Backspace` | Browser(BrowserAction::Back) | Navigate back |
| `r` / `R` | BrowserLibrary(ReloadCategory) | Refresh library category |
| `F7` | Browser(BrowserAction::ChangeSearchType) | Cycle search tab |
| `Enter` | (panel-specific) | Primary action (play, open, focus) |
| `o` ... | (mode) Context Menu | Prefix for the table below |
| `g` then `a`/`b` | GoToArtist / GoToAlbum | Go to artist / album |
| `F1` | Browser(BrowserAction::Search) | Toggle YTM search (also global) |

There is no `Browser(LocalFilter)` binding and no `Tab` category binding. The global `/` is FuzzyFinder.

## Browser `o` Mode (Library Tracks View)

| Key | Action | Description |
|-----|--------|-------------|
| `Enter` | BrowserSongs(PlaySong) | Play selected |
| `p` | BrowserSongs(PlaySongs) | Play all |
| `P` | BrowserSongs(AddSongsToPlaylist) | Queue all |
| `g` | BrowserSongs(AddSongToPlaylist) | Save song to playlist |
| `N` | BrowserSongs(InsertNext) | Insert next in queue |
| `q` | BrowserSongs(QueueSong) | Queue song (append to end) |
| `a` | BrowserSongs(GoToArtist) | Go to artist page |
| `b` | BrowserSongs(GoToAlbum) | Go to album page |
| `l` | BrowserSongs(ViewLyrics) | View lyrics |
| `i` | BrowserSongs(ViewSongInfo) | View song info |
| `f` | BrowserSongs(GetPlaylistDetails) | Playlist details popup |
| `y` | BrowserSongs(CopySongUrl) | Copy URL |
| `r` | BrowserSongs(GetRelatedTracks) | Get related tracks |
| `h` | BrowserSongs(RatePlaylist) | Like / unlike album |
| `S` | BrowserSongs(ToggleSubscribeArtist) | Subscribe / unsubscribe artist |
| `s` | Playlist(ToggleShuffle) | Toggle shuffle |
| `z` | Playlist(ToggleRepeat) | Cycle repeat mode |
| `t` | Playlist(ToggleLike) | Like / unlike |
| `d` | Playlist(ToggleDislike) | Dislike / undislike |
| `D` | BrowserSongs(DeletePlaylist) | Delete playlist |
| `R` | BrowserSongs(RenamePlaylist) | Rename playlist |
| `E` | BrowserSongs(EditPlaylistDetails) | Edit playlist details |
| `e` | BrowserSongs(OpenPlaylistEditor) | Open playlist editor (vim) |
| `x` | BrowserSongs(RemoveTrackFromPlaylist) | Remove track |
| `M` | BrowserSongs(MergePlaylist) | Merge playlists |
| `O` | BrowserLibrary(CycleSortOrder) | Cycle sort order |
| `c` | Filter(Close) | Close filter |
| `k` | Sort(Close) | Close sort popup |

Notes: bare `Q` (QueueSong) and `y` (CopySongUrl) also work directly in the library view outside `o`-mode. `o.E` runs EditPlaylistDetails and `o.C` runs SaveToExistingPlaylist (previously both were on `o.E` and saving was unreachable). The plain Songs tab `o`-menu is a subset (no `D`/`R`/`E`/`e`/`x`/`f`/`M`/`O`/`c`/`k`/`q`); artist-songs and playlist-songs panels add `a` PlayAlbum and `A` AddAlbumToPlaylist.

## Browser Library View (Direct Keys)

| Key | Action | Description |
|-----|--------|-------------|
| `Enter` | BrowserLibrary(ActivateSelected) | Open selected (tracks view) |
| `Esc` | BrowserLibrary(DismissTracks) | Back from tracks |
| `J` / `K` | BrowserSongs(MoveTrackDown/MoveTrackUp) | Move track down / up |
| `V` | BrowserSongs(ToggleVisualMode) | Visual mode |
| `d` then `d`/`g`/`G` | BrowserSongs(DeleteSelected/DeleteToTop/DeleteToBottom) | Delete mode |
| `Q` | BrowserSongs(QueueSong) | Queue song (append to end) |
| `y` | BrowserSongs(CopySongUrl) | Copy URL |

Category actions (SwitchToNextCategory, SwitchToPrevCategory, FocusContent, FocusCategory) exist in `BrowserLibraryAction` but have no keymap binding.

## Other Browser Panels

| Key | Action | Description |
|-----|--------|-------------|
| `Enter` | BrowserArtists(DisplaySelectedArtistAlbums) | Show artist albums |
| `o.S` / `o.U` | BrowserArtists(Subscribe/Unsubscribe) | Subscribe / unsubscribe artist |
| `o.s` / `o.z` / `o.t` / `o.d` | Playlist(ToggleShuffle/Repeat/Like/Dislike) | Playback toggles (artists panel) |
| `Enter` | BrowserPlaylists(DisplaySelectedPlaylist) | Show playlist tracks |
| `Enter` | BrowserArtistSongs/PlaylistSongs(PlaySong) | Play selected |
| `C-n` / `C-p` | BrowserSearch(Next/PrevSearchSuggestion) | Search suggestions |
| `Alt-j` / `Alt-k` | BrowserSearch(Prev/NextSearchSuggestion) | Search suggestions (alt) |
| `Esc` | BrowserSearch(Close) | Close search |
| `k` / `j` / `Enter` / `Esc` | PlaylistSavePopup(MoveUp/MoveDown/Save/Cancel) | Save popup |

## List Movement

Shared by queue, browser lists, and popups.

| Key | Action |
|-----|--------|
| `j` / `Down` | List(Down) |
| `k` / `Up` | List(Up) |
| `PageUp` / `Ctrl-b` / `Ctrl-u` | List(PageUp) |
| `PageDown` / `Ctrl-f` / `Ctrl-d` | List(PageDown) |
| `g` then `g` | List(First) |
| `g` then `G` | List(Last) |
| `G` | List(Last) |

There are no `Ctrl-n` / `Ctrl-p` list bindings. Bare `g` opens a Go To mode; it is not First on its own.

## Sort Mode

| Key | Action | Description |
|-----|--------|-------------|
| `Enter` | Sort(SortSelectedAsc) | Sort ascending |
| `Alt-Enter` | Sort(SortSelectedDesc) | Sort descending |
| `Alt-4` | Sort(ClearSort) | Clear sort |
| `4` | Sort(Close) | Close sort popup |
| `Esc` | Sort(Close) | Close sort popup |

## Filter Mode

| Key | Action | Description |
|-----|--------|-------------|
| `Enter` | Filter(Apply) | Apply filter |
| `Alt-3` | Filter(ClearFilter) | Clear filter |
| `3` | Filter(Close) | Close filter |
| `Esc` | Filter(Close) | Close filter |

## Text Entry Context

| Key | Action |
|-----|--------|
| `Enter` | TextEntry(Submit) |
| `Left` | TextEntry(Left) |
| `Right` | TextEntry(Right) |
| `Backspace` | TextEntry(Backspace) |
| `Ctrl-w` | TextEntry(DeleteWord) |

There is no `Esc` binding in the text-entry map.

## Lyrics Popup

Handled in `LyricsPopup::handle_key` (`youtui/src/app/ui/playlist/lyrics_popup.rs`), not via keymap actions. Bare `l`/`h` move the cursor; panel focus uses `Tab`/`BackTab`/`Alt-l`/`Alt-h`.

| Key | Action |
|-----|--------|
| `Esc` / `q` | Close popup |
| `j` / `Down` / `J` | Move cursor down |
| `k` / `Up` / `K` | Move cursor up |
| `H` / `Left` | Cursor left within line |
| `L` / `Right` | Cursor right within line |
| `g` | First line (first annotation when focused there) |
| `G` | Last line (last annotation when focused there) |
| `0` | Line start |
| `$` | Line end |
| `w` / `W` | Next word / WORD start |
| `b` / `B` | Previous word / WORD start |
| `e` / `E` | Next word / WORD end |
| `Ctrl+d` | Page down (10 lines) |
| `Ctrl+u` | Page up (10 lines) |
| `{` / `}` | Previous / next paragraph |
| `a` | Toggle annotations panel |
| `R` | Toggle romaji |
| `Tab` / `Alt-l` | Focus annotations panel |
| `BackTab` / `Alt-h` | Focus lyrics panel |
| `V` | Enter visual mode (`Esc` or `V` exits) |
| `y` | Yank selection (visual mode) to clipboard |
| `Enter` | Lyrics focus: seek `[m:ss]` timestamp; annotations focus: copy annotation to clipboard |
| `/` | Toggle filter |
| `Space` | Toggle play / pause |
| `(` / `)` | View previous / next song in queue |
| `<` / `>` | Play previous / next song |
| `[` / `]` | Seek back / forward 5s |

## Log Viewer

| Key | Action |
|-----|--------|
| `j` / `k` / `Up` / `Down` | Scroll down / up |
| `Ctrl-u` / `Ctrl-d` | Page up / page down |
| `Left` / `Right` | Reduce / increase shown level |
| `Shift+Left` / `Shift+Right` | Reduce / increase captured level |
| `t` | Toggle hide filtered |
| `f` | Toggle fullscreen |
| `h` / `l` | Focus selector / focus log |
| `H` | Toggle target selector |
| `g` then ... | Chord G |
| `G` | Last |

## Help Context

| Key | Action | Description |
|-----|--------|-------------|
| `Esc` | Help(Close) | Close help |
| `q` | Help(Close) | Close help |
| `?` | Help(Close) | Close help |
