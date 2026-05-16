<#
.SYNOPSIS
    Build workspace-server with native Ladybug on Windows (shared DLL).

.DESCRIPTION
    Mirrors tools/ladybug_code_index_worker/build_embedded.ps1: downloads the official
    liblbug Windows shared artifact if missing, sets LBUG_SHARED / LBUG_LIBRARY_DIR /
    LBUG_INCLUDE_DIR, then runs cargo build for am-rust-full.

    Run from anywhere:
      powershell -ExecutionPolicy Bypass -File scripts/build_workspace_server.ps1

.NOTES
    Parent repo must be agentic-memory (this crate lives at agentic-memory/am-rust-full).
#>

param(
    [ValidateSet("release", "debug")]
    [string]$Profile = "release",

    # Optional: redirect Cargo output (helps when the default target disk is full).
    [string]$CargoTargetDir = ""
)

$ErrorActionPreference = "Stop"

if ($env:LBUG_BUILD_FROM_SOURCE -or $env:LBUG_RUST_BUILD_FROM_SOURCE) {
    throw "Refusing to build: LBUG_BUILD_FROM_SOURCE would force a Ladybug source build."
}

$AmRustRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$RepoRoot = Resolve-Path (Join-Path $AmRustRoot "..")
$WorkerManifest = Join-Path $RepoRoot "tools\ladybug_code_index_worker\Cargo.toml"

function Find-LbugCrateDir {
    $registry = Join-Path $env:USERPROFILE ".cargo\registry\src"
    $matches = Get-ChildItem -LiteralPath $registry -Recurse -Directory -Filter "lbug-0.16.1" -ErrorAction SilentlyContinue
    if ($matches) {
        return $matches[0].FullName
    }
    & cargo fetch --manifest-path (Join-Path $AmRustRoot "Cargo.toml")
    if ($LASTEXITCODE -ne 0) {
        throw "cargo fetch failed while preparing the lbug crate."
    }
    $matches = Get-ChildItem -LiteralPath $registry -Recurse -Directory -Filter "lbug-0.16.1" -ErrorAction SilentlyContinue
    if (-not $matches) {
        throw "Could not locate lbug-0.16.1 under Cargo registry after cargo fetch."
    }
    return $matches[0].FullName
}

function Ensure-WindowsSharedLbug {
    $crateDir = Find-LbugCrateDir
    $workerRoot = Split-Path -Parent (Resolve-Path -LiteralPath $WorkerManifest).Path
    $sharedDir = Join-Path $workerRoot ".ladybug\windows-x86_64-shared"
    $includeDir = Join-Path $workerRoot ".ladybug\windows-x86_64-shared-include"
    $dllPath = Join-Path $sharedDir "lbug_shared.dll"
    $importLibPath = Join-Path $sharedDir "lbug_shared.lib"

    if (-not ((Test-Path -LiteralPath $dllPath) -and (Test-Path -LiteralPath $importLibPath))) {
        New-Item -ItemType Directory -Force -Path $sharedDir | Out-Null
        $tag = "v0.16.1"
        $archive = "liblbug-windows-x86_64.zip"
        $url = "https://github.com/LadybugDB/ladybug/releases/download/$tag/$archive"
        $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("lbug-shared-" + [System.Guid]::NewGuid().ToString())
        New-Item -ItemType Directory -Force -Path $tmp | Out-Null
        try {
            $zip = Join-Path $tmp $archive
            Write-Host "Downloading official Ladybug shared artifact from $url"
            Invoke-WebRequest -Uri $url -OutFile $zip -Headers @{ "User-Agent" = "agentic-memory-build" }
            Expand-Archive -LiteralPath $zip -DestinationPath $sharedDir -Force
        }
        finally {
            Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
        }
    }

    if (-not (Test-Path -LiteralPath $dllPath)) {
        throw "Missing Ladybug shared DLL: $dllPath"
    }

    $sourceIncludeDir = Join-Path $crateDir "lbug-src\src\include"
    if (-not (Test-Path -LiteralPath $sourceIncludeDir)) {
        throw "Missing lbug crate headers: $sourceIncludeDir"
    }

    if (Test-Path -LiteralPath $includeDir) {
        Remove-Item -LiteralPath $includeDir -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $includeDir | Out-Null
    Get-ChildItem -LiteralPath $sourceIncludeDir -Force | ForEach-Object {
        Copy-Item -LiteralPath $_.FullName -Destination $includeDir -Recurse -Force
    }
    Copy-Item -LiteralPath (Join-Path $sharedDir "lbug.h") -Destination $includeDir -Force
    Copy-Item -LiteralPath (Join-Path $sharedDir "lbug.hpp") -Destination $includeDir -Force

    $env:LBUG_SHARED = "1"
    $env:LBUG_LIBRARY_DIR = $sharedDir
    $env:LBUG_INCLUDE_DIR = $includeDir
    Write-Host "LBUG_LIBRARY_DIR=$sharedDir"
}

if ($IsWindows -or $env:OS -eq "Windows_NT") {
    Ensure-WindowsSharedLbug
}

$cargoArgs = @("build", "--manifest-path", (Join-Path $AmRustRoot "Cargo.toml"), "--bin", "workspace-server")
if ($Profile -eq "release") {
    $cargoArgs += "--release"
}
if ($CargoTargetDir) {
    $cargoArgs += @("--target-dir", $CargoTargetDir)
}

Write-Host "cargo $($cargoArgs -join ' ')"
& cargo @cargoArgs
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

if (($IsWindows -or $env:OS -eq "Windows_NT") -and $env:LBUG_LIBRARY_DIR) {
    $profileDir = if ($Profile -eq "release") { "release" } else { "debug" }
    $targetRoot = if ($CargoTargetDir) { $CargoTargetDir }
        elseif ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR }
        else { Join-Path $AmRustRoot "target" }
    $outputDir = Join-Path $targetRoot $profileDir
    $dllPath = Join-Path $env:LBUG_LIBRARY_DIR "lbug_shared.dll"
    if ((Test-Path -LiteralPath $outputDir) -and (Test-Path -LiteralPath $dllPath)) {
        Copy-Item -LiteralPath $dllPath -Destination $outputDir -Force
        Write-Host "Copied lbug_shared.dll to $outputDir"
    }
}
