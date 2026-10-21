param(
    [switch]$SkipWeb,
    [switch]$SkipCli
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

if (-not $SkipWeb) {
    Push-Location (Join-Path $root "web")
    try {
        pnpm build
        if ($LASTEXITCODE -ne 0) { throw "web build failed" }
    } finally {
        Pop-Location
    }
}

if (-not $SkipCli) {
    Push-Location $root
    try {
        cargo build --release -p selena-cli
        if ($LASTEXITCODE -ne 0) { throw "cli build failed" }
    } finally {
        Pop-Location
    }
}

$cli = Join-Path $root "target\release\selena.exe"
$bin = Join-Path $root "crates\selena-desktop\bin"
New-Item -ItemType Directory -Force -Path $bin | Out-Null
Copy-Item $cli (Join-Path $bin "selena.exe") -Force
Copy-Item (Join-Path $root "crates\selena-desktop\path-hook.ps1") (Join-Path $bin "path-hook.ps1") -Force

Push-Location (Join-Path $root "crates\selena-desktop")
try {
    & (Join-Path $root "web\node_modules\.bin\tauri.CMD") build
    if ($LASTEXITCODE -ne 0) { throw "tauri build failed" }
} finally {
    Pop-Location
}

Write-Host "Selena desktop bundle is in target/release/bundle/nsis"
