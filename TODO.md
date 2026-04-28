# TODO.md - youtui Optimization & Missing Features

### 🔴 High Priority

#### 1. Fix Thumbnail Notifications for Saved Playlist Songs
- **Problem:** Mako notifications appear without thumbnails because the notification is sent before the thumbnail fetch completes.
- **Fix:**
  - Locate the "song saved" notification code path (likely `app/ui/playlist.rs` or notification helper).
  - Add async await on `prefetch_thumbnails_for_indices` OR use a callback that triggers the notification only after thumbnail `Result::Ok`.
  - Fallback to a default placeholder image if thumbnail fetch fails (never send empty notification).
- **Validation:** Manually save a song and confirm the image appears in Mako.

#### 2. Sync Download Queue on Shuffle Toggle
- **Problem:** Toggling shuffle (`S` key) does not rebuild the `download_queue`. The pre-buffer still reflects the old order.
- **Fix:**
  - Locate `handle_action` for `PlaylistAction::ToggleShuffle` in `playlist.rs`.
  - After `shuffle_enabled` flag changes, call a helper to rebuild `download_queue` based on the new visual order.
  - Reuse the "skip already downloaded" logic from `download_upcoming_from_id`.
  - Edge case: If a download is in progress, keep it running but update the remaining queue.
- **Validation:** Toggle shuffle while a song plays; verify next download follows shuffled order.

---

### 🟡 Medium Priority

#### 3. UI Icon Polish (Non-ASCII → ASCII)
- Replace any remaining Unicode block characters (e.g., `▸`, `⏵`) with plain ASCII (`>`, `v`) for universal terminal rendering.
- Location: `draw_media_controls.rs`, `footer.rs`, and any widget files.

#### 4. Clean Up Dead Code in Footer
- Remove any remaining dead-code branches for the old progress-percentage display in `footer.rs`.
- The code path is gated by a feature flag; delete it entirely.

#### 5. Update Tests for AudioQuality Default
- Add a test: `assert_eq!(AudioQuality::default(), AudioQuality::Low)`.
- In tests that instantiate a downloader with `AudioQuality::default()`, assert that the format string contains `bestaudio[ext=m4a]` with the low-quality variant.
- Location: `youtui/src/app/structures.rs` (test module) and `yt_dlp.rs` tests.

---

### 🟢 Low Priority / Optional

#### 6. Dynamic Download Concurrency
- Track average download time of the last N songs.
- If average > 6s, reduce semaphore count by 1; otherwise increase up to a hard max of 4.
- Expose `MAX_CONCURRENT_DOWNLOADS` as an env var override (e.g., `YOUTUI_MAX_DOWNLOADS`).

#### 7. Resolve Thumbnail Notification Inconsistencies (OS-level)
- Investigate `notify-rust` library behavior with different notification daemons (Mako, Dunst, etc.).
- Ensure the notification timeout is sufficient for the image to load.

---

### ✅ Completed This Session
- [x] Fix playback latency (removed filesize check)
- [x] Audio quality runtime adjustment ('A' key)
- [x] Set default quality to Low
- [x] Fix download queue (skip downloaded, sequential prefetch)
- [x] Fix BA decoder error (forced M4A compatibility)
- [x] ASCII status icons
- [x] Remove progress percentage spam
- [x] Add detailed logging for queue debugging
- [x] Document, commit, and push (`f86fa38`)