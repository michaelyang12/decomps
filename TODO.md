# TODO

## Land the launcher

- [ ] Merge `feature/decomp-frontend` into `main`. (The original handoff said `master`, but this repo's branch is `main`.)
- [ ] Build it on Windows. Open a new terminal first so the rustup PATH change applies:

  ```powershell
  cd C:\Users\micha\Decomps\launcher
  cargo build --release
  ```

  Output: `launcher\target\release\decomps-launcher.exe`. Rust (rustup, MSVC toolchain) and VS 2022 Build Tools are already installed.
- [ ] Check shortcuts still sync: `python3 tools/sync_shortcuts.py --check --external` should print only `ok` lines.
- [ ] Optional: Start Menu shortcuts for the launcher itself, one windowed and one with `--fullscreen` for couch mode.

## SteamGridDB art

Done 2026-11-07: the key is in `.env` (git-ignored, and in `.git/info/exclude` until the merge brings the `.gitignore` rule). All 6 games matched the right SteamGridDB game, and their ids are pinned in `shortcuts.json`. Art for every game is already in `art/cache/`.

- [ ] Look over the chosen art; replace any you dislike by dropping files into `art/custom/<title-slug>/` (the tile's **Custom art** button).

## Ideas

- An in-app editor for an entry (title, platform, picking among the scan's target candidates), so new games never need a hand edit of `shortcuts.json`.
- An **Add library folder** button that appends to `scan`.
