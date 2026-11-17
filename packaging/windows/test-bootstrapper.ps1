[CmdletBinding(DefaultParameterSetName = 'Package')]
param(
    [Parameter(Mandatory, ParameterSetName = 'Package')]
    [string] $MsiPath,
    [Parameter(Mandatory, ParameterSetName = 'Package')]
    [string] $ExpectedVersion,
    [Parameter(ParameterSetName = 'Package')]
    [string] $SetupPath,
    [Parameter(ParameterSetName = 'Package')]
    [string] $WixDirectory,
    [Parameter(ParameterSetName = 'Package')]
    [string] $VcRedistPath,
    [Parameter(ParameterSetName = 'Package')]
    [string] $WorkingDirectory,
    [Parameter(Mandatory, ParameterSetName = 'Syntax')]
    [switch] $SyntaxOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-Condition {
    param([bool] $Condition, [string] $Message)
    if (-not $Condition) { throw $Message }
}

function Assert-Throws {
    param([scriptblock] $Action, [string] $Message)
    $failed = $false
    try { & $Action } catch { $failed = $true }
    Assert-Condition $failed $Message
}

function Test-VerifiedDownload {
    param([System.Management.Automation.Language.ScriptBlockAst] $BuildAst)

    $functions = @($BuildAst.FindAll({
        param($node)
        $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and
        $node.Name -eq 'Get-VerifiedDownload'
    }, $true))
    Assert-Condition ($functions.Count -eq 1) 'Expected exactly one verified-download implementation.'
    # Load only this function, never the build script's download/build commands.
    . ([scriptblock]::Create($functions[0].Extent.Text))

    $temporary = Join-Path ([IO.Path]::GetTempPath()) ('murmure-bootstrapper-test-' + [guid]::NewGuid().ToString('N'))
    [void] (New-Item -ItemType Directory -Path $temporary)
    try {
        $fixture = Join-Path $temporary 'synthetic-payload.txt'
        [IO.File]::WriteAllText($fixture, 'Synthetic installer payload; never executable.')
        $hash = (Get-FileHash -LiteralPath $fixture -Algorithm SHA256).Hash.ToLowerInvariant()
        $requests = [Collections.Generic.List[uri]]::new()

        function Invoke-WebRequest {
            [CmdletBinding()]
            param([uri] $Uri, [string] $OutFile)
            $requests.Add($Uri)
            Copy-Item -LiteralPath $fixture -Destination $OutFile
        }

        $valid = [pscustomobject] @{ url = 'https://example.invalid/synthetic'; sha256 = $hash }
        $validDestination = Join-Path $temporary 'valid.txt'
        Get-VerifiedDownload -Dependency $valid -Destination $validDestination
        Assert-Condition ((Get-FileHash -LiteralPath $validDestination -Algorithm SHA256).Hash.ToLowerInvariant() -ceq $hash) 'A valid pinned payload must be accepted.'

        $wrongHash = [pscustomobject] @{ url = $valid.url; sha256 = ('0' * 64) }
        Assert-Throws { Get-VerifiedDownload -Dependency $wrongHash -Destination (Join-Path $temporary 'wrong.txt') } 'A modified payload must be rejected.'

        $requestsBeforeInsecureSource = $requests.Count
        $insecure = [pscustomobject] @{ url = 'http://example.invalid/synthetic'; sha256 = $hash }
        Assert-Throws { Get-VerifiedDownload -Dependency $insecure -Destination (Join-Path $temporary 'insecure.txt') } 'An insecure source must be rejected.'
        Assert-Condition ($requests.Count -eq $requestsBeforeInsecureSource) 'An insecure source must be rejected before any request.'

        $invalidHash = [pscustomobject] @{ url = $valid.url; sha256 = 'not-a-sha256' }
        Assert-Throws { Get-VerifiedDownload -Dependency $invalidHash -Destination (Join-Path $temporary 'invalid.txt') } 'An unpinned payload must be rejected.'
        Assert-Condition ($requests.Count -eq $requestsBeforeInsecureSource) 'An invalid pin must be rejected before any request.'
    } finally {
        Remove-Item -LiteralPath $temporary -Recurse -Force
    }
    Write-Host 'Verified-download tests passed using synthetic files only; no network request or installer execution.'
}

$buildAst = $null
foreach ($script in Get-ChildItem -LiteralPath $PSScriptRoot -Filter '*.ps1' -File) {
    $tokens = $null
    $parseErrors = $null
    $ast = [Management.Automation.Language.Parser]::ParseFile($script.FullName, [ref] $tokens, [ref] $parseErrors)
    Assert-Condition ($parseErrors.Count -eq 0) "Invalid PowerShell syntax in $($script.Name): $($parseErrors -join '; ')"
    if ($script.Name -eq 'build-bootstrapper.ps1') { $buildAst = $ast }
}
Assert-Condition ($null -ne $buildAst) 'The bootstrapper builder script is missing.'
Test-VerifiedDownload -BuildAst $buildAst
if ($SyntaxOnly) { return }
if (-not $IsWindows) { throw 'MSI database inspection requires Windows. Use -SyntaxOnly for portable checks.' }

$extractionOptions = @($SetupPath, $WixDirectory, $VcRedistPath, $WorkingDirectory)
$specified = @($extractionOptions | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }).Count
Assert-Condition ($specified -eq 0 -or $specified -eq 4) 'SetupPath, WixDirectory, VcRedistPath and WorkingDirectory must be supplied together.'
$resolvedMsi = (Resolve-Path -LiteralPath $MsiPath).Path

