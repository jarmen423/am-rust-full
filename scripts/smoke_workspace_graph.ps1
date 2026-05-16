<#
.SYNOPSIS
    Smoke-test workspace-server graph endpoints (HTTP 200 + JSON shape).

.PARAMETER ExePath
    Path to workspace-server.exe. Defaults to debug build under repo target/.
.PARAMETER Port
    Listening port (default 3031).
#>

param(
    [string]$ExePath = "",
    [int]$Port = 3031
)

$ErrorActionPreference = "Stop"
$Root = Resolve-Path (Join-Path $PSScriptRoot "..")

if (-not $ExePath) {
    $ExePath = Join-Path $Root "target\debug\workspace-server.exe"
    if (-not (Test-Path -LiteralPath $ExePath)) {
        $ExePath = Join-Path $Root "target\release\workspace-server.exe"
    }
}

if (-not (Test-Path -LiteralPath $ExePath)) {
    throw "workspace-server not found. Build first:`n  pwsh -File scripts/build_workspace_server.ps1`n  or: cargo build --bin workspace-server`n  (shim-only: add --no-default-features)"
}

$env:PORT = "$Port"

# Minimal dist tree so ServeDir + index fallback never points at a missing path during CI smoke.
$tmpDist = Join-Path $env:TEMP ("ws-smoke-workspace-dist-" + [Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Force -Path $tmpDist | Out-Null
Set-Content -Path (Join-Path $tmpDist "index.html") -Value "<!DOCTYPE html><title>smoke</title><body>ok</body>" -Encoding UTF8
$env:DIST_PATH = $tmpDist

$p = Start-Process -FilePath $ExePath -WorkingDirectory $Root `
    -WindowStyle Hidden -PassThru `
    -RedirectStandardOutput (Join-Path $env:TEMP "ws-smoke-out.txt") `
    -RedirectStandardError (Join-Path $env:TEMP "ws-smoke-err.txt")

try {
    Start-Sleep -Seconds 3
    if ($p.HasExited) {
        $errPath = Join-Path $env:TEMP "ws-smoke-err.txt"
        $err = if (Test-Path $errPath) { Get-Content -LiteralPath $errPath -Raw } else { "(no stderr file)" }
        throw "workspace-server exited early (code=$($p.ExitCode)). Stderr:`n$err"
    }
    $base = "http://127.0.0.1:$Port"
    $explore = Invoke-RestMethod -Uri "$base/api/workspace/graph/explore?limit=5" -TimeoutSec 15
    if ($explore.status -ne "ok") { throw "explore: unexpected status $($explore.status)" }

    $repos = Invoke-RestMethod -Uri "$base/api/workspace/graph/repos" -TimeoutSec 15
    if ($repos.status -ne "ok") { throw "repos: unexpected status $($repos.status)" }

    Write-Host "OK: graph/explore + graph/repos returned status=ok (nodes=$($explore.nodes.Count), repos=$($repos.repos.Count))"
}
finally {
    if (-not $p.HasExited) {
        Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
    }
}
