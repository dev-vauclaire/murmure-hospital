[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string] $SetupPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$manifestPath = Join-Path $PSScriptRoot '../prerequisites.json'
$dependency = (Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json).vc_redist
$sevenZip = Join-Path $env:ProgramFiles '7-Zip/7z.exe'
if (-not (Test-Path -LiteralPath $sevenZip -PathType Leaf)) {
    throw '7-Zip is required to inspect the generated NSIS setup.'
}

$inspection = Join-Path ([IO.Path]::GetTempPath()) ('murmure-nsis-inspection-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $inspection -ErrorAction Stop | Out-Null
try {
    # Extract only the prerequisite, not the speech model or application files.
    & $sevenZip e '-y' "-o$inspection" (Resolve-Path -LiteralPath $SetupPath).Path 'VC_redist.x64.exe' '-r' | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw 'The generated NSIS setup could not be inspected.'
    }
    $payload = Join-Path $inspection 'VC_redist.x64.exe'
    if (-not (Test-Path -LiteralPath $payload -PathType Leaf)) {
        throw 'The NSIS setup does not contain the Visual C++ prerequisite.'
    }
    $actualHash = (Get-FileHash -LiteralPath $payload -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualHash -cne $dependency.sha256) {
        throw 'The embedded Visual C++ prerequisite differs from its pinned SHA-256.'
    }
    Write-Host 'The NSIS setup contains the verified Visual C++ x64 prerequisite.'
} finally {
    Remove-Item -LiteralPath $inspection -Recurse -Force
}
