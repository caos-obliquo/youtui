# Subsystem: Lyrics

## Pipeline Order

```
Priority 0: Genius JSON API ← preferred (full lyrics, no auth)
Priority 1: Musixmatch ← may return partial (free tier)
Priority 2: Bandcamp ← constructed URL → bandcamp-lyrics CLI
Priority 3: lyr CLI ← last resort, multiple artist variants
Priority 4: Error("No lyrics found from any provider")
```

## Implementation

File: `app/server/messages.rs:502-705` — `GetLyrics` backend task.

Called from: `effect_handlers_playlist.rs` when `ViewLyrics` callback fires.

```rust
struct GetLyrics(String, String, String);
// artist, title, genius_token
```

## Genius JSON API (Priority 0)

**Replaced HTML scraping in 2026-06-21.** No more fragile `split("data-lyrics-container")`.

### Flow

```
1. Search with Bearer token (if available):
   GET https://api.genius.com/search?q={artist}+{title}
   Header: Authorization: Bearer {token}
   → parse /response/hits/0/result/id

2. Public search (no auth needed):
   GET https://genius.com/api/search/song?q={title}+{artist}
   → parse /response/sections/0/hits/0/result/id

3. Fetch lyrics JSON:
   GET https://genius.com/api/songs/{id}/lyrics
   → parse /response/lyrics/lyrics/body/html
```

### Cleaning Steps

```rust
1. Replace <br>/<br/> with \n
2. Strip remaining HTML tags (annotation links <a>)
3. Decode entities: &quot;, &#x27;, &amp;, &lt;, &gt;
4. Filter lines: trim, drop empty, drop "Contributors"/"You might also like"
5. Merge orphaned parenthetical lines
6. Quality gate: >50 chars AND >2 lines
```

### Section Markers

`[Verse 1]`, `[Chorus]`, `[Bridge]` are plain text in Genius HTML — preserved automatically.

## Musixmatch (Priority 1)

```rust
Musixmatch::builder().build()?
    .matcher_lyrics(&title, &artist).await?
    .lyrics_body
```

Free tier returns partial lyrics (~16 lines, chorus+bridge only). Full lyrics require API key.

## Bandcamp (Priority 2)

Constructs URL from artist/title slugs, calls external `bandcamp-lyrics` CLI:

```
https://{artist_slug}.bandcamp.com/track/{song_slug}
```

Tries these suffixes for the artist slug: `""`, `"-2"`, `"-3"`, `"-4"`, `"-5"`.

## lyr CLI (Priority 3)

External `lyr` command, tries multiple artist/title normalization variants:

- `(artist, title)` — original
- `(first_artist, title)` — split on comma
- `(two_artists, title)` — first two split by "and"
- `(first_artist, norm_title)` — normalized
- `(norm_artist, title)` — normalized
- `(norm_artist, norm_title)` — both normalized

Normalization: lowercase, collapse whitespace.

## Rendering

File: `app/ui/playlist/lyrics_popup.rs` — `LyricsPopup` struct.

### Layout

```
┌─── Lyrics ───────────────────────┬─── Annotations (a: N) ──────┐
│  0 Lyric line                    │ [1] ── fragment             │
│ +1 Next line                     │     explanation text        │
│ +2 Next line                     │     wraps multiple lines    │
│ ...                              │                              │
│                                  │ [2] ── next fragment        │
│                                  │     explanation             │
│                                  │                              │
│   ~                              │     ~                        │
│                                  │                              │
├──────────────────────────────────┴──────────────────────────────┤
│ Esc/q: Close | a: Toggle annot  | Tab/l/h: Switch panel focus  │
└─────────────────────────────────────────────────────────────────┘
```

When `a` is pressed and annotations exist, the view splits 55/45 between lyrics and annotations. `Tab`/`l`/`h` switches focus between panels.

### Annotations Panel (Separate Component)

Annotations panel has its own cursor state and vim navigation independent of lyrics:

| Key | Action |
|-----|--------|
| `j`/`Down`/`J` | Scroll annotation text down |
| `k`/`Up`/`K` | Scroll annotation text up |
| `g` | Jump to first annotation |
| `G` | Jump to last annotation |
| `0` | Line start within annotation |
| `$` | Line end within annotation |
| `w`/`W`/`b`/`B`/`e`/`E` | Word motions within annotation |
| `v` | Enter visual mode in annotation |
| `y` | Yank annotation text to clipboard |
| `Enter` | Open annotation URL (if linked) |

Annotations display their index and reference count: `[N annotations for this line]`.

### Component Isolation (Critical)

Lyrics and annotations are **separate interactive components** that communicate via shared state. When either panel is focused, the other panel's commands are disabled:

- **Annotations focused**: `Enter` opens URL, `(`/`)`/`<`/`>` seek commands **do not work**. Only annotation-scoped commands available.
- **Lyrics focused**: `(`/`)` seek queue, `<`/`>` seek position, `Enter` seeks timestamp. Annotation commands disabled.

This prevents accidental seeks while reading annotations. Toggle `a` to close annotations, focus returns to lyrics.

### Relative Line Numbers

Both panels display relative line numbers:

- **Lyrics**: `0` for current line, `+N`/`-N` for lines above/below
- **Annotations**: `[N]` for annotation index within the popup

### Navigation

#### Normal Mode

| Key | Action |
|-----|--------|
| `j`/`Down`/`J` | Move cursor down |
| `k`/`Up`/`K` | Move cursor up |
| `H`/`Left` | Move cursor left within line |
| `L`/`Right` | Move cursor right within line |
| `g` | Jump to first line |
| `G` | Jump to last line |
| `w`/`W` | Next word start |
| `b`/`B` | Previous word start |
| `e`/`E` | Next word end |
| `0` | Line start |
| `$` | Line end |
| `Ctrl+d` | Page down (n × 10 lines) |
| `Ctrl+u` | Page up (n × 10 lines) |
| `(`/`)` | View previous/next in queue |
| `<`/`>` | Seek backward/forward |
| `[`/`]` | Seek backward/forward larger |
| `}` | Next paragraph (double newline) |
| `{` | Previous paragraph |

#### Visual Mode

Enter: `V`. Exit: `Esc` or `V`.

| Key | Action |
|-----|--------|
| `j`/`Down`/`J` | Extend selection down |
| `k`/`Up`/`K` | Extend selection up |
| `H`/`Left` | Move cursor left within line |
| `L`/`Right` | Move cursor right within line |
| `g` | Jump selection start to first line |
| `G` | Jump selection end to last line |
| `0` | Line start |
| `$` | Line end |
| `w`/`W` | Next word start |
| `b`/`B` | Previous word start |
| `e`/`E` | Next word end |
| `Ctrl+d` | Page down selection (n × 10) |
| `Ctrl+u` | Page up selection (n × 10) |
| `y` | Yank selection to clipboard (`wl-copy`) |

### Enter Seeks Timestamp

If a lyric line starts with `[m:ss]` or `[mm:ss]`, pressing `Enter` seeks to that absolute position.

## Known Issues

- Free Musixmatch returns partial lyrics (quality gate: >50 chars and >3 lines — too lenient for detecting partial)
- Annotation panel: last annotation entry may be cut off by popup height