function Read-MsiProperty {
    param([object] $Database, [string] $Name)
    $view = $null
    $record = $null
    try {
        $query = 'SELECT `Value` FROM `Property` WHERE `Property` = ' + "'$Name'"
        $view = $Database.OpenView($query)
        [void] $view.Execute()
        $record = $view.Fetch()
        if ($null -eq $record) { return $null }
        return [string] $record.StringData(1)
    } finally {
        if ($null -ne $record) { [void] [Runtime.InteropServices.Marshal]::FinalReleaseComObject($record) }
        if ($null -ne $view) {
            [void] $view.Close()
            [void] [Runtime.InteropServices.Marshal]::FinalReleaseComObject($view)
        }
    }
}

$installer = $null
$database = $null
$summary = $null
try {
    $installer = New-Object -ComObject WindowsInstaller.Installer
    $database = $installer.OpenDatabase($resolvedMsi, 0)
    Assert-Condition ((Read-MsiProperty -Database $database -Name 'ALLUSERS') -ceq '1') 'The MSI must install for all users (ALLUSERS=1).'
    Assert-Condition ((Read-MsiProperty -Database $database -Name 'ProductVersion') -ceq $ExpectedVersion) 'The MSI application version differs from the expected version.'
    # Tauri must use the embedded standalone installer, never a network bootstrapper.
    $standaloneFound = $false
    $actions = $database.OpenView('SELECT `Action` FROM `CustomAction`')
    try {
        [void] $actions.Execute()
        while ($null -ne ($action = $actions.Fetch())) {
            try {
                $name = [string] $action.StringData(1)
                Assert-Condition ($name -notmatch 'Bootstrapper|UpdateWebView2ViaEdgeUpdate') "Unexpected network installation action in the MSI: $name"
                if ($name -ceq 'InvokeStandalone') { $standaloneFound = $true }
            } finally {
                [void] [Runtime.InteropServices.Marshal]::FinalReleaseComObject($action)
            }
        }
    } finally {
        [void] $actions.Close()
        [void] [Runtime.InteropServices.Marshal]::FinalReleaseComObject($actions)
    }
    Assert-Condition $standaloneFound 'The MSI must install the embedded WebView2 standalone runtime.'
    $binaryView = $database.OpenView('SELECT `Name` FROM `Binary` WHERE `Name` = ''MicrosoftEdgeWebView2RuntimeInstaller.exe''')
    $binaryRecord = $null
    try {
        [void] $binaryView.Execute()
        $binaryRecord = $binaryView.Fetch()
        Assert-Condition ($null -ne $binaryRecord) 'The offline WebView2 installer is missing from the MSI Binary table.'
    } finally {
        if ($null -ne $binaryRecord) { [void] [Runtime.InteropServices.Marshal]::FinalReleaseComObject($binaryRecord) }
        [void] $binaryView.Close()
        [void] [Runtime.InteropServices.Marshal]::FinalReleaseComObject($binaryView)
    }
    $summary = $database.SummaryInformation(0)
    $architecture = [string] $summary.Property(7)
    Assert-Condition ($architecture.Split(';')[0] -ceq 'x64') 'The MSI must target x64.'
} finally {
    foreach ($comObject in @($summary, $database, $installer)) {
        if ($null -ne $comObject) { [void] [Runtime.InteropServices.Marshal]::FinalReleaseComObject($comObject) }
    }
}
Write-Host "MSI database checked without installation: x64, perMachine, version $ExpectedVersion."

if ($specified -eq 4) {
    $setup = (Resolve-Path -LiteralPath $SetupPath).Path
    $vcRedist = (Resolve-Path -LiteralPath $VcRedistPath).Path
    $dark = (Resolve-Path -LiteralPath (Join-Path $WixDirectory 'dark.exe')).Path
    Assert-Condition (-not (Test-Path -LiteralPath $WorkingDirectory)) 'Extraction requires a fresh output directory.'
    & $dark -nologo -x $WorkingDirectory $setup
    if ($LASTEXITCODE -ne 0) { throw 'WiX could not extract the generated setup.' }
    $container = Join-Path $WorkingDirectory 'AttachedContainer'
    foreach ($payload in @(
        @{ Name = 'Murmure.msi'; Source = $resolvedMsi },
        @{ Name = 'VC_redist.x64.exe'; Source = $vcRedist }
    )) {
        $matches = @(Get-ChildItem -LiteralPath $container -File -Recurse | Where-Object { $_.Name -ceq $payload.Name })
        Assert-Condition ($matches.Count -eq 1) "The setup must contain exactly one $($payload.Name) payload."
        $expectedHash = (Get-FileHash -LiteralPath $payload.Source -Algorithm SHA256).Hash
        $actualHash = (Get-FileHash -LiteralPath $matches[0].FullName -Algorithm SHA256).Hash
        Assert-Condition ($actualHash -ceq $expectedHash) "The embedded $($payload.Name) differs from its verified source."
    }
    Write-Host 'Both Burn payloads match their source hashes; neither has been executed.'
}
