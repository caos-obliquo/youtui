# Crate: vi-text-editor

**2,648 LOC, 1 file** — Standalone vim-mode text editor widget for Ratatui TUI.

## Modes

| Mode | Enter | Exit | Behavior |
|------|-------|------|----------|
| `Insert` | `i`/`a`/`I`/`A`/`s`/`S` | `Esc` | Typed text inserted at cursor |
| `Normal` | `Esc` from Insert | key to enter other modes | Navigation, operators, motions |
| `VisualLine` | `V` | `Esc`/`V`/`d`/`y` | Select full lines, `d` delete, `y` yank |
| `VisualChar` | `v` | `Esc`/`v`/`d`/`y`/`c` | Select character range |
| `OperatorPending` | `d`/`c`/`y`/`r`/`f`/`F`/`t`/`T` | motion or char | Awaiting motion/target |
| `TextObjectPending` | `i`/`a` from OP | text object char | Awaiting `w`/`(`/`"`/`'`/`` ` `` |
| `SurroundAddPending` | `ys` | motion + char | Awaiting range + surround char |
| `SurroundTargetChar` | `ds`/`cs` | surround char | Awaiting target char |
| `Search` | `/`/`?` | `Enter`/`Esc` | Type search query, Enter to find, Esc cancel |

## Motions

| Key | Motion | Scope |
|-----|--------|-------|
| `h`/`Left` | Char left | All modes |
| `l`/`Right` | Char right | All modes |
| `j`/`Down` | Line down (multiline) or history forward (single-line) | Normal |
| `k`/`Up` | Line up (multiline) or history back (single-line) | Normal |
| `w` | Word forward (next word start) | Normal, visual |
| `W` | BIG-word forward (whitespace-delimited) | Normal, visual |
| `b` | Word back | Normal, visual |
| `B` | BIG-word back | Normal, visual |
| `e` | Word end | Normal, visual |
| `E` | BIG-word end | Normal, visual |
| `0`/`Home` | Line start | Normal, visual |
| `$`/`End` | Line end | Normal, visual |
| `^` | Line first non-whitespace | Normal, visual |
| `gg` | First line | Normal, visual |
| `G` | Last line | Normal, visual |
| `%` | Matching bracket `()[]{}` | Normal |
| `/`{query} | Search forward | Normal (Search mode) |
| `?`{query} | Search backward | Normal (Search mode) |
| `n` | Repeat last search forward | Normal |
| `N` | Repeat last search backward | Normal |
| `f`{char} | Find char forward | Normal |
| `F`{char} | Find char back | Normal |
| `t`{char} | Till char forward | Normal |
| `T`{char} | Till char back | Normal |
| `;` | Repeat last `f`/`F`/`t`/`T` | Normal |
| `,` | Reverse last `f`/`F`/`t`/`T` | Normal |

## Operators

| Sequence | Action | Clipboard | Mode |
|----------|--------|-----------|------|
| `x` | Delete char | Yes | Normal |
| `dd` | Delete line | Yes | OP |
| `dw` | Delete word forward | Yes | OP |
| `dW` | Delete BIG-word forward | Yes | OP |
| `dB` | Delete BIG-word back | Yes | OP |
| `dE` | Delete to end of BIG-word | Yes | OP |
| `d$` | Delete to line end | Yes | OP |
| `dh` | Delete left char | Yes | OP |
| `dl` | Delete right char | Yes | OP |
| `D` | Delete to line end (synonym `d$`) | Yes | Normal |
| `diw` | Delete inner word | Yes | TO |
| `daw` | Delete a word (incl. trailing space) | Yes | TO |
| `di(`/`da(` | Delete inside/a parens | Yes | TO |
| `di"/da"` | Delete inside/a quotes | Yes | TO |
| `di'/da'` | Delete inside/a single quotes | Yes | TO |
| `` di`/da` `` | Delete inside/a backtick | Yes | TO |
| `c` | Enter operator-pending for change | Yes | Normal |
| `cw` | Change word forward | Yes | OP→Insert |
| `cW` | Change BIG-word forward | Yes | OP→Insert |
| `c$` | Change to line end | Yes | OP→Insert |
| `cc` | Change line | Yes | OP→Insert |
| `C` | Change to line end (synonym `c$`) | Yes | Normal |
| `yy` | Yank line | Yes | OP |
| `Y` | Yank line (synonym `yy`) | Yes | Normal |
| `yw`/`yW` | Yank word/BIG-word forward | Yes | OP |
| `y$` | Yank to line end | Yes | OP |
| `p` | Paste after cursor | No | Normal |
| `P` | Paste before cursor | No | Normal |
| `r`{char} | Replace char | No | OP |
| `~` | Toggle case | Yes | Normal |
| `J` | Join lines (multiline) | No | Normal |
| `s` | Delete char + insert (synonym `cl`) | Yes | Normal→Insert |
| `S` | Delete line + insert (synonym `cc`) | Yes | Normal→Insert |

## Surround

| Sequence | Action |
|----------|--------|
| `ds`{char} | Delete surrounding pair (`ds(` removes `(...)`) |
| `cs`{from}{to} | Change surrounding (`cs'"` changes `'...'` → `"..."`) |
| `ys`{motion}{char} | Add surround to motion range |
| `ysiw(` | Wrap current word in `()` |
| `yss(` | Wrap entire line in `()` |

## Switch

| Key | Action |
|-----|--------|
| `Ctrl-a` | Increment number under cursor |
| `Ctrl-x` | Decrement number under cursor |

## Text Objects

| Sequence | Selects |
|----------|---------|
| `iw` | Inner word (no trailing spaces) |
| `aw` | A word (includes trailing spaces) |
| `i(` / `i)` | Inner parens (excludes delimiters) |
| `a(` / `a)` | A paren block (includes delimiters) |
| `i"` | Inner double quotes |
| `a"` | A double-quoted string (includes quotes) |
| `i'` | Inner single quotes |
| `a'` | A single-quoted string (includes quotes) |
| `` i` `` | Inner backtick |
| `` a` `` | A backtick-quoted string (includes backticks) |

## Repeat

| Key | Action |
|-----|--------|
| `.` | Repeat last change (insert, delete, change, replace, paste) |
| `u` | Undo (50-entry stack) |
| `Ctrl-r` | Redo (50-entry stack) |

## Config Editor Usage

Multiline mode (`ViTextEditor::new_multiline()`):
- `Enter` inserts newline
- `j`/`k` move between lines
- `gg`/`G` go to first/last line
- `J` joins current line with next

Single-line mode (`ViTextEditor::new()`, default):
- `Enter` submits text (returns `true` from `handle_key`)
- `j`/`k` navigate command history
- No multiline operations

## UTF-8 Safety

Cursor is a byte index. Before every `handle_key` call, `clamp_cursor()` ensures cursor is on a valid UTF-8 char boundary. Operates on ASCII words (space-delimited) rather than unicode grapheme clusters — intentional suckless design.

## Tests

```bash
cargo test --release -p vi-text-editor
# 67 tests pass
```
