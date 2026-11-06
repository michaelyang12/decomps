---
name: sync-shortcuts
description: Update the Windows game shortcuts in Decomps (exes/ launchers, Start Menu and Desktop .lnk files). Use after moving or renaming a game folder, adding a game, changing a build's output path, a launcher update (e.g. new Nebula version), or when the user says a shortcut is broken or asks to add/update shortcuts.
---

# Sync shortcuts

`tools/sync_shortcuts.py` (Python 3, stdlib only; works from WSL) keeps `.lnk` files in step with `shortcuts.json`.

1. Preview from the Decomps root. It changes nothing and exits 1 if anything would change:

   ```sh
   python3 tools/sync_shortcuts.py --check --external
   ```

2. Read the report:
   - `create`/`update`: the `exes/` launcher is missing or points somewhere else.
   - `missing`: the target doesn't exist and no single file with that name exists under `Games/`. The game isn't built yet, or the path changed. Fix the `target` in `shortcuts.json`.
   - `moved`: the target was found by name elsewhere under `Games/`. On a real run it is written back to `shortcuts.json`.
   - `ambiguous`: several files have that name. Set the right `target` in `shortcuts.json` by hand.
   - `unlisted Games/X`: a game folder has no shortcut. Add an entry, asking the user which file launches it if that isn't obvious.
   - `repair`: a Start Menu or Desktop shortcut with a missing target was matched to a file under `Games/`.
   - `broken`: same as `repair`, but no match was found. Report it to the user.
   - `extra`: a `.lnk` in `exes/` that isn't in the manifest. Delete it only with `--prune`, if the user wants that.

3. Apply with the same flags minus `--check`. Then run `--check --external` again; it should print only `ok` lines.

4. If `shortcuts.json` changed, commit it in the root repo.

## Manifest rules (`shortcuts.json`)

- `target` is relative to the Decomps root. A `*` glob picks the highest natural-sorted match; use one where a version folder changes, as in `Games/Nebula/versions/*/Nebula.exe`.
- Point at the file the user actually launches. For example, SSBU uses `Play SSBU.bat` and `Play SSBU (modded).bat`, and Sunshine uses `sms-standalone.exe`.
- The `name` becomes `exes/<name>.lnk`.

## What it does not do

It doesn't touch registry entries such as uninstall keys, file associations or `HKCU\Software\Nebula\InstallRoot`. If an installed app's folder moves, update those separately; see `AGENTS.md`.
