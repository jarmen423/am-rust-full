#!/usr/bin/env pwsh
# Smoke: generate rust-analyzer SCIP for the mini fixture, run jina-ladybug-repo-index, print hint for CALLS verification.
# Requires: JINA_API_KEY, rust-analyzer on PATH (optional skip), LBUG DLL next to indexer if applicable.

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot
$Mini = Join-Path $RepoRoot "fixtures\scip_calls_smoke\mini_rust_proj"
$Indexer = Join-Path $RepoRoot "target\debug\jina-ladybug-repo-index.exe"

if (-not (Test-Path $Indexer)) {
    Write-Error "Build the indexer first: cargo build -p am-workspace --features jina-ladybug-index --bin jina-ladybug-repo-index"
}

Push-Location $Mini
try {
    if (Get-Command rust-analyzer -ErrorAction SilentlyContinue) {
        rust-analyzer scip .
        Write-Host "Wrote SCIP index at $(Join-Path $Mini 'index.scip')"
    } else {
        Write-Warning "rust-analyzer not on PATH; skipping `rust-analyzer scip .`. Place index.scip in $Mini manually."
    }

    $db = Join-Path $env:TEMP "scip_calls_smoke.lbug"
    if (-not $env:JINA_API_KEY) {
        Write-Error "Set JINA_API_KEY before running this script."
    }

    & $Indexer `
        --repo $Mini `
        --db $db `
        --repo-id "fixture/scip-calls-smoke" `
        --init-schema

    Write-Host "Done. Inspect CALLS non-zero:"
    Write-Host "  MATCH (:Function)-[r:CALLS]->(:Function) RETURN count(r);"
    Write-Host "DB: $db"
}
finally {
    Pop-Location
}
