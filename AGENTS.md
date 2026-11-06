# AGENTS.md

Personal workspace of Nintendo game decompilations, static recompilations and PC ports, played on Windows. This root repo tracks only workspace glue (this file, `.gitignore`, `make-shortcuts.ps1`). Each game is its own git repo under `Games/`, ignored here.

## Layout

| Path | What |
|---|---|
| `Games/sm64ex-alo/` | Super Mario 64 PC port (fork of sm64ex; upstream `AloUltraExt/sm64ex-alo`). No agent docs; see its `README.md`. |
| `Games/sms-pc-port/` | Super Mario Sunshine PC port from the matching decomp. Own `AGENTS.md`. |
| `Games/ssbu-recomp-main/ssbu-recomp-main/` | Super Smash Bros. Ultimate static recomp (note the doubled folder). Own `CLAUDE.md` and `.claude/skills/`. |
| `Games/totk-recomp/` | Prebuilt Tears of the Kingdom recomp install; no source. Own `AGENTS.md`. |
| `exes/` | Windows `.lnk` launchers for every game (git-ignored, generated). |
| `make-shortcuts.ps1` | Regenerates `exes/`. |

Also launched from `exes/`, but installed outside this folder: Nebula and SM64DS Port under `%LOCALAPPDATA%\Programs\`.

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

## Launchers

- Regenerate after a game moves, a build output path changes, or Nebula updates:
  `powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'C:\Users\micha\Decomps\make-shortcuts.ps1'`.
  It prints `MISSING` for any target that doesn't exist yet.
- To add a game, add a row to `$items` in `make-shortcuts.ps1`. Shortcuts start in the target's own folder because the games load data relative to it.
- SSBU launches through `Play SSBU.bat` and `Play SSBU (modded).bat`, not an exe.
- Sunshine launches `sms-standalone.exe` (game data embedded), not `sms.exe` (which needs the disc image passed in).

## Git

Root repo: no remote. Agents may commit here; never push. Short imperative subjects. Never stage anything under `Games/` or `exes/`.
