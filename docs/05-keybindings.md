# Keybindings

All default keybindings by context. Customizable in `config.toml`.

## Global
| Key | Action |
|-----|--------|
| `Esc` `Esc` (double) | Close all search/filter/popups/help |

## Global Context

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
| `F7` | Browser(BrowserAction::ChangeSearchType) | Switch search tab |
| `F11` | ViewLogs | Show logs |
| `q` | Quit | Quit (with confirm) |
| `:` | OpenUrl | Open command prompt |

## Playlist Context

| Key | Action | Description |
|-----|--------|-------------|
| `j` | Playlist(PlaylistAction::Down) | Move down |
| `k` | Playlist(PlaylistAction::Up) | Move up |
| `J` | Playlist(PlaylistAction::ShiftDown) | Move song down in queue |
| `K` | Playlist(PlaylistAction::ShiftUp) | Move song up in queue |
| `Enter` | Playlist(PlaylistAction::PlaySelected) | Play selected song |
| `d` | Playlist(PlaylistAction::DeleteSelected) | Delete from queue |
| `o` | Playlist(PlaylistAction::OpenContextMenu) | Context menu |
| `V` | Playlist(PlaylistAction::ToggleVisualMode) | Visual mode |
| `u` | Playlist(PlaylistAction::UndoDelete) | Undo last delete |
| `/` | Playlist(PlaylistAction::LocalFilter) | Local fuzzy filter |
| `l` | NextSong | Next track |
| `h` | PrevSong | Previous track |

## Browser Context

| Key | Action | Description |
|-----|--------|-------------|
| `h`/`Left` | Browser(BrowserAction::PrevTab) | Previous search tab |
| `l`/`Right` | Browser(BrowserAction::NextTab) | Next search tab |
| `F1` | Browser(BrowserAction::Search) | Toggle YTM search |
| `F7` | Browser(BrowserAction::ChangeSearchType) | Cycle search tab |
| `/` | Browser(BrowserAction::LocalFilter) | Local fuzzy filter |
| `Backspace` | Browser(BrowserAction::Back) | Navigate back |
| `r` | Browser(BrowserAction::Reload) | Reload current category |

## Browser Context Menu (o mode)

| Key | Action | Description |
|-----|--------|-------------|
| `Enter` | BrowserSongs(PlaySong) | Play selected |
| `p` | BrowserSongs(PlaySongs) | Play all |
| `P` | BrowserSongs(AddSongsToPlaylist) | Queue all |
| `s` | BrowserSongs(AddSongToPlaylist) | Save to playlist |
| `N` | BrowserSongs(InsertNext) | Insert next in queue |
| `a` | BrowserSongs(GoToArtist) | Go to artist page |
| `b` | BrowserSongs(GoToAlbum) | Go to album page |
| `l` | BrowserSongs(ViewLyrics) | View lyrics |
| `y` | BrowserSongs(CopySongUrl) | Copy URL |
| `r` | BrowserSongs(GetRelatedTracks) | Get related tracks |
| `t` | BrowserSongs(RatePlaylist) | Rate playlist |
| `S` | BrowserSongs(ToggleSubscribeArtist) | Subscribe artist |
| `D` | BrowserSongs(DeletePlaylist) | Delete playlist |
| `R` | BrowserSongs(RenamePlaylist) | Rename playlist |
| `E` | BrowserSongs(EditPlaylistDetails) | Edit playlist details |
| `e` | BrowserSongs(OpenPlaylistEditor) | Open playlist editor |
| `i` | BrowserSongs(GetPlaylistDetails) | Playlist details popup |
| `x` | BrowserSongs(RemoveTrackFromPlaylist) | Remove track |
| `M` | BrowserSongs(MergePlaylist) | Merge playlists |
| `O` | BrowserLibrary(CycleSortOrder) | Cycle sort order |
| `c` | Filter(Close) | Close filter |
| `z` | Sort(Close) | Close sort popup |

## Browser Library Context

| Key | Action | Description |
|-----|--------|-------------|
| `Tab` | BrowserLibrary(NextCategory) | Next category |
| `Enter` | BrowserLibrary(FocusContent) | Focus content panel |
| `j`/`k` | Move within category/content | Navigation |
| `r` | BrowserLibrary(ReloadCategory) | Refresh category |
| `y` | BrowserSongs(CopySongUrl) | Copy URL |
| `/` | Browser(BrowserAction::LocalFilter) | Local filter |

## Sort Mode

| Key | Action | Description |
|-----|--------|-------------|
| `Enter` | Sort(SortSelectedAsc) | Sort ascending |
| `Alt-Enter` | Sort(SortSelectedDesc) | Sort descending |
| `Alt-4` | Sort(ClearSort) | Clear sort |
| `4` | Sort(Close) | Close sort popup |

## List Context

| Key | Action |
|-----|--------|
| `j`/`Down` | List(Down) |
| `k`/`Up` | List(Up) |
| `Ctrl-d` | List(PageDown) |
| `Ctrl-u` | List(PageUp) |
| `g` | List(First) |
| `G` | List(Last) |
| `Ctrl-n` | List(Down) |
| `Ctrl-p` | List(Up) |

## Text Entry Context

| Key | Action |
|-----|--------|
| `Enter` | TextEntry(Submit) |
| `Esc` | TextEntry(Submit) |
| `Left` | TextEntry(Left) |
| `Right` | TextEntry(Right) |
| `Backspace` | TextEntry(Backspace) |
| `Ctrl-w` | TextEntry(DeleteWord) |

## Lyrics Context

| Key | Action |
|-----|--------|
| `Esc`/`q` | Close popup |
| `j`/`Down`/`J` | Move/scroll down |
| `k`/`Up`/`K` | Move/scroll up |
| `H`/`Left` | Cursor left within line |
| `L`/`Right` | Cursor right within line |
| `g` | First line |
| `G` | Last line |
| `0` | Line start |
| `$` | Line end |
| `w`/`W` | Next word |
| `b`/`B` | Prev word |
| `e`/`E` | Word end |
| `Ctrl+d` | Page down |
| `Ctrl+u` | Page up |
| `a` | Toggle annotations |
| `R` | Toggle romaji |
| `Tab`/`l` | Next panel |
| `BackTab`/`h` | Prev panel |
| `V` | Enter visual mode |
| `y` | Yank selection (visual mode) |
| `Enter` | Seek timestamp |
| `/` | Filter mode |
| `}` | Next paragraph |
| `{` | Prev paragraph |

## Help Context

| Key | Action | Description |
|-----|--------|-------------|
| `Esc` | Help(Close) | Close help |
| `q` | Help(Close) | Close help |
| `?` | Help(Close) | Close help |
