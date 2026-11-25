[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-NsisSource {
    param([Parameter(Mandatory)] [object] $Dependency)

    if ($Dependency.version -notmatch '^\d+\.\d+$') {
        throw 'NSIS requires a numeric major.minor version.'
    }
    $expectedUrl = "https://github.com/tauri-apps/binary-releases/releases/download/nsis-$($Dependency.version)/nsis-$($Dependency.version).zip"
    if ($Dependency.url -cne $expectedUrl -or $Dependency.sha256 -cnotmatch '^[a-f0-9]{64}$') {
        throw 'NSIS requires its pinned official Tauri archive URL and SHA-256.'
    }
}

function Install-VerifiedNsisArchive {
    param(
        [Parameter(Mandatory)] [string] $ArchivePath,
        [Parameter(Mandatory)] [object] $Dependency,
        [Parameter(Mandatory)] [string] $Destination
    )

    Assert-NsisSource -Dependency $Dependency
    $actualHash = (Get-FileHash -LiteralPath $ArchivePath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualHash -cne $Dependency.sha256) {
        throw 'The NSIS archive differs from its pinned SHA-256.'
    }

    $staging = Join-Path ([IO.Path]::GetTempPath()) ('murmure-nsis-toolchain-' + [guid]::NewGuid().ToString('N'))
    [void] (New-Item -ItemType Directory -Path $staging)
    try {
        Expand-Archive -LiteralPath $ArchivePath -DestinationPath $staging -ErrorAction Stop
        $toolchain = Join-Path $staging "nsis-$($Dependency.version)"
        foreach ($relative in @(
            'makensis.exe', 'Bin/makensis.exe',
            'Stubs/lzma-x86-unicode', 'Stubs/lzma_solid-x86-unicode',
            'Include/MUI2.nsh', 'Include/LogicLib.nsh', 'Include/WordFunc.nsh',
            'Plugins/x86-unicode/System.dll'
        )) {
            if (-not (Test-Path -LiteralPath (Join-Path $toolchain $relative) -PathType Leaf)) {
                throw "The verified NSIS archive is missing a required file: $relative"
            }
        }

        # Validate the replacement before touching a previous tool cache.
        $parent = Split-Path -Parent $Destination
        [void] (New-Item -ItemType Directory -Path $parent -Force)
        if (Test-Path -LiteralPath $Destination) {
            Remove-Item -LiteralPath $Destination -Recurse -Force
        }
        Move-Item -LiteralPath $toolchain -Destination $Destination -ErrorAction Stop
    } finally {
        Remove-Item -LiteralPath $staging -Recurse -Force
    }
}

function Initialize-NsisToolchain {
    if (-not $IsWindows) {
        throw 'This NSIS archive prepares the Windows CI toolchain.'
    }
    $manifest = Join-Path $PSScriptRoot '../nsis-source.json'
    $dependency = Get-Content -LiteralPath $manifest -Raw | ConvertFrom-Json
    Assert-NsisSource -Dependency $dependency

    $temporary = Join-Path ([IO.Path]::GetTempPath()) ('murmure-nsis-download-' + [guid]::NewGuid().ToString('N'))
    [void] (New-Item -ItemType Directory -Path $temporary)
    try {
        $archive = Join-Path $temporary "nsis-$($dependency.version).zip"
        Invoke-WebRequest -Uri $dependency.url -OutFile $archive -ErrorAction Stop
        $destination = Join-Path $env:LOCALAPPDATA 'tauri/NSIS'
        Install-VerifiedNsisArchive -ArchivePath $archive -Dependency $dependency -Destination $destination
        $compiler = Join-Path $destination 'makensis.exe'
        $version = & $compiler '-VERSION'
        if ($LASTEXITCODE -ne 0 -or $version.Trim() -cne "v$($dependency.version)") {
            throw 'The prepared NSIS compiler version is not the pinned version.'
        }
        Write-Host "NSIS $($dependency.version) prepared from the verified official archive."
    } finally {
        Remove-Item -LiteralPath $temporary -Recurse -Force
    }
}

# Synthetic tests import the functions without downloading or running tools.
if ($MyInvocation.InvocationName -ne '.') {
    Initialize-NsisToolchain
}
