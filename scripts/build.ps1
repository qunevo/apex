$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
$cargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
$cargoBinary = if ($cargoCommand) { $cargoCommand.Source } else { Join-Path $env:USERPROFILE '.cargo/bin/cargo.exe' }
if (-not (Test-Path -LiteralPath $cargoBinary)) { throw 'Install stable Rust and the native build tools first.' }
Push-Location -LiteralPath $workspace
try {
    & $cargoBinary fmt --check
    if ($LASTEXITCODE -ne 0) { throw 'Rust formatting failed.' }
    & $cargoBinary clippy --locked --all-targets -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'Rust lint checks failed.' }
    & $cargoBinary test --locked
    if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed.' }
    & $cargoBinary build --locked --release
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed. Stop a running viewer before replacing the executable on Windows.' }
} finally { Pop-Location }
