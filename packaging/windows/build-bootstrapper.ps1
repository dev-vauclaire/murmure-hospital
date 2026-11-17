[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string] $MsiPath,
    [Parameter(Mandatory)]
    [string] $OutputDirectory,
    [Parameter(Mandatory)]
    [ValidateRange(1, 65535)]
    [int] $BundleRevision,
    [Parameter(Mandatory)]
    [string] $WorkingDirectory
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) {
    throw 'The bootstrapper must be built on Windows.'
}

$repositoryRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$dependencies = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'bootstrapper-dependencies.json') -Raw | ConvertFrom-Json
$package = Get-Content -LiteralPath (Join-Path $repositoryRoot 'package.json') -Raw | ConvertFrom-Json
if ($package.version -notmatch '^\d+\.\d+\.\d+$') {
    throw 'The setup requires a numeric major.minor.patch application version.'
}
$applicationVersion = [version] $package.version
if ($applicationVersion.Major -gt 255 -or $applicationVersion.Minor -gt 255 -or $applicationVersion.Build -gt 65535) {
    throw 'The application version exceeds MSI version limits.'
}
$bundleVersion = "$($package.version).$BundleRevision"
$resolvedMsi = (Resolve-Path -LiteralPath $MsiPath).Path

# Do not wrap a per-user or unexpected MSI. This only reads the package database.
& (Join-Path $PSScriptRoot 'test-bootstrapper.ps1') -MsiPath $resolvedMsi -ExpectedVersion $package.version

# A fresh workspace avoids reusing an unverified executable from a previous build.
$work = New-Item -ItemType Directory -Path $WorkingDirectory -ErrorAction Stop
$output = New-Item -ItemType Directory -Path $OutputDirectory -Force
$setupPath = Join-Path $output.FullName 'Murmure-Hospital-Setup.exe'
if (Test-Path -LiteralPath $setupPath) {
    throw 'The setup output already exists.'
}

function Get-VerifiedDownload {
    param([object] $Dependency, [string] $Destination)

    $uri = [uri] $Dependency.url
    if ($uri.Scheme -ne 'https' -or $Dependency.sha256 -notmatch '^[a-f0-9]{64}$') {
        throw 'Dependencies must have an HTTPS source and a pinned SHA256.'
    }
    Invoke-WebRequest -Uri $uri -OutFile $Destination -ErrorAction Stop
    $actualHash = (Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualHash -cne $Dependency.sha256) {
        throw "Dependency hash mismatch: $([IO.Path]::GetFileName($Destination))"
    }
}

function Assert-MicrosoftSignature {
    param([string] $Path)

    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    if ($signature.Status -ne 'Valid' -or
        $null -eq $signature.SignerCertificate -or
        $signature.SignerCertificate.Subject -notmatch '(^|,\s*)O=Microsoft Corporation(,|$)') {
        throw "The prerequisite must have a valid Microsoft signature: $([IO.Path]::GetFileName($Path))"
    }
}

$wixArchive = Join-Path $work.FullName 'wix.zip'
Get-VerifiedDownload -Dependency $dependencies.wix -Destination $wixArchive
$wixDirectory = Join-Path $work.FullName 'wix'
Expand-Archive -LiteralPath $wixArchive -DestinationPath $wixDirectory -ErrorAction Stop
foreach ($tool in @('candle.exe', 'light.exe', 'dark.exe', 'WixBalExtension.dll', 'WixUtilExtension.dll')) {
    if (-not (Test-Path -LiteralPath (Join-Path $wixDirectory $tool) -PathType Leaf)) {
        throw "The pinned WiX archive is missing $tool."
    }
}

$vcRedistPath = Join-Path $work.FullName 'VC_redist.x64.exe'
Get-VerifiedDownload -Dependency $dependencies.vc_redist -Destination $vcRedistPath
Assert-MicrosoftSignature -Path $vcRedistPath
$vcVersion = [version] (Get-Item -LiteralPath $vcRedistPath).VersionInfo.FileVersion
if ($vcVersion -ne [version] $dependencies.vc_redist.version) {
    throw 'The Visual C++ redistributable version differs from the pinned version.'
}

