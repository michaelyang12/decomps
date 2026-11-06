#!/usr/bin/env python3
"""Keep the workspace's Windows shortcuts pointing at the right files.

1. Resolves every entry in shortcuts.json (paths relative to the Decomps root,
   `*` globs allowed; the highest natural-sorted match wins, e.g. the newest
   Nebula version). A fixed path that no longer exists is searched for by file
   name under Games/; a single hit is used and written back to shortcuts.json.
2. Creates or updates exes/<name>.lnk for each entry (working directory = the
   target's folder, since the games load data relative to it).
3. Reports exes/*.lnk files not in the manifest (--prune deletes them) and
   Games/ folders that no shortcut covers.
4. With --external: repairs Start Menu and Desktop shortcuts whose target is
   missing but which can be matched to a file under Games/ (a moved folder or
   a manifest target with the same file name). Shortcuts whose target exists
   are never touched.

Runs from WSL (via powershell.exe) or native Windows Python. Standard library only.

  python3 tools/sync_shortcuts.py              # sync exes/
  python3 tools/sync_shortcuts.py --check      # report only; exit 1 if anything would change
  python3 tools/sync_shortcuts.py --external   # also repair Start Menu / Desktop shortcuts
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import re
import subprocess
import sys
from pathlib import Path, PurePosixPath, PureWindowsPath

ROOT = Path(__file__).resolve().parent.parent
GAMES = ROOT / "Games"
EXES = ROOT / "exes"
MANIFEST = ROOT / "shortcuts.json"
IS_WIN = os.name == "nt"
SEARCH_DEPTH = 6
# Large dependency/data trees that never hold a launcher; skipped when searching.
SKIP_DIRS = {".git", "toolchain", "deps", "node_modules", "build-cache", "decomp", "sdk",
             "dump", "romfs_sky", "content", "game-inputs", "cache", "__pycache__"}


# ---------- paths ----------

def to_win(p: Path | str) -> str:
    s = str(p)
    if IS_WIN:
        return s
    m = re.match(r"^/mnt/([a-zA-Z])(/.*)?$", s)
    if m:
        return f"{m[1].upper()}:" + (m[2] or "/").replace("/", "\\")
    return subprocess.run(["wslpath", "-w", s], capture_output=True, text=True, check=True).stdout.strip()


def from_win(s: str) -> Path | None:
    if not s:
        return None
    if IS_WIN:
        return Path(s)
    m = re.match(r"^([A-Za-z]):[\\/](.*)$", s)
    if not m:
        return None
    return Path(f"/mnt/{m[1].lower()}") / PurePosixPath(*PureWindowsPath(m[2]).parts)


def same_path(a: str, b: str) -> bool:
    return (a or "").rstrip("\\").lower() == (b or "").rstrip("\\").lower()


def natural_key(p: Path):
    return [int(t) if t.isdigit() else t.lower() for t in re.split(r"(\d+)", str(p))]


# ---------- PowerShell bridge (WScript.Shell is the only stdlib-free way to read/write .lnk) ----------

def ps(script: str, data) -> object:
    body = ("$ErrorActionPreference = 'Stop'\n"
            "[Console]::OutputEncoding = [Text.Encoding]::UTF8\n"
            "$data = @'\n" + json.dumps(data) + "\n'@ | ConvertFrom-Json\n"
            "$sh = New-Object -ComObject WScript.Shell\n" + script)
    exe = "powershell" if IS_WIN else "powershell.exe"
    enc = base64.b64encode(body.encode("utf-16-le")).decode()
    r = subprocess.run([exe, "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", enc],
                       capture_output=True, text=True, encoding="utf-8", errors="replace")
    if r.returncode != 0:
        sys.exit(f"powershell failed:\n{r.stderr.strip()}")
    out = r.stdout.strip()
    return json.loads(out) if out else None


READ_PS = r"""
$out = foreach ($p in $data) { if (Test-Path -LiteralPath $p) {
  $l = $sh.CreateShortcut($p)
  [pscustomobject]@{ path = $p; target = $l.TargetPath; wd = $l.WorkingDirectory; icon = $l.IconLocation } } }
ConvertTo-Json -Compress -InputObject @($out)
"""

WRITE_PS = r"""
foreach ($s in $data) {
  $l = $sh.CreateShortcut($s.path)
  $l.TargetPath = $s.target
  $l.WorkingDirectory = $s.wd
  if ($s.icon) { $l.IconLocation = $s.icon }
  $l.Save()
}
"""

# Shortcuts in the Start Menu / Desktop whose target file is missing.
BROKEN_EXTERNAL_PS = r"""
$dirs = @("$env:APPDATA\Microsoft\Windows\Start Menu", "$env:ProgramData\Microsoft\Windows\Start Menu",
          [Environment]::GetFolderPath('Desktop'), "$env:USERPROFILE\Desktop", "$env:PUBLIC\Desktop") |
        Where-Object { $_ -and (Test-Path -LiteralPath $_) } | Sort-Object -Unique
$out = foreach ($f in (Get-ChildItem -Recurse -Filter *.lnk -LiteralPath $dirs -ErrorAction SilentlyContinue)) {
  $l = $sh.CreateShortcut($f.FullName)
  if ($l.TargetPath -and -not (Test-Path -LiteralPath $l.TargetPath)) {
    [pscustomobject]@{ path = $f.FullName; target = $l.TargetPath; wd = $l.WorkingDirectory; icon = $l.IconLocation } } }
