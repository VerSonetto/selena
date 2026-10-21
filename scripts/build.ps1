$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

Push-Location (Join-Path $root "web")
try {
    pnpm install --frozen-lockfile
    pnpm build
    if ($LASTEXITCODE -ne 0) { throw "web build failed" }
} finally {
    Pop-Location
}

Push-Location $root
try {
    cargo build --release -p selena-cli
    if ($LASTEXITCODE -ne 0) { throw "cli build failed" }
} finally {
    Pop-Location
}

Write-Host "Binary: $root\target\release\selena.exe"
Write-Host "For a Windows installer, run scripts/build-desktop.ps1"
