# youtui

Vim-style YouTube Music player for your terminal.

---

## Requirements

- **Rust** (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- **yt-dlp** (`sudo pacman -S yt-dlp` or `brew install yt-dlp`)
- **alsa-lib** (Linux: `sudo pacman -S alsa-lib` or `sudo apt install libasound2-dev`)
- A **YouTube Music account** (free Google account)

---

## Install & Run (5 minutes)

### Step 1: Install youtui

```sh
git clone https://github.com/caos-obliquo/youtui
cd youtui
cargo build --release
cargo install --path youtui --force
```

### Step 2: Get your YouTube cookie

1. Open **Chrome** (or Firefox) and go to `music.youtube.com`
2. **Log in** with your Google account
3. Press **F12** to open DevTools
4. Click the **Network** tab
5. **Reload the page** (F5)
6. In the filter box, type **`music`**
7. Click any item in the list (a POST request to `music.youtube.com`)
8. Scroll down in the right panel to **Request Headers**
9. Find the **`Cookie:`** line - it's a VERY long string
10. Right-click it → **Copy value**
11. Run these commands:

```sh
mkdir -p ~/.config/youtui
echo 'COOKIE_VALUE_HERE' > ~/.config/youtui/cookie.txt
```

Replace `COOKIE_VALUE_HERE` with the actual cookie you copied.

### Step 3: Run

```sh
youtui
```

Press `1` to see your playlist, `j`/`k` to move up/down, `Enter` to play.

---

## How to use

| Key | What it does |
|---|---|
| `1` | Your playlist |
| `2` | Search songs |
| `3` | Browse artists |
| `4` | Browse playlists |
| `j`/`k` | Move up/down |
| `Enter` | Play selected song |
| `Space` | Play/Pause |
| `y` | Show lyrics |
| `q` | Quit (press `y` to confirm) |
| `?` | Show all keybinds |

That's it. No OAuth, no API keys, no Google Cloud — just a browser cookie.

---

## Config file

`~/.config/youtui/config.toml` (optional):

```toml
auth_type = "Browser"
downloader_type = "YtDlp"
yt_dlp_command = "yt-dlp"
```

Press `C-e` (Ctrl+e) inside youtui to edit this file.

---

## Features

- **Vim keys** — j/k/h/l, C-b/C-f, g/G, no F-keys
- **Lyrics** — press `y`, fetches from Genius + Musixmatch
- **Scrobble** — add `[scrobbling]` section to config
- **Dark Souls quit** — `q` shows YOU DIED
- **Copy URL** — `C-y` copies song link
- **Open URL** — `:` paste a YouTube Music link

---

## Build & Test

```sh
cargo build --release
./target/release/youtui

cargo test --bins --lib
```

---

## License

MIT
