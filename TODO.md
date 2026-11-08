# TODO

## Land the launcher

Done 2026-11-07: merged to `master` and built at `launcher\target\release\decomps-launcher.exe`. Start Menu and Desktop shortcuts called **Decomps Launcher** point at it, and `sync_shortcuts.py --check --external` prints only `ok`.

## SteamGridDB art

Done 2026-11-07: the key is in `.env` (git-ignored, and in `.git/info/exclude` until the merge brings the `.gitignore` rule). All 6 games matched the right SteamGridDB game, and their ids are pinned in `shortcuts.json`. Art for every game is already in `art/cache/`.

- [ ] Look over the chosen art; replace any you dislike by dropping files into `art/custom/<title-slug>/` (the tile's **Custom art** button).

## Ideas

- An in-app editor for an entry (title, platform, picking among the scan's target candidates), so new games never need a hand edit of `shortcuts.json`.
- An **Add library folder** button that appends to `scan`.
