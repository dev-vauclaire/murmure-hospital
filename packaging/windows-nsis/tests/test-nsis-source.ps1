[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$preparationScript = Join-Path $PSScriptRoot '../scripts/prepare-nsis.ps1'
foreach ($scriptPath in @($PSCommandPath, $preparationScript)) {
    $tokens = $null
    $parseErrors = $null
    [void] [Management.Automation.Language.Parser]::ParseFile($scriptPath, [ref] $tokens, [ref] $parseErrors)
    if ($parseErrors.Count -ne 0) {
        throw "Invalid PowerShell syntax in $scriptPath : $($parseErrors -join '; ')"
    }
}
. $preparationScript
Add-Type -AssemblyName System.IO.Compression.FileSystem

function Assert-Condition {
    param([bool] $Condition, [string] $Message)
    if (-not $Condition) { throw $Message }
}

function Assert-Throws {
    param([scriptblock] $Action, [string] $Message)
    $failed = $false
    try { & $Action | Out-Null } catch { $failed = $true }
    Assert-Condition -Condition $failed -Message $Message
}

$temporary = Join-Path ([IO.Path]::GetTempPath()) ('murmure-nsis-source-test-' + [guid]::NewGuid().ToString('N'))
[void] (New-Item -ItemType Directory -Path $temporary)
try {
    $archiveRoot = Join-Path $temporary 'fixture'
    $toolchain = Join-Path $archiveRoot 'nsis-3.11'
    foreach ($relative in @(
        'makensis.exe', 'Bin/makensis.exe',
        'Stubs/lzma-x86-unicode', 'Stubs/lzma_solid-x86-unicode',
        'Include/MUI2.nsh', 'Include/LogicLib.nsh', 'Include/WordFunc.nsh',
        'Plugins/x86-unicode/System.dll'
    )) {
        $path = Join-Path $toolchain $relative
        [void] (New-Item -ItemType Directory -Path (Split-Path -Parent $path) -Force)
        [IO.File]::WriteAllText($path, 'Synthetic fixture only; never execute.')
    }
    $archive = Join-Path $temporary 'synthetic.zip'
    [IO.Compression.ZipFile]::CreateFromDirectory($archiveRoot, $archive)
    $dependency = [pscustomobject]@{
        version = '3.11'
        url = 'https://github.com/tauri-apps/binary-releases/releases/download/nsis-3.11/nsis-3.11.zip'
        sha256 = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    $destination = Join-Path $temporary 'cache/NSIS'
    Install-VerifiedNsisArchive -ArchivePath $archive -Dependency $dependency -Destination $destination
    Assert-Condition (Test-Path -LiteralPath (Join-Path $destination 'makensis.exe')) 'A matching archive should prepare the toolchain.'
    $marker = Join-Path $destination 'previous-cache.txt'
    [IO.File]::WriteAllText($marker, 'Preserve this cache on validation failure.')

    $invalidHash = [pscustomobject]@{ version = $dependency.version; url = $dependency.url; sha256 = ('0' * 64) }
    Assert-Throws { Install-VerifiedNsisArchive -ArchivePath $archive -Dependency $invalidHash -Destination $destination } 'A modified archive must be rejected.'
    Assert-Condition (Test-Path -LiteralPath $marker) 'A hash failure must preserve the existing cache.'

    Remove-Item -LiteralPath (Join-Path $toolchain 'Include/WordFunc.nsh')
    $incomplete = Join-Path $temporary 'incomplete.zip'
    [IO.Compression.ZipFile]::CreateFromDirectory($archiveRoot, $incomplete)
    $incompleteSource = [pscustomobject]@{
        version = $dependency.version; url = $dependency.url
        sha256 = (Get-FileHash -LiteralPath $incomplete -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    Assert-Throws { Install-VerifiedNsisArchive -ArchivePath $incomplete -Dependency $incompleteSource -Destination $destination } 'An incomplete archive must be rejected.'
    Assert-Condition (Test-Path -LiteralPath $marker) 'An incomplete archive must preserve the existing cache.'

    foreach ($invalid in @(
        [pscustomobject]@{ version = 'latest'; url = $dependency.url; sha256 = $dependency.sha256 },
        [pscustomobject]@{ version = '3.11'; url = $dependency.url.Replace('https://', 'http://'); sha256 = $dependency.sha256 },
        [pscustomobject]@{ version = '3.11'; url = $dependency.url; sha256 = 'not-pinned' }
    )) {
        Assert-Throws { Assert-NsisSource -Dependency $invalid } 'An unpinned source must be rejected.'
    }
} finally {
    Remove-Item -LiteralPath $temporary -Recurse -Force
}

Write-Host 'NSIS source checks passed; synthetic files only, no downloads and no executable run.'
