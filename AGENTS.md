# AGENTS.md

Personal workspace of Nintendo game decompilations, static recompilations and PC ports, played on Windows. This root repo tracks only workspace glue: this file, `.gitignore`, `shortcuts.json`, `launcher/`, `tools/` and `.claude/skills/`. Each game is its own folder under `Games/` (usually its own git repo), ignored here.

## Layout

| Path | What |
|---|---|
| `Games/sm64ex-alo/` | Super Mario 64 PC port (fork of sm64ex; upstream `AloUltraExt/sm64ex-alo`). No agent docs; see its `README.md`. |
| `Games/sms-pc-port/` | Super Mario Sunshine PC port from the matching decomp. Own `AGENTS.md`. |
| `Games/ssbu-recomp-main/ssbu-recomp-main/` | Super Smash Bros. Ultimate static recomp (note the doubled folder). Own `CLAUDE.md` and `.claude/skills/`. |
| `Games/totk-recomp/` | Prebuilt Tears of the Kingdom recomp install; no source. Own `AGENTS.md`. |
| `Games/Nebula/` | Installed Nebula recomp app (installer-managed, not a repo; holds a 5 GB toolchain). |
| `Games/SM64DS Port/` | Installed SM64DS Port launcher (installer-managed, not a repo; owns the `.tang` mod file type). |
| `exes/` | Windows `.lnk` launchers for every game (git-ignored, generated). |
| `shortcuts.json` | The game list: what each launcher in `exes/` points at, plus title, platform and art lookups for `launcher/`. |
| `launcher/` | Decomps Launcher, a Tauri desktop app (Rust + plain HTML/JS) that shows every game with art and launches it. |
| `art/` | Launcher art: `art/cache/` (downloaded) and `art/custom/` (hand-placed overrides). Git-ignored. |
| `.env` | API keys for the launcher (`STEAMGRIDDB_API_KEY`). Git-ignored. |
| `tools/sync_shortcuts.py` | Syncs `exes/`, and repairs Start Menu and Desktop shortcuts, from `shortcuts.json`. |

## Working on a game

- `cd` into the game and read its `AGENTS.md`/`CLAUDE.md` first. Its rules override this file.
- Commit inside the game's repo, never here. Each one has its own remote and branch conventions.
- Game data (ROMs, disc images, NSP/XCI dumps, `baserom.*`, `rom/`, `build/dump`) is the owner's copy. Never commit, upload or copy it out of the workspace.

## Environment

Sessions run in WSL, but the games and their builds are Windows programs.

- Call Windows tools with Windows paths: `powershell.exe -NoProfile -Command ...`, `cmd.exe /c ...`.
- MSYS2 MINGW64 lives at `C:\msys64`. Build sm64ex-alo from WSL with:

  ```sh
  MSYSTEM=MINGW64 CHERE_INVOKING=1 WSLENV=MSYSTEM:CHERE_INVOKING /mnt/c/msys64/usr/bin/bash.exe -lc \
    'cd /c/Users/micha/Decomps/Games/sm64ex-alo && make -j$(nproc) HIGH_FPS_PC=1 NODRAWINGDISTANCE=1 TEXTSAVES=1 EXTERNAL_DATA=1'
  ```

  This produces `build/us_pc/sm64.us.f3dex2e.exe`. Those flags were used for the current build. `EXTERNAL_DATA=1` means `res/` must sit next to the exe.
- Before moving or renaming a game folder, check what is using it:
  - Windows refuses (`Permission denied`) while the game, or any file in its folder, is open. Run `Get-Process | ? Path -like '*Decomps*'`.
  - If a WSL process (such as another agent session) has its working directory in the folder, WSL shows the moved folder as `d?????????` until that process exits. The files are fine.
  - Claude Code keys session history by path (`~/.claude/projects/-mnt-c-...`), so a moved folder starts with no history.
  - Nebula and SM64DS Port are registered installs. Moving either means updating its `HKCU\...\Uninstall\{Nebula,SM64DSPort}` values (`InstallLocation`, `UninstallString`, `DisplayIcon`). Nebula also needs `HKCU\Software\Nebula\InstallRoot`; SM64DS needs `HKCU\Software\Classes\SM64DSLauncher.Mod` (`DefaultIcon`, `shell\open\command`).
  - Then sync shortcuts (below).

## Manifest (`shortcuts.json`)

One entry per game tile and per `exes/` launcher. Adding a game means adding one entry, then building the game and syncing shortcuts (below):

```json
{ "name": "Super Mario Galaxy", "target": "Games/Nebula/versions/*/Nebula.exe",
  "title": "Super Mario Galaxy", "platform": "Wii", "port": "Nebula static recomp",
  "art": { "libretro": "Super Mario Galaxy (USA)" } }
```

