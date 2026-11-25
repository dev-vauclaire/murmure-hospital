[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$preparationScript = Join-Path $PSScriptRoot '../scripts/prepare-prerequisites.ps1'
$inspectionScript = Join-Path $PSScriptRoot '../scripts/check-setup-prerequisites.ps1'
foreach ($scriptPath in @($PSCommandPath, $preparationScript, $inspectionScript)) {
    $tokens = $null
    $parseErrors = $null
    [void] [Management.Automation.Language.Parser]::ParseFile($scriptPath, [ref] $tokens, [ref] $parseErrors)
    if ($parseErrors.Count -ne 0) {
        throw "Invalid PowerShell syntax in $scriptPath : $($parseErrors -join '; ')"
    }
}
. $preparationScript

$script:Assertions = 0
function Assert-Condition {
    param([bool] $Condition, [string] $Message)
    if (-not $Condition) { throw $Message }
    $script:Assertions += 1
}

function Assert-Throws {
    param([scriptblock] $Action, [string] $Message)
    $failed = $false
    try { & $Action | Out-Null } catch { $failed = $true }
    Assert-Condition -Condition $failed -Message $Message
}

$temporary = Join-Path ([IO.Path]::GetTempPath()) ('murmure-nsis-prerequisite-test-' + [guid]::NewGuid().ToString('N'))
[void] (New-Item -ItemType Directory -Path $temporary)
try {
    $fixture = Join-Path $temporary 'synthetic-payload.txt'
    [IO.File]::WriteAllText($fixture, 'Synthetic text only; never an executable.')
    $hash = (Get-FileHash -LiteralPath $fixture -Algorithm SHA256).Hash.ToLowerInvariant()
    $source = [pscustomobject]@{
        version = '14.52.37029.0'
        url = 'https://download.visualstudio.microsoft.com/synthetic/VC_redist.x64.exe'
        sha256 = $hash
    }
    $requests = [Collections.Generic.List[string]]::new()
    $download = {
        param([string] $Uri, [string] $Path)
        $requests.Add($Uri)
        Copy-Item -LiteralPath $fixture -Destination $Path
    }

    $payload = Join-Path $temporary 'verified.txt'
    Get-VerifiedVcDownload -Dependency $source -Destination $payload -Download $download
    Assert-Condition ((Get-FileHash -LiteralPath $payload -Algorithm SHA256).Hash.ToLowerInvariant() -ceq $hash) 'A matching download must be accepted.'
    Assert-Condition ($requests.Count -eq 1) 'The matching download should be requested once.'

    $modified = [pscustomobject]@{ version = $source.version; url = $source.url; sha256 = ('0' * 64) }
    $rejected = Join-Path $temporary 'rejected.txt'
    Assert-Throws { Get-VerifiedVcDownload -Dependency $modified -Destination $rejected -Download $download } 'A changed payload must fail SHA-256 verification.'
    Assert-Condition (-not (Test-Path -LiteralPath $rejected)) 'A rejected payload must not remain reusable.'

    $requestsBeforeInvalid = $requests.Count
    foreach ($invalid in @(
        [pscustomobject]@{ version = $source.version; url = $source.url.Replace('https://', 'http://'); sha256 = $hash },
        [pscustomobject]@{ version = $source.version; url = $source.url.Replace('download.visualstudio.microsoft.com', 'example.invalid'); sha256 = $hash },
        [pscustomobject]@{ version = $source.version; url = $source.url; sha256 = 'not-pinned' },
        [pscustomobject]@{ version = 'latest'; url = $source.url; sha256 = $hash }
    )) {
        Assert-Throws { Get-VerifiedVcDownload -Dependency $invalid -Destination $rejected -Download $download } 'An invalid source must be rejected.'
    }
    Assert-Condition ($requests.Count -eq $requestsBeforeInvalid) 'Invalid sources must be rejected before requesting anything.'

    $microsoft = { param($Path) [pscustomobject]@{ Status = 'Valid'; SignerCertificate = [pscustomobject]@{ Subject = 'CN=Synthetic, O=Microsoft Corporation, C=US' } } }
    $expectedVersion = { param($Path) '14.52.37029.0' }
    Assert-MicrosoftVcPayload -Path $payload -Dependency $source -SignatureReader $microsoft -VersionReader $expectedVersion

    $invalidSignature = { param($Path) [pscustomobject]@{ Status = 'HashMismatch'; SignerCertificate = [pscustomobject]@{ Subject = 'O=Microsoft Corporation' } } }
    Assert-Throws { Assert-MicrosoftVcPayload -Path $payload -Dependency $source -SignatureReader $invalidSignature -VersionReader $expectedVersion } 'An invalid signature must be rejected.'
    $otherPublisher = { param($Path) [pscustomobject]@{ Status = 'Valid'; SignerCertificate = [pscustomobject]@{ Subject = 'O=Synthetic Publisher' } } }
    Assert-Throws { Assert-MicrosoftVcPayload -Path $payload -Dependency $source -SignatureReader $otherPublisher -VersionReader $expectedVersion } 'Another publisher must be rejected.'
    $unsigned = { param($Path) [pscustomobject]@{ Status = 'NotSigned'; SignerCertificate = $null } }
    Assert-Throws { Assert-MicrosoftVcPayload -Path $payload -Dependency $source -SignatureReader $unsigned -VersionReader $expectedVersion } 'An unsigned payload must be rejected.'
    Assert-Throws { Assert-MicrosoftVcPayload -Path $payload -Dependency $source -SignatureReader $microsoft -VersionReader { param($Path) '14.51.0.0' } } 'A different file version must be rejected.'

    Write-VcPrerequisiteFiles -OutputDirectory $temporary -VcRedistPath $payload -Dependency $source
    $headerPath = Join-Path $temporary 'vc-redist.nsh'
    $header = Get-Content -LiteralPath $headerPath -Raw
    Assert-Condition ($header.Contains('!define HOSPITAL_VC_REDIST_VERSION "14.52.37029.0"')) 'The NSIS include must carry the pinned minimum version.'
    Assert-Condition ($header.Contains('!define HOSPITAL_VC_REDIST_FILE "')) 'The NSIS include must identify the embedded payload.'
    $bytes = [IO.File]::ReadAllBytes($headerPath)
    Assert-Condition (-not ($bytes.Length -ge 3 -and $bytes[0] -eq 0xef -and $bytes[1] -eq 0xbb -and $bytes[2] -eq 0xbf)) 'The NSIS include must be UTF-8 without a BOM.'
    $metadata = Get-Content -LiteralPath (Join-Path $temporary 'prerequisites-info.json') -Raw | ConvertFrom-Json
    Assert-Condition ($metadata.vc_redist.sha256 -ceq $hash) 'Provenance must carry the verified payload hash.'

    $literalPayload = Join-Path $temporary 'synthetic $literal payload.txt'
    Copy-Item -LiteralPath $fixture -Destination $literalPayload
    Write-VcPrerequisiteFiles -OutputDirectory $temporary -VcRedistPath $literalPayload -Dependency $source
    Assert-Condition ((Get-Content -LiteralPath $headerPath -Raw).Contains('$$literal')) 'Literal dollar signs in paths must not become NSIS variables.'
} finally {
    Remove-Item -LiteralPath $temporary -Recurse -Force
}

Write-Host "Visual C++ prerequisite controls passed ($script:Assertions assertions); synthetic files only, no network request or installer execution."
