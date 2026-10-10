$ErrorActionPreference = "SilentlyContinue"

$bundle = "E:\SSD M2\cargo-target\warp-generator\release\bundle"
$out = "E:\SSD M2\releases"

if (-not (Test-Path $bundle)) { exit 0 }

New-Item -ItemType Directory -Force -Path "$out\nsis", "$out\msi" | Out-Null

$locked = @()

foreach ($f in Get-ChildItem "$bundle\nsis\*.exe") {
    try {
        Copy-Item $f.FullName "$out\nsis\" -Force -ErrorAction Stop
    } catch {
        $locked += $f.Name
    }
}

foreach ($f in Get-ChildItem "$bundle\msi\*.msi") {
    try {
        Copy-Item $f.FullName "$out\msi\" -Force -ErrorAction Stop
    } catch {
        $locked += $f.Name
    }
}

$latest = Get-ChildItem "$out\nsis\*.exe" | Sort-Object Name -Descending | Select-Object -First 1
if ($latest) { Write-Host "installers synced, latest: $($latest.Name)" }

foreach ($n in $locked) {
    Write-Host "skipped (file busy): $n"
}
