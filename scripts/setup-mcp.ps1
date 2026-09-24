param([string]$Workspace = (Split-Path -Parent $PSScriptRoot))
$ErrorActionPreference = 'Stop'
$Workspace = (Resolve-Path -LiteralPath $Workspace).Path
$executable = Join-Path $Workspace 'target/release/apex.exe'
if (-not (Test-Path -LiteralPath $executable)) { throw 'Run cargo build --release first.' }
$configDirectory = Join-Path $Workspace '.codex'
$configPath = Join-Path $configDirectory 'config.toml'
New-Item -ItemType Directory -Force -Path $configDirectory | Out-Null
if ((Test-Path -LiteralPath $configPath) -and ((Get-Content -Raw -LiteralPath $configPath) -match '\[mcp_servers\.apex\]')) {
    Write-Output 'APEX MCP is already configured in this project.'
    exit 0
}
$binaryToml = $executable.Replace('\', '/')
$workspaceToml = $Workspace.Replace('\', '/')
$configuration = @"

[mcp_servers.apex]
command = '$binaryToml'
args = ['mcp', '--with-viewer', '--workspace', '$workspaceToml']
cwd = '$workspaceToml'
startup_timeout_sec = 20
tool_timeout_sec = 120
enabled = true
"@
Add-Content -LiteralPath $configPath -Value $configuration -Encoding utf8
Write-Output "Configured APEX in $configPath. Restart the MCP connection or open a new trusted Codex task."