ConvertTo-Json -Compress -InputObject @($out)
"""


# ---------- resolving targets ----------

def win_exists(p: Path) -> bool:
    """Ask Windows directly. WSL can show a folder as missing (`d?????????`) after it was moved
    while a WSL process had its working directory inside it."""
    return not IS_WIN and bool(ps("Test-Path -LiteralPath $data | ConvertTo-Json", to_win(p)))


def search_by_name(name: str) -> list[Path]:
    hits = []
    base_depth = len(GAMES.parts)
    for dirpath, dirnames, filenames in os.walk(GAMES):
        if len(Path(dirpath).parts) - base_depth >= SEARCH_DEPTH:
            dirnames[:] = []
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        hits += [Path(dirpath) / f for f in filenames if f.lower() == name.lower()]
    return hits


def resolve(entry: dict, notes: list[str]) -> Path | None:
    rel = entry["target"]
    if any(c in rel for c in "*?["):
        matches = sorted(ROOT.glob(rel), key=natural_key)
        return matches[-1] if matches else None
    p = ROOT / rel
    if p.exists() or win_exists(p):
        return p
    hits = search_by_name(p.name)
    if len(hits) == 1:
        new_rel = hits[0].relative_to(ROOT).as_posix()
        notes.append(f"moved    {entry['name']}: {rel} -> {new_rel}")
        entry["target"] = new_rel
        return hits[0]
    if len(hits) > 1:
        notes.append(f"ambiguous {entry['name']}: {p.name} found at " + ", ".join(h.relative_to(ROOT).as_posix() for h in hits))
    return None


def repair_external(broken: dict, game_dirs: dict[str, Path], by_name: dict[str, Path]) -> Path | None:
    """Map a missing target onto Games/: longest path suffix starting at a game folder name, else a manifest file name."""
    parts = PureWindowsPath(broken["target"]).parts[1:]
    for i, part in enumerate(parts):
        gd = game_dirs.get(part.lower())
        if gd is not None:
            cand = gd.joinpath(*parts[i + 1:])
            if cand.exists():
                return cand
    return by_name.get(PureWindowsPath(broken["target"]).name.lower())


# ---------- main ----------

def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--check", action="store_true", help="report only; exit 1 if anything would change")
    ap.add_argument("--prune", action="store_true", help="delete exes/*.lnk that are not in shortcuts.json")
    ap.add_argument("--external", action="store_true", help="also repair broken Start Menu / Desktop shortcuts")
    args = ap.parse_args()

    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    entries = manifest["shortcuts"]
    notes: list[str] = []
    pending = 0

    resolved = {e["name"]: resolve(e, notes) for e in entries}
    manifest_changed = any(n.startswith("moved") for n in notes)

    # exes/ sync
    EXES.mkdir(exist_ok=True)
    lnk_paths = {name: to_win(EXES / f"{name}.lnk") for name in resolved}
    current = {r["path"].lower(): r for r in (ps(READ_PS, list(lnk_paths.values())) or [])}
    writes = []
    for name, target in resolved.items():
        if target is None:
            print(f"missing  {name}  ({next(e['target'] for e in entries if e['name'] == name)})")
            continue
        want = {"path": lnk_paths[name], "target": to_win(target), "wd": to_win(target.parent)}
        have = current.get(want["path"].lower())
        if have and same_path(have["target"], want["target"]) and same_path(have["wd"], want["wd"]):
            print(f"ok       {name}")
            continue
        print(f"{'update ' if have else 'create '}  {name} -> {want['target']}")
        writes.append(want)

    unmanaged = sorted(p for p in EXES.glob("*.lnk") if p.stem not in resolved)
    for p in unmanaged:
        print(f"{'delete ' if args.prune else 'extra  '}  exes/{p.name} (not in shortcuts.json)")

    # Games/ folders with no shortcut
    game_dirs = {d.name.lower(): d for d in GAMES.iterdir() if d.is_dir()} if GAMES.is_dir() else {}
    covered = {Path(e["target"]).parts[1].lower() for e in entries if Path(e["target"]).parts[:1] == ("Games",)}
    for key, d in sorted(game_dirs.items()):
        if key not in covered:
            print(f"unlisted Games/{d.name} has no shortcut; add it to shortcuts.json")

    # Start Menu / Desktop
    ext_writes = []
    if args.external:
        by_name = {t.name.lower(): t for t in resolved.values() if t is not None}
        for b in ps(BROKEN_EXTERNAL_PS, None) or []:
            fix = repair_external(b, game_dirs, by_name)
            if fix is None:
                if any(part.lower() in game_dirs for part in PureWindowsPath(b["target"]).parts):
                    print(f"broken   {b['path']} -> {b['target']} (no match found under Games/)")
                continue
            old_dir = str(PureWindowsPath(b["target"]).parent)
            new_t = to_win(fix)
            icon = b["icon"]
            if icon and old_dir.lower() in icon.lower():
                icon = re.sub(re.escape(old_dir), lambda _: str(PureWindowsPath(new_t).parent), icon, flags=re.I)
            print(f"repair   {b['path']} -> {new_t}")
            ext_writes.append({"path": b["path"], "target": new_t, "wd": to_win(fix.parent), "icon": icon})

    for n in notes:
        print(n)

    pending = len(writes) + len(ext_writes) + (len(unmanaged) if args.prune else 0) + int(manifest_changed)
    if args.check:
        return 1 if pending else 0

    if writes or ext_writes:
        ps(WRITE_PS, writes + ext_writes)
    if args.prune:
        for p in unmanaged:
            p.unlink()
    if manifest_changed:
        MANIFEST.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        print("wrote shortcuts.json (commit it)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
