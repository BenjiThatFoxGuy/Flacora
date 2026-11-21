# Shared settings for native release builds.

function Get-ReleaseTarget([string] $Root, [string] $BuildRoot) {
    $buildDirectory = 'target/release-build'
    if ($IsLinux) {
        $release = Get-Content -LiteralPath /etc/os-release -Raw
        if ($release -cnotmatch '(?m)^ID="?ubuntu"?\r?$' -or
            $release -cnotmatch '(?m)^VERSION_ID="?22\.04"?\r?$') {
            throw 'Linux releases must be built on Ubuntu 22.04 LTS (glibc 2.35). On Windows, use the Ubuntu-22.04 WSL distro.'
        }
        $buildDirectory = 'target/release-build-ubuntu-22.04'
    }

    Push-Location $Root
    try {
        $version = rustc -vV
        if ($LASTEXITCODE -ne 0) { throw "Could not identify the Rust target" }
    } finally {
        Pop-Location
    }
    $triple = ($version | Select-String '^host: (.+)$').Matches[0].Groups[1].Value
    $platform = switch -Regex ($triple) {
        '^x86_64-pc-windows-msvc$' { 'windows-x64' }
        '^aarch64-pc-windows-msvc$' { 'windows-arm64' }
        '^x86_64-unknown-linux-gnu$' { 'linux-x64' }
        '^aarch64-unknown-linux-gnu$' { 'linux-arm64' }
        '^x86_64-apple-darwin$' { 'macos-x64' }
        '^aarch64-apple-darwin$' { 'macos-arm64' }
        default { throw "No native release target for $triple" }
    }
    @{
        Triple = $triple
        Platform = $platform
        Directory = Join-Path $BuildRoot $buildDirectory
        ExeSuffix = if ($triple -like '*-windows-*') { '.exe' } else { '' }
    }
}

function Invoke-ReleaseBuild([string] $Root, [string] $Project, [hashtable] $Target) {
    $savedFlags = $env:CARGO_ENCODED_RUSTFLAGS
    $savedStrip = $env:CARGO_PROFILE_RELEASE_STRIP
    $profile = [Environment]::GetFolderPath('UserProfile')
    $cargoCache = $env:CARGO_HOME
    if (!$cargoCache -and $profile) { $cargoCache = Join-Path $profile '.cargo' }
    $flags = @()
    foreach ($mapping in @(
        @{ From = $profile; To = '/build/user' },
        @{ From = $cargoCache; To = '/build/cargo' },
        @{ From = $Root; To = '/build/flacora' }
    )) {
        if (!$mapping.From) { continue }
        $absolute = [IO.Path]::GetFullPath($mapping.From)
        foreach ($prefix in @($absolute, $absolute.Replace('\', '/')) | Select-Object -Unique) {
            $flags += @('--remap-path-prefix', "$prefix=$($mapping.To)")
        }
    }
    if ($Target.Triple -like '*-windows-msvc') {
        $flags += @('-C', 'target-feature=+crt-static')
    }
    Push-Location $Project
    try {
        $env:CARGO_ENCODED_RUSTFLAGS = $flags -join [char] 31
        $env:CARGO_PROFILE_RELEASE_STRIP = 'symbols'
        cargo build --release --locked --target $Target.Triple --target-dir $Target.Directory
        if ($LASTEXITCODE -ne 0) { throw "Release build failed in $Project" }
    } finally {
        $env:CARGO_ENCODED_RUSTFLAGS = $savedFlags
        $env:CARGO_PROFILE_RELEASE_STRIP = $savedStrip
        Pop-Location
    }
}
