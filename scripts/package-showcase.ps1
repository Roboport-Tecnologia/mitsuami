<#
Builds the showcase (examples/showcase) and packs it as an MSIX for the
Microsoft Store:

  examples\showcase\target\msix\mitsuami-showcase_<version>_<arch>.msix

Usage: scripts\package-showcase.ps1 [-Version 1.0.0.0] [-Arch x64|arm64] [-Register]

- The identity is in examples\showcase\msix\AppxManifest.xml; the script
  fills in the version and architecture. The Store wants the last part of
  the version to be 0, and each submission's version higher than the last.
- The package is unsigned: the Store signs what it publishes.
- `-Register` installs the unpacked layout for this user instead of
  packing it (needs Developer Mode), so the packaged app can be tried as
  the Store would install it. `Get-AppxPackage *MitsuamiShowcase* |
  Remove-AppxPackage` removes it.

Needs the Windows SDK (makeappx.exe, makepri.exe) and the MSVC build of
the pinned toolchain, as scripts\build-examples.ps1 does.
#>
param(
    [string]$Version = '1.0.0.0',
    [ValidateSet('x64', 'arm64')]
    [string]$Arch = 'x64',
    [switch]$Register
)
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$showcase = Join-Path $root 'examples\showcase'
Set-Location $root

if ($Version -notmatch '^\d+\.\d+\.\d+\.0$') {
    throw "Version '$Version' must be four numbers ending in .0 (the Store reserves the last one)."
}

$channel = (Select-String -Path rust-toolchain.toml -Pattern '^channel\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
$toolchain = "+$channel-x86_64-pc-windows-msvc"
$target = @{ x64 = 'x86_64-pc-windows-msvc'; arm64 = 'aarch64-pc-windows-msvc' }[$Arch]

# The newest Windows SDK that has the packaging tools.
$kits = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
$sdk = Get-ChildItem $kits -Directory -ErrorAction SilentlyContinue |
    Where-Object { Test-Path (Join-Path $_.FullName 'x64\makeappx.exe') } |
    Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
if (-not $sdk) { throw "No Windows SDK with makeappx.exe under $kits" }
$makeappx = Join-Path $sdk.FullName 'x64\makeappx.exe'
$makepri = Join-Path $sdk.FullName 'x64\makepri.exe'

Write-Host "==> Building the showcase for $target (release)"
& cargo $toolchain build --release --manifest-path (Join-Path $showcase 'Cargo.toml') --target $target
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$out = Join-Path $showcase 'target\msix'
$layout = Join-Path $out "layout-$Arch"
if (Test-Path $layout) { Remove-Item -Recurse -Force $layout }
New-Item -ItemType Directory -Force $layout | Out-Null
Copy-Item (Join-Path $showcase "target\$target\release\showcase.exe") $layout
Copy-Item -Recurse (Join-Path $showcase 'msix\Assets') $layout
$manifest = (Get-Content -Raw (Join-Path $showcase 'msix\AppxManifest.xml')).
    Replace('$(Version)', $Version).Replace('$(Architecture)', $Arch)
[IO.File]::WriteAllText((Join-Path $layout 'AppxManifest.xml'), $manifest)

# resources.pri indexes the logos' scale and target-size variants, so the
# taskbar and Start pick the one drawn for their size.
Write-Host '==> Indexing resources'
$priconfig = Join-Path $out 'priconfig.xml'
& $makepri createconfig /cf $priconfig /dq en-US /pv 10.0.0 /o | Out-Null
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& $makepri new /pr $layout /cf $priconfig /mn (Join-Path $layout 'AppxManifest.xml') /of (Join-Path $layout 'resources.pri') /o | Out-Null
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if ($Register) {
    Write-Host '==> Registering the layout for this user'
    Add-AppxPackage -Register (Join-Path $layout 'AppxManifest.xml') -ForceApplicationShutdown
    Write-Host '    Start it from the Start menu: mitsuami showcase'
    exit 0
}

$msix = Join-Path $out "mitsuami-showcase_${Version}_$Arch.msix"
Write-Host '==> Packing'
& $makeappx pack /d $layout /p $msix /o | Out-Null
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host "    -> $msix"
