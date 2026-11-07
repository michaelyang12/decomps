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

Until this is done, TotK and both SSBU tiles show placeholder covers (libretro-thumbnails has no Switch art), and no game has a logo.

- [ ] Get a free API key: steamgriddb.com > log in > Preferences > API.
- [ ] Save it where only the launcher reads it. `.env` is git-ignored:

  ```sh
  echo STEAMGRIDDB_API_KEY=<key> > /mnt/c/Users/micha/Decomps/.env
  ```

- [ ] Press the launcher's refresh button, or run `decomps-launcher.exe --fetch-art`. Check `art/cache/fetch.log`.
- [ ] Pin each game's SteamGridDB id in `shortcuts.json` (`"art": {"steamgriddb": <id>}`) so a title search can't drift to the wrong game. The ids it matched are in `art/cache/<title-slug>/meta.json` (`steamgriddb_id`); confirm each one on steamgriddb.com.
- [ ] Look over the chosen art; replace any you dislike by dropping files into `art/custom/<title-slug>/` (the tile's **Custom art** button).

## Ideas

- An in-app editor for an entry (title, platform, picking among the scan's target candidates), so new games never need a hand edit of `shortcuts.json`.
- An **Add library folder** button that appends to `scan`.