- `name` (required): the `exes/<name>.lnk` name and the tile's stable id. Renaming it drops the tile's play history.
- `target` (required): relative to this root. A `*` glob picks the highest natural-sorted match.
- `title`: display name (default `name`). Tiles with the same title share art, and later ones are badged with the last comma-separated part of `port` (this is how `Play SSBU (modded)` is shown).
- `platform`: original console (`Nintendo 64`, `GameCube`, `Wii`, `Nintendo DS`, `Switch`). Shown as a label, and picks the libretro-thumbnails system (`Nintendo - <platform>`; override with `art.libretroSystem`).
- `port`: short description of the build, e.g. `Static recomp`.
- `art.libretro`: the No-Intro name as listed on thumbnails.libretro.com, e.g. `Super Mario 64 (USA)`. Omit it if the system isn't covered (Switch isn't).
- `art.steamgriddb`: SteamGridDB game id (number, preferred once known) or search term (string). Default: search by `title`.

The list is flat, with no grouping by console: everything here runs as a native Windows program, so `platform` is only a label and an art lookup key.

## Launcher

`launcher/` builds `decomps-launcher.exe`: a library of every manifest entry with cover art, a hero backdrop and logo for the selected game, last-played and playtime. Windowed by default; F11 or Start on a controller toggles a fullscreen couch mode, which is fully gamepad-driven (A play, X open folder, Y refresh art, D-pad/stick to move, LB/RB jump to the ends). Each game starts with its working directory set to the target's folder, the same as the `.lnk` files.

- **Build** (Windows; Rust via rustup with the MSVC toolchain and VS 2022 Build Tools):

  ```sh
  powershell.exe -NoProfile -Command 'cd C:\Users\micha\Decomps\launcher; cargo build --release'
  ```

  This produces `launcher/target/release/decomps-launcher.exe`, a single exe with the UI embedded. Build from a Windows path: for a worktree under WSL's `/home`, mirror `launcher/` to a Windows folder first, since cargo and MSVC aren't reliable on `\\wsl$` paths. `cargo test` runs the manifest unit tests.
- **Run:** the exe finds the Decomps root by `--root <dir>`, else `DECOMPS_ROOT`, else the nearest parent folder holding `shortcuts.json` and `Games/`, else `%USERPROFILE%\Decomps`. `--fullscreen` starts in couch mode. `--fetch-art` refetches all art headlessly and exits.
- **Art:** fetched in the background on start, for games with empty slots (retried weekly, or every game on Refresh art). Slots are `cover` (2:3 box art), `hero` (wide backdrop) and `logo`. Sources, in order:
  1. SteamGridDB (cover, hero, logo). Needs `STEAMGRIDDB_API_KEY=...` in `.env` here; a free key comes from steamgriddb.com > Preferences > API.
  2. libretro-thumbnails (`Named_Boxarts` for the cover, `Named_Snaps` for the hero). No key needed, but no Switch and no logos.

  Gaps show a generated placeholder tile. To override any slot by hand, put `cover.png`, `hero.png` or `logo.png` (jpg and webp also work) in `art/custom/<title-slug>/`; the tile's **Custom art** button opens that folder. Results of the last fetch are in `art/cache/fetch.log`. Only titles and ids are sent to these services, never game data.
- **State:** play history and the fullscreen choice live in `%LOCALAPPDATA%\local.decomps.launcher\state.json`.
- A launch whose process exits within 15 seconds counts as a hand-off to another process (a launcher stub), so it records no playtime and fullscreen mode doesn't pop back over the game.

## Shortcuts

After moving or renaming a game folder, adding a game, changing a build output path, or a Nebula update, use the `sync-shortcuts` skill (`.claude/skills/sync-shortcuts/SKILL.md`). In short:

```sh
python3 tools/sync_shortcuts.py --check --external   # preview; exits 1 if anything would change
python3 tools/sync_shortcuts.py --external           # apply: exes/ + Start Menu/Desktop repairs
```

- To add a game, add an entry to `shortcuts.json` (`target` relative to this root; a `*` glob picks the newest match).
- `--external` only rewrites shortcuts whose target is missing. Other apps' shortcuts that happen to live under `AppData\Local\Programs` (such as Zed) must never be repointed.
- SSBU launches through `Play SSBU.bat` and `Play SSBU (modded).bat`, not an exe.
- Sunshine launches `sms-standalone.exe` (game data embedded), not `sms.exe` (which needs the disc image passed in).

## Git

Root repo: no remote. Agents may commit here; never push. Short imperative subjects. Never stage anything under `Games/` or `exes/`.
