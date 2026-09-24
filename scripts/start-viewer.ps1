param([int]$Port = 8765)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
$executable = Join-Path $workspace 'target/release/apex.exe'
if (-not (Test-Path -LiteralPath $executable)) { throw 'Run cargo build --release first.' }
& $executable serve --port $Port --workspace $workspace
