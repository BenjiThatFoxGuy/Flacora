#Requires -Version 7
# Builds the Flacora app in release mode and copies the executable to dist/.
#
#   ./scripts/build-app.ps1            -> dist/Flacora-<version>-windows-x64.exe
#   ./scripts/build-app.ps1 -Out build

param(
    [string]$Out = "dist",
    [string]$BuildRoot
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'release-build.ps1')
if (-not $BuildRoot) { $BuildRoot = $root }

Push-Location $root
try {
    $metadata = cargo metadata --locked --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw "Could not read gui package metadata" }
} finally {
    Pop-Location
}
$version = ($metadata.packages | Where-Object { $_.name -eq 'flacora' } | Select-Object -First 1).version
if (-not $version) { throw "Could not determine flacora version from Cargo metadata" }
$build = Get-ReleaseTarget $root
$build.Directory = Join-Path $BuildRoot 'target/release-build'
$exe = "flacora$($build.ExeSuffix)"
Invoke-ReleaseBuild $BuildRoot $root $build

$dist = if ([System.IO.Path]::IsPathRooted($Out)) { $Out } else { Join-Path $root $Out }
New-Item -ItemType Directory -Force $dist | Out-Null
$target = Join-Path $dist "Flacora-$version-$($build.Platform)$($build.ExeSuffix)"
Copy-Item -LiteralPath (Join-Path $build.Directory "$($build.Triple)/release/$exe") -Destination $target -Force
$size = (Get-Item $target).Length
Write-Host ("{0} ({1:N1} MB)" -f $target, ($size / 1MB))