# Render the existing project license locally; the installer needs no license download.
$licenseText = Get-Content -LiteralPath (Join-Path $repositoryRoot 'LICENSE') -Raw
$licenseRtf = [Text.StringBuilder]::new()
[void] $licenseRtf.Append('{\rtf1\ansi\deff0{\fonttbl{\f0 Consolas;}}\f0\fs16 ')
foreach ($character in $licenseText.ToCharArray()) {
    switch ([int] $character) {
        13 { }
        10 { [void] $licenseRtf.Append('\par ') }
        92 { [void] $licenseRtf.Append('\\') }
        123 { [void] $licenseRtf.Append('\{') }
        125 { [void] $licenseRtf.Append('\}') }
        default {
            if ([int] $character -gt 127) {
                $code = [int] $character
                if ($code -gt 32767) { $code -= 65536 }
                [void] $licenseRtf.Append("\u$code`?")
            } else {
                [void] $licenseRtf.Append($character)
            }
        }
    }
}
[void] $licenseRtf.Append('}')
$licensePath = Join-Path $work.FullName 'license.rtf'
[IO.File]::WriteAllText($licensePath, $licenseRtf.ToString(), [Text.Encoding]::ASCII)

$objectPath = Join-Path $work.FullName 'bootstrapper.wixobj'
$extensions = @('-ext', (Join-Path $wixDirectory 'WixBalExtension.dll'), '-ext', (Join-Path $wixDirectory 'WixUtilExtension.dll'))
$definitions = @(
    "-dBundleVersion=$bundleVersion",
    "-dVcRedistVersion=$($dependencies.vc_redist.version)",
    "-dVcRedistPath=$vcRedistPath",
    "-dMsiPath=$resolvedMsi",
    "-dIconPath=$(Join-Path $repositoryRoot 'src-tauri/icons/icon.ico')",
    "-dLicensePath=$licensePath"
)
& (Join-Path $wixDirectory 'candle.exe') -nologo -arch x64 @extensions @definitions -out $objectPath (Join-Path $PSScriptRoot 'bootstrapper.wxs')
if ($LASTEXITCODE -ne 0) { throw 'WiX could not compile the bootstrapper.' }
& (Join-Path $wixDirectory 'light.exe') -nologo @extensions -pdbout (Join-Path $work.FullName 'bootstrapper.wixpdb') -out $setupPath $objectPath
if ($LASTEXITCODE -ne 0) { throw 'WiX could not link the bootstrapper.' }

# Extract, never execute, the generated setup and check its actual embedded payloads.
& (Join-Path $PSScriptRoot 'test-bootstrapper.ps1') -MsiPath $resolvedMsi -ExpectedVersion $package.version `
    -SetupPath $setupPath -WixDirectory $wixDirectory -VcRedistPath $vcRedistPath `
    -WorkingDirectory (Join-Path $work.FullName 'extracted')

$metadata = [ordered]@{
    bundle_version = $bundleVersion
    setup = [IO.Path]::GetFileName($setupPath)
    setup_sha256 = (Get-FileHash -LiteralPath $setupPath -Algorithm SHA256).Hash.ToLowerInvariant()
    install_scope = 'perMachine'
    msi = [IO.Path]::GetFileName($resolvedMsi)
    msi_sha256 = (Get-FileHash -LiteralPath $resolvedMsi -Algorithm SHA256).Hash.ToLowerInvariant()
    vc_redist = [ordered]@{
        version = $dependencies.vc_redist.version
        url = $dependencies.vc_redist.url
        sha256 = $dependencies.vc_redist.sha256
        microsoft_signature_verified = $true
    }
    webview2 = [ordered]@{
        managed_by = 'tauri'
        install_mode = 'offlineInstaller'
        embedded_in = 'msi'
    }
    wix_version = $dependencies.wix.version
    wix_sha256 = $dependencies.wix.sha256
    signed = $false
}
$metadata | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $output.FullName 'bootstrapper-info.json') -Encoding utf8
Write-Host "Built $setupPath (bundle $bundleVersion); no package has been installed."
