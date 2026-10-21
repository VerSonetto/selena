$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

Write-Host "Starting Selena API on http://127.0.0.1:13697"
Start-Process -FilePath "$root\target\debug\selena.exe" -ArgumentList @("web", "--no-open") -WorkingDirectory $root

Push-Location (Join-Path $root "web")
try {
    pnpm dev
} finally {
    Pop-Location
}
