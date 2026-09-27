<#
Builds every example of the `mitsuami` crate for WinUI and copies the
binaries into one folder, so they can be run straight from there:

  target\examples\winui\<example>.exe

Usage: scripts\build-examples.ps1 [-Release] [-CargoFlags '<extra flags>']

It uses the MSVC build of the pinned toolchain: windows-rs links through
raw-dylib, which fails on the gnu one.
#>
param(
    [switch]$Release,
    [string]$CargoFlags = ''
)
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

# rust-toolchain.toml names only the channel, which resolves to the host's
# default target (gnu on some machines), so name the MSVC one explicitly.
$channel = (Select-String -Path rust-toolchain.toml -Pattern '^channel\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
$toolchain = "+$channel-x86_64-pc-windows-msvc"

$buildProfile = 'debug'
$cargoArgs = @($toolchain, 'build', '--package', 'mitsuami', '--examples')
if ($Release) {
    $buildProfile = 'release'
    $cargoArgs += '--release'
}
if ($CargoFlags) {
    $cargoArgs += $CargoFlags -split '\s+'
}

$targetDir = (cargo $toolchain metadata --format-version 1 --no-deps | ConvertFrom-Json).target_directory
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$out = Join-Path $targetDir 'examples\winui'

# Every `examples/<name>.rs` and `examples/<name>/main.rs` is an example.
$examples = @()
foreach ($item in Get-ChildItem crates\mitsuami\examples) {
    if (-not $item.PSIsContainer -and $item.Extension -eq '.rs') {
        $examples += $item.BaseName
    } elseif ($item.PSIsContainer -and (Test-Path (Join-Path $item.FullName 'main.rs'))) {
        $examples += $item.Name
    }
}

Write-Host "==> Building $($examples.Count) examples for winui ($buildProfile)"
& cargo @cargoArgs
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if (Test-Path $out) { Remove-Item -Recurse -Force $out }
New-Item -ItemType Directory -Force $out | Out-Null
foreach ($example in $examples) {
    Copy-Item (Join-Path $targetDir "$buildProfile\examples\$example.exe") $out
}
Write-Host "    -> $out"
Write-Host ''
Write-Host "Run one with e.g.: $(Join-Path $out "$($examples[0]).exe")"
