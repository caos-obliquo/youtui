---
name: Bug report - crates
about: Create a report to help us improve the crates
title: '[Bug report]: '
labels: ['triage', 'api']
assignees: caos-obliquo

---

**Describe the bug**
A clear and concise description of what the bug is.

**To Reproduce**
Steps to reproduce the behavior.

**Expected behavior**
A clear and concise description of what you expected to happen.

**If ytmapi-rs: Provide the source json file that caused the error**
This can be returned using the `youtui` command line app with the `--show-source` flag plus `--input-json PATH`.
E.g, error was returned when searching for 'The Beatles'. Provide the output of `youtui search "The Beatles" --show-source`

**If ytmapi-rs: Provide a screenshot of the YouTube Music page that shows the result you expected**
E.g, error was returned when searching for 'The Beatles'. Provide a screenshot of your YouTube Music searching for 'The Beatles'.
Please provide as close to a full page screenshot as possible, without including identifying information.

**Environment (please complete the following information):**
 - OS/distro: [e.g. macOS / fedora]
 - Crate: [e.g ytmapi-rs]
 - Version [e.g. 0.3.3 or git tag f8e24a5]
 - ytmapi-rs version/commit: [e.g. output of `cargo pkgid -p ytmapi-rs` or git rev-parse]
 - Cookie/auth setup: [e.g. browser cookie file at ~/.config/youtui/cookie.txt, fresh or expired; or OAuth]
 - Exact CLI command plus its JSON output: [e.g. `youtui search "The Beatles" --show-source` with output attached]
 - Fixture file: [attach the saved JSON from `--input-json` if applicable]

**Additional context**
Add any other context about the problem here.
