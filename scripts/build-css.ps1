# Rebuild the Tailwind CSS using the standalone CLI (no Node).
# Downloads the CLI on first run into ./tools. Run from the repo root:
#   ./scripts/build-css.ps1          # one-off build
#   ./scripts/build-css.ps1 -Watch   # rebuild on change

param([switch]$Watch)

$ErrorActionPreference = "Stop"
$tool = "tools/tailwindcss.exe"

if (-not (Test-Path $tool)) {
    New-Item -ItemType Directory -Force tools | Out-Null
    Write-Host "Downloading Tailwind standalone CLI..."
    Invoke-WebRequest -Uri "https://github.com/tailwindlabs/tailwindcss/releases/latest/download/tailwindcss-windows-x64.exe" -OutFile $tool
}

$args = @("-i", "styles/input.css", "-o", "static/app.css", "--minify")
if ($Watch) { $args += "--watch" }
& ".\$tool" @args
