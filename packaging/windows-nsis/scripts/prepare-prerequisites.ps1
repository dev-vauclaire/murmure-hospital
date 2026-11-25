[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-VcSource {
    param([Parameter(Mandatory)] [object] $Dependency)

    $uri = [uri] $Dependency.url
    if (-not $uri.IsAbsoluteUri -or $uri.Scheme -ne 'https' -or
        $uri.DnsSafeHost -ne 'download.visualstudio.microsoft.com' -or
        -not $uri.AbsolutePath.EndsWith('/VC_redist.x64.exe') -or
        $uri.Query -ne '' -or $uri.Fragment -ne '' -or $uri.UserInfo -ne '') {
        throw 'Visual C++ must come from a pinned official Microsoft HTTPS URL.'
    }
    if ($Dependency.sha256 -cnotmatch '^[a-f0-9]{64}$') {
        throw 'Visual C++ requires a pinned SHA-256.'
    }
    if ($Dependency.version -notmatch '^\d+\.\d+\.\d+\.\d+$') {
        throw 'Visual C++ requires a numeric four-part version.'
    }
    [void] [version] $Dependency.version
}

function Get-VerifiedVcDownload {
    param(
        [Parameter(Mandatory)] [object] $Dependency,
        [Parameter(Mandatory)] [string] $Destination,
        [scriptblock] $Download = {
            param([string] $Uri, [string] $Path)
            Invoke-WebRequest -Uri $Uri -OutFile $Path -ErrorAction Stop
        }
    )

    Assert-VcSource -Dependency $Dependency
    try {
        & $Download $Dependency.url $Destination | Out-Null
        $actualHash = (Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actualHash -cne $Dependency.sha256) {
            throw 'The Visual C++ download differs from its pinned SHA-256.'
        }
    } catch {
        # A failed check must never leave a payload that a later build could use.
        Remove-Item -LiteralPath $Destination -Force -ErrorAction SilentlyContinue
        throw
    }
}

function Assert-MicrosoftVcPayload {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] [object] $Dependency,
        [scriptblock] $SignatureReader = {
            param([string] $File)
            Get-AuthenticodeSignature -LiteralPath $File -ErrorAction Stop
        },
        [scriptblock] $VersionReader = {
            param([string] $File)
            (Get-Item -LiteralPath $File -ErrorAction Stop).VersionInfo.FileVersion
        }
    )

    $signature = & $SignatureReader $Path
    if ($null -eq $signature -or $signature.Status -ne 'Valid' -or
        $null -eq $signature.SignerCertificate -or
        $signature.SignerCertificate.Subject -notmatch '(^|,\s*)O=Microsoft Corporation(,|$)') {
        throw 'Visual C++ must have a valid Microsoft Authenticode signature.'
    }
    $fileVersion = & $VersionReader $Path
    if ([version] $fileVersion -ne [version] $Dependency.version) {
        throw 'The Visual C++ file version differs from its pinned version.'
    }
}

function Write-VcPrerequisiteFiles {
    param(
        [Parameter(Mandatory)] [string] $OutputDirectory,
        [Parameter(Mandatory)] [string] $VcRedistPath,
        [Parameter(Mandatory)] [object] $Dependency
    )

    # NSIS expands dollar signs even inside quoted strings. Preserve literal paths.
    $literalPath = $VcRedistPath.Replace('$', '$$').Replace('"', '$\"')
    if ($literalPath.Contains("`r") -or $literalPath.Contains("`n")) {
        throw 'The Visual C++ payload path must fit on one NSIS source line.'
    }
    $header = @(
        '; Generated after SHA-256, Microsoft signature and file-version checks.',
        ('!define HOSPITAL_VC_REDIST_VERSION "' + $Dependency.version + '"'),
        ('!define HOSPITAL_VC_REDIST_FILE "' + $literalPath + '"')
    ) -join "`n"
    $encoding = [Text.UTF8Encoding]::new($false)
    [IO.File]::WriteAllText((Join-Path $OutputDirectory 'vc-redist.nsh'), $header + "`n", $encoding)

    $metadata = [ordered]@{
        vc_redist = [ordered]@{
            version = $Dependency.version
            url = $Dependency.url
            sha256 = (Get-FileHash -LiteralPath $VcRedistPath -Algorithm SHA256).Hash.ToLowerInvariant()
            filename = 'VC_redist.x64.exe'
        }
    }
    [IO.File]::WriteAllText((Join-Path $OutputDirectory 'prerequisites-info.json'), ($metadata | ConvertTo-Json -Depth 4) + "`n", $encoding)
}

function Initialize-VcPrerequisite {
    if (-not $IsWindows) {
        throw 'Visual C++ Authenticode verification requires a Windows build runner.'
    }
    $repositoryRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../../..')).Path
    $manifest = Join-Path $PSScriptRoot '../prerequisites.json'
    $dependency = (Get-Content -LiteralPath $manifest -Raw | ConvertFrom-Json).vc_redist
    Assert-VcSource -Dependency $dependency

    $outputDirectory = Join-Path $repositoryRoot 'src-tauri/target/nsis-prerequisites'
    [void] (New-Item -ItemType Directory -Path $outputDirectory -Force)
    # Discard generated include/metadata first: a failed build cannot reuse them.
    foreach ($generated in @('vc-redist.nsh', 'prerequisites-info.json', 'VC_redist.x64.exe')) {
        Remove-Item -LiteralPath (Join-Path $outputDirectory $generated) -Force -ErrorAction SilentlyContinue
    }
    $vcRedistPath = Join-Path $outputDirectory 'VC_redist.x64.exe'
    try {
        Get-VerifiedVcDownload -Dependency $dependency -Destination $vcRedistPath
        Assert-MicrosoftVcPayload -Path $vcRedistPath -Dependency $dependency
        Write-VcPrerequisiteFiles -OutputDirectory $outputDirectory -VcRedistPath $vcRedistPath -Dependency $dependency
        if (-not (Test-Path -LiteralPath (Join-Path $outputDirectory 'vc-redist.nsh') -PathType Leaf)) {
            throw 'The generated NSIS prerequisite include is missing.'
        }
    } catch {
        foreach ($generated in @('vc-redist.nsh', 'prerequisites-info.json', 'VC_redist.x64.exe')) {
            Remove-Item -LiteralPath (Join-Path $outputDirectory $generated) -Force -ErrorAction SilentlyContinue
        }
        throw
    }
    Write-Host "Visual C++ x64 $($dependency.version) verified and prepared for NSIS embedding."
}

# Dot-sourcing exposes only the verification functions to the synthetic tests.
if ($MyInvocation.InvocationName -ne '.') {
    Initialize-VcPrerequisite
}
