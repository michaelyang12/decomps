# Regenerates the Windows launchers in exes\ (git-ignored).
# Run: powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\Users\micha\Decomps\make-shortcuts.ps1
$root  = $PSScriptRoot
$games = Join-Path $root 'Games'
$progs = Join-Path $env:LOCALAPPDATA 'Programs'
$out   = Join-Path $root 'exes'

$nebulaVer = (Get-Content (Join-Path $progs 'Nebula\current.json') -Raw | ConvertFrom-Json).version
$ssbu = "$games\ssbu-recomp-main\ssbu-recomp-main"

$items = [ordered]@{
  'Super Mario 64 (sm64ex-alo)' = "$games\sm64ex-alo\build\us_pc\sm64.us.f3dex2e.exe"
  'Super Mario Sunshine'        = "$games\sms-pc-port\build\windows-64\sms-standalone.exe"
  'Zelda TOTK'                  = "$games\totk-recomp\totk.exe"
  'Play SSBU'                   = "$ssbu\Play SSBU.bat"
  'Play SSBU (modded)'          = "$ssbu\Play SSBU (modded).bat"
  'Nebula'                      = "$progs\Nebula\versions\$nebulaVer\Nebula.exe"
  'SM64DS Port'                 = "$progs\SM64DS Port\SM64DSLauncher.exe"
}

New-Item -ItemType Directory -Force $out | Out-Null
$sh = New-Object -ComObject WScript.Shell
foreach ($name in $items.Keys) {
  $target = $items[$name]
  $lnk = $sh.CreateShortcut((Join-Path $out "$name.lnk"))
  $lnk.TargetPath = $target
  $lnk.WorkingDirectory = Split-Path $target   # games load data relative to their own folder
  $lnk.Save()
  $flag = if (Test-Path -LiteralPath $target) { 'ok' } else { 'MISSING' }
  '{0,-8} {1,-30} -> {2}' -f $flag, $name, $target
}
