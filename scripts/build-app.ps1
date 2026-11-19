#Requires -Version 7
# Builds the Flacora app in release mode and packages it as an AppImage on Linux.
#
#   ./scripts/build-app.ps1            -> dist/<platform>/Flacora-<version>-<platform>[.exe|.AppImage]
#   ./scripts/build-app.ps1 -Out build

param(
    [string]$Out,
    [string]$BuildRoot
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'release-build.ps1')
if (-not $BuildRoot) { $BuildRoot = $root }

function New-FlacoraAppImage([string] $Binary, [string] $Destination, [string] $Architecture) {
    $tool = if ($env:APPIMAGETOOL) { $env:APPIMAGETOOL } else { 'appimagetool' }
    $tool = (Get-Command -Name $tool -CommandType Application -ErrorAction SilentlyContinue |
        Select-Object -First 1).Source
    if (-not $tool) { throw "Install appimagetool or set APPIMAGETOOL to its executable path" }

    $stage = mktemp -d
    if ($LASTEXITCODE -ne 0) { throw "Could not create an AppImage staging directory" }
    $pending = "$Destination.$([guid]::NewGuid().ToString('N')).tmp"
    $savedArch = $env:ARCH
    $savedExtract = $env:APPIMAGE_EXTRACT_AND_RUN
    try {
        $appDir = Join-Path $stage 'Flacora.AppDir'
        New-Item -ItemType Directory -Path (Join-Path $appDir 'usr/bin') -Force | Out-Null
        & install -m 0755 -- $Binary (Join-Path $appDir 'usr/bin/flacora')
        if ($LASTEXITCODE -ne 0) { throw "Could not stage the Flacora executable" }
        $desktop = Get-Content -LiteralPath (Join-Path $root 'assets/flacora.desktop') -Raw
        [IO.File]::WriteAllText((Join-Path $appDir 'flacora.desktop'), $desktop.Replace("`r`n", "`n"))
        Copy-Item -LiteralPath (Join-Path $root 'assets/appicon.png') -Destination (Join-Path $appDir 'flacora.png')
        & ln -s -- 'usr/bin/flacora' (Join-Path $appDir 'AppRun')
        if ($LASTEXITCODE -ne 0) { throw "Could not create the AppImage entry point" }

        $env:ARCH = $Architecture
        $env:APPIMAGE_EXTRACT_AND_RUN = '1'
        $image = Join-Path $stage 'Flacora.AppImage'
        $toolArgs = @('--no-appstream')
        if ($env:APPIMAGETOOL_RUNTIME_FILE) {
            $toolArgs += @('--runtime-file', $env:APPIMAGETOOL_RUNTIME_FILE)
        }
        & $tool @toolArgs $appDir $image
        if ($LASTEXITCODE -ne 0) { throw "appimagetool failed to build Flacora" }
        & install -m 0755 -- $image $pending
        if ($LASTEXITCODE -ne 0) { throw "Could not copy the Flacora AppImage" }
        [IO.File]::Move($pending, $Destination, $true)
    } finally {
        $env:ARCH = $savedArch
        $env:APPIMAGE_EXTRACT_AND_RUN = $savedExtract
        if (Test-Path -LiteralPath $pending) { Remove-Item -LiteralPath $pending -Force }
        Remove-Item -LiteralPath $stage -Recurse -Force
    }
}

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

$dist = if (-not $Out) {
    Join-Path $BuildRoot "dist/$($build.Platform)"
} elseif ([IO.Path]::IsPathRooted($Out)) {
    $Out
} else {
    Join-Path $root $Out
}
New-Item -ItemType Directory -Force $dist | Out-Null
$binary = Join-Path $build.Directory "$($build.Triple)/release/$exe"
$name = "Flacora-$version-$($build.Platform)"
if ($build.Platform -like 'linux-*') {
    $target = Join-Path $dist "$name.AppImage"
    New-FlacoraAppImage $binary $target (($build.Triple -split '-')[0])
} else {
    $target = Join-Path $dist "$name$($build.ExeSuffix)"
    Copy-Item -LiteralPath $binary -Destination $target -Force
}
$size = (Get-Item $target).Length
Write-Host ("{0} ({1:N1} MB)" -f $target, ($size / 1MB))
