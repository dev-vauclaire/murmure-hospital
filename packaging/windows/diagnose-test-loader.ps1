[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string] $SourceDirectory,
    [Parameter(Mandatory)]
    [string] $OutputDirectory,
    [Parameter(Mandatory)]
    [ValidatePattern('^[a-z0-9][a-z0-9-]{0,50}$')]
    [string] $Label,
    [string] $DumpbinPath,
    [string] $MtPath,
    [string] $CdbPath,
    [string] $GflagsPath,
    [switch] $TraceOnly,
    [ValidateRange(60, 3600)]
    [int] $CompileTimeoutSeconds = 1800
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Invoke-CapturedProcess {
    param(
        [string] $Executable,
        [string[]] $Arguments,
        [string] $WorkingDirectory,
        [string] $StdoutPath,
        [string] $StderrPath,
        [int] $TimeoutSeconds,
        [switch] $FilterCargoJson
    )

    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $Executable
    $start.WorkingDirectory = $WorkingDirectory
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.StandardOutputEncoding = [Text.Encoding]::UTF8
    $start.StandardErrorEncoding = [Text.Encoding]::UTF8
    foreach ($argument in $Arguments) {
        [void] $start.ArgumentList.Add($argument)
    }

    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $start
    $started = $false
    try {
        [void] $process.Start()
        $started = $true
        # Drain both streams concurrently: compiler output must not fill a pipe.
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        $timedOut = -not $process.WaitForExit($TimeoutSeconds * 1000)
        if ($timedOut) {
            $process.Kill($true)
            if (-not $process.WaitForExit(10000)) {
                throw 'The diagnostic process did not exit after termination.'
            }
        }
        if (-not $stdout.Wait(10000) -or -not $stderr.Wait(10000)) {
            throw 'The diagnostic process output streams did not close.'
        }
        $standardOutput = $stdout.GetAwaiter().GetResult()
        if ($FilterCargoJson) {
            $standardOutput = Remove-CargoEnvironmentValues -Text $standardOutput
        }
        [IO.File]::WriteAllText($StdoutPath, $standardOutput)
        [IO.File]::WriteAllText($StderrPath, $stderr.GetAwaiter().GetResult())
        $exitCode = $process.ExitCode
        $unsignedCode = [BitConverter]::ToUInt32([BitConverter]::GetBytes([int] $exitCode), 0)
        return [pscustomobject] @{
            exit_code = $exitCode
            exit_code_hex = ('0x{0:x8}' -f $unsignedCode)
            timed_out = $timedOut
            stdout_path = $StdoutPath
            stderr_path = $StderrPath
        }
    } finally {
        if ($started -and -not $process.HasExited) {
            $process.Kill($true)
            [void] $process.WaitForExit(10000)
        }
        $process.Dispose()
    }
}

function Remove-CargoEnvironmentValues {
    param([string] $Text)
    $lines = foreach ($line in $Text -split '\r?\n') {
        if ([string]::IsNullOrWhiteSpace($line)) { $line; continue }
        try {
            $message = $line | ConvertFrom-Json -AsHashtable
        } catch {
            $line
            continue
        }
        if ($message['reason'] -eq 'build-script-executed' -and $message.Contains('env')) {
            # Retain native search paths, but never publish compile-time environment values.
            [void] $message.Remove('env')
            ConvertTo-Json -InputObject $message -Depth 100 -Compress
        } else {
            $line
        }
    }
    return ($lines -join [Environment]::NewLine)
}

function Test-DiagnosticTool {
    param([string] $ToolPath)
    return (-not [string]::IsNullOrWhiteSpace($ToolPath) -and
        (Test-Path -LiteralPath $ToolPath -PathType Leaf))
}

function Save-DllInventory {
    param([string] $TestExecutable, [string] $Destination)

    $exeDirectory = Split-Path -Parent $TestExecutable
    $profileDirectory = Split-Path -Parent $exeDirectory
    $windowsDirectory = [Environment]::GetFolderPath([Environment+SpecialFolder]::Windows)
    $candidates = [Collections.Generic.List[object]]::new()
    foreach ($entry in @(
        @{ role = 'test-directory'; directory = $exeDirectory },
        @{ role = 'profile-directory'; directory = $profileDirectory }
    )) {
        if (Test-Path -LiteralPath $entry.directory -PathType Container) {
            foreach ($dll in Get-ChildItem -LiteralPath $entry.directory -Filter '*.dll' -File) {
                $candidates.Add(@{ role = $entry.role; path = $dll.FullName })
            }
        }
    }
    # These are candidates, not a claim that Windows loaded a particular copy.
    foreach ($name in @('DirectML.dll', 'onnxruntime.dll', 'comctl32.dll',
        'msvcp140.dll', 'msvcp140_1.dll', 'msvcp140_2.dll',
        'vcruntime140.dll', 'vcruntime140_1.dll', 'concrt140.dll')) {
        $candidate = Join-Path $windowsDirectory "System32/$name"
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            $candidates.Add(@{ role = 'system32'; path = $candidate })
        }
    }
    $sideBySideDirectory = Join-Path $windowsDirectory 'WinSxS'
    if (Test-Path -LiteralPath $sideBySideDirectory -PathType Container) {
        foreach ($assembly in Get-ChildItem -LiteralPath $sideBySideDirectory -Directory -Filter 'amd64_microsoft.windows.common-controls_*') {
            $candidate = Join-Path $assembly.FullName 'comctl32.dll'
            if (Test-Path -LiteralPath $candidate -PathType Leaf) {
                $candidates.Add(@{ role = 'side-by-side-common-controls'; path = $candidate })
            }
        }
    }

    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $records = @(
        foreach ($candidate in $candidates) {
            if ($seen.Add($candidate.path)) {
                try {
                    $item = Get-Item -LiteralPath $candidate.path
                    [pscustomobject] @{
                        role = $candidate.role
                        path = $item.FullName
                        link_target = $item.LinkTarget
                        file_version = $item.VersionInfo.FileVersion
                        product_version = $item.VersionInfo.ProductVersion
                        bytes = $item.Length
                        sha256 = (Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
                    }
                } catch {
                    [pscustomobject] @{
                        role = $candidate.role
                        path = $candidate.path
                        error = $_.Exception.Message
                    }
                }
            }
        }
    )
    ConvertTo-Json -InputObject @($records) -Depth 5 |
        Set-Content -LiteralPath $Destination -Encoding utf8
    return $records
}

function Get-LoaderFlagWasSet {
    param([string] $ImageName)
    $key = "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options\$ImageName"
    $settings = Get-ItemProperty -LiteralPath $key -Name 'GlobalFlag' -ErrorAction SilentlyContinue
    if ($null -eq $settings -or $null -eq $settings.PSObject.Properties['GlobalFlag']) {
        return $false
    }
    $raw = $settings.GlobalFlag
    if ($raw -is [string]) {
        $flags = [Convert]::ToUInt32(($raw -replace '^0[xX]', ''), 16)
    } else {
        $flags = [int64] $raw
    }
    return (($flags -band 2) -ne 0)
}

function Invoke-LoaderTrace {
    param(
        [string] $TestExecutable,
        [string] $CrateDirectory,
        [string] $CargoPath,
        [string] $Destination,
        [string] $Cdb,
        [string] $Gflags
    )

    $trace = [ordered] @{
        status = 'tools_unavailable'
        log_path = (Join-Path $Destination 'loader.log')
        stdout_path = (Join-Path $Destination 'cdb-stdout.txt')
        stderr_path = (Join-Path $Destination 'cdb-stderr.txt')
        flag_cleanup_exit_code = $null
        context = 'cargo-test-runner'
        error = $null
    }
    if (-not (Test-DiagnosticTool $Cdb) -or -not (Test-DiagnosticTool $Gflags)) {
        return $trace
    }

    $imageName = [IO.Path]::GetFileName($TestExecutable)
    $addedFlag = $false
    try {
        $alreadySet = Get-LoaderFlagWasSet -ImageName $imageName
        $trace['loader_flag_preexisting'] = $alreadySet
        if (-not $alreadySet) {
            # Attempt cleanup even if enabling the flag itself times out.
            $addedFlag = $true
            $enable = Invoke-CapturedProcess -Executable $Gflags -Arguments @('/i', $imageName, '+sls') `
                -WorkingDirectory $Destination -TimeoutSeconds 15 `
                -StdoutPath (Join-Path $Destination 'gflags-enable.txt') `
                -StderrPath (Join-Path $Destination 'gflags-enable-stderr.txt')
            if ($enable.exit_code -ne 0 -or $enable.timed_out) {
                throw 'Could not enable loader snaps; see the GFlags output.'
            }
        }

        $commandPath = Join-Path $Destination 'loader-commands.cdb'
        @'
.lastevent
.exr -1
lmf
sxe -c ".lastevent; .exr -1; lmf; q" 0xc0000139
sxe -c ".lastevent; .exr -1; lmf; q" 0xc0000135
sxe -c ".lastevent; .exr -1; lmf; q" 0xc000007b
g
'@ | Set-Content -LiteralPath $commandPath -Encoding ascii
        $runnerArguments = @($Cdb, '-G', '-nosqm', '-noshell', '-sins', '-netsyms', 'no',
            '-logo', $trace.log_path, '-cf', $commandPath)
        $runnerJson = ConvertTo-Json -InputObject $runnerArguments -Compress
        $runnerConfig = "target.x86_64-pc-windows-msvc.runner=$runnerJson"
        # Cargo adds the test executable and configures its DLL search path and CWD.
        # Frozen resolution reuses the dependency graph from the initial observation.
        # No symbol downloads or memory dumps; the deadline also bounds debugger prompts.
        $run = Invoke-CapturedProcess -Executable $CargoPath `
            -Arguments @('test', '--locked', '--frozen', '--lib', '--config', $runnerConfig, '--', '--list') `
            -WorkingDirectory $CrateDirectory -TimeoutSeconds 120 `
            -StdoutPath $trace.stdout_path -StderrPath $trace.stderr_path
        $trace['cargo_runner_exit_code'] = $run.exit_code
        $trace['debugger_exit_code'] = $null
        $debuggerError = [IO.File]::ReadAllText($trace.stderr_path)
        $debuggerExit = [regex]::Match($debuggerError, '(?i)exit code:\s*(0x[0-9a-f]{8}|-?\d+)')
        if ($debuggerExit.Success) { $trace.debugger_exit_code = $debuggerExit.Groups[1].Value }
        $trace['timed_out'] = $run.timed_out
        $hasLog = (Test-Path -LiteralPath $trace.log_path -PathType Leaf) -and
            (Get-Item -LiteralPath $trace.log_path).Length -gt 0
        if (-not $hasLog) {
            $trace.status = 'failed'
            $trace.error = 'CDB did not produce a non-empty loader trace.'
        } elseif ($run.timed_out) {
            $trace.status = 'timed_out'
        } elseif ($run.exit_code -ne 0) {
            $trace.status = 'failed'
            $trace.error = 'The Cargo debugger runner exited unsuccessfully; inspect its output and loader trace.'
        } else {
            $trace.status = 'completed'
        }
    } catch {
        $trace.status = 'failed'
        $trace.error = $_.Exception.Message
    } finally {
        if ($addedFlag) {
            try {
                $cleanup = Invoke-CapturedProcess -Executable $Gflags -Arguments @('/i', $imageName, '-sls') `
                    -WorkingDirectory $Destination -TimeoutSeconds 15 `
                    -StdoutPath (Join-Path $Destination 'gflags-cleanup.txt') `
                    -StderrPath (Join-Path $Destination 'gflags-cleanup-stderr.txt')
                $trace.flag_cleanup_exit_code = $cleanup.exit_code
                if ($cleanup.exit_code -ne 0 -or $cleanup.timed_out -or (Get-LoaderFlagWasSet -ImageName $imageName)) {
                    throw 'The per-image loader flag could not be removed; see GFlags cleanup output.'
                }
            } catch {
                $trace.status = 'cleanup_failed'
                $trace.error = $_.Exception.Message
            }
        }
    }
    return $trace
}

$output = [IO.Path]::GetFullPath($OutputDirectory)
[void] [IO.Directory]::CreateDirectory($output)
$summaryPath = Join-Path $output 'summary.json'
$inspectionResults = [Collections.Generic.List[object]]::new()
$summary = [ordered] @{
    label = $Label
    source_directory = $SourceDirectory
    compile_exit_code = $null
    compile_timed_out = $false
    compile_stdout = (Join-Path $output 'cargo-artifacts.jsonl')
    compile_stderr = (Join-Path $output 'cargo-stderr.txt')
    executable = $null
    cargo_list_exit_code = $null
    cargo_list_exit_code_hex = $null
    cargo_list_timed_out = $false
    cargo_list_stdout = (Join-Path $output 'cargo-list-stdout.txt')
    cargo_list_stderr = (Join-Path $output 'cargo-list-stderr.txt')
    child_exit_code_hex = $null
    list_exit_code = $null
    list_exit_code_hex = $null
    list_timed_out = $false
    list_stdout = (Join-Path $output 'list-stdout.txt')
    list_stderr = (Join-Path $output 'list-stderr.txt')
    imports_path = $null
    dependents_path = $null
    manifest_path = $null
    dll_inventory_path = (Join-Path $output 'dll-inventory.json')
    tool_inspections_path = (Join-Path $output 'tool-inspections.json')
    loader_trace = $null
    instrumentation_error = $null
}
if ($TraceOnly) {
    # Preserve the original observation even if the second phase cannot start.
    $summary = Get-Content -LiteralPath $summaryPath -Raw | ConvertFrom-Json -AsHashtable
}

try {
    if (-not $IsWindows) { throw 'This diagnostic must run on Windows.' }
    $source = (Resolve-Path -LiteralPath $SourceDirectory).Path
    $crate = Join-Path $source 'src-tauri'
    $cargo = (Get-Command cargo.exe -ErrorAction Stop).Source

    if ($TraceOnly) {
        if ($summary.label -cne $Label -or $summary.source_directory -cne $source) {
            throw 'The existing diagnostic summary belongs to another variant.'
        }
        if ($null -ne $summary.executable) {
            $postToolsPath = Join-Path $output 'dll-inventory-after-debugger-tools.json'
            [void] (Save-DllInventory -TestExecutable $summary.executable -Destination $postToolsPath)
            $summary['post_tools_dll_inventory_path'] = $postToolsPath
        }
    } else {
        $summary.source_directory = $source
        Write-Host "Compiling the $Label test executable without running tests."
        $build = Invoke-CapturedProcess -Executable $cargo `
            -Arguments @('test', '--locked', '--lib', '--no-run', '--message-format=json') `
            -WorkingDirectory $crate -TimeoutSeconds $CompileTimeoutSeconds `
            -StdoutPath $summary.compile_stdout -StderrPath $summary.compile_stderr -FilterCargoJson
        $summary.compile_exit_code = $build.exit_code
        $summary.compile_timed_out = $build.timed_out
        if ($build.exit_code -eq 0 -and -not $build.timed_out) {
            $executables = @(
                foreach ($line in [IO.File]::ReadLines($summary.compile_stdout)) {
                    if ([string]::IsNullOrWhiteSpace($line)) { continue }
                    $artifact = $line | ConvertFrom-Json -AsHashtable
                    if ($artifact['reason'] -eq 'compiler-artifact' -and
                        $artifact['profile']['test'] -eq $true -and
                        $artifact['target']['name'] -eq 'murmure_lib' -and
                        $null -ne $artifact['executable']) {
                        [string] $artifact['executable']
                    }
                }
            )
            if ($executables.Count -ne 1) { throw 'Expected one Murmure library test executable from Cargo.' }
            $summary.executable = (Resolve-Path -LiteralPath $executables[0]).Path

            # Cargo configures the child DLL search path and working directory itself.
            # This is the primary observation, matching the original failing invocation.
            $cargoListing = Invoke-CapturedProcess -Executable $cargo `
                -Arguments @('test', '--locked', '--lib', '--', '--list') `
                -WorkingDirectory $crate -TimeoutSeconds 120 `
                -StdoutPath $summary.cargo_list_stdout -StderrPath $summary.cargo_list_stderr
            $summary.cargo_list_exit_code = $cargoListing.exit_code
            $summary.cargo_list_exit_code_hex = $cargoListing.exit_code_hex
            $summary.cargo_list_timed_out = $cargoListing.timed_out
            $cargoError = [IO.File]::ReadAllText($summary.cargo_list_stderr)
            $childExit = [regex]::Match($cargoError, '(?i)exit code:\s*(0x[0-9a-f]{8})')
            if ($childExit.Success) { $summary.child_exit_code_hex = $childExit.Groups[1].Value.ToLowerInvariant() }

            # First observe the failing environment as-is: do not copy or replace DLLs.
            # Direct execution is secondary: its context differs from Cargo's child.
            $listing = Invoke-CapturedProcess -Executable $summary.executable -Arguments @('--list') `
                -WorkingDirectory (Split-Path -Parent $summary.executable) -TimeoutSeconds 30 `
                -StdoutPath $summary.list_stdout -StderrPath $summary.list_stderr
            $summary.list_exit_code = $listing.exit_code
            $summary.list_exit_code_hex = $listing.exit_code_hex
            $summary.list_timed_out = $listing.timed_out
            $dlls = @(Save-DllInventory -TestExecutable $summary.executable -Destination $summary.dll_inventory_path)

            if (Test-DiagnosticTool $DumpbinPath) {
                $summary.dependents_path = Join-Path $output 'executable-dependents.txt'
                $summary.imports_path = Join-Path $output 'executable-imports.txt'
                foreach ($entry in @(
                    @{ option = '/dependents'; path = $summary.dependents_path },
                    @{ option = '/imports'; path = $summary.imports_path }
                )) {
                    $inspection = Invoke-CapturedProcess -Executable $DumpbinPath -Arguments @('/nologo', $entry.option, $summary.executable) `
                        -WorkingDirectory $output -TimeoutSeconds 30 -StdoutPath $entry.path -StderrPath ($entry.path + '.stderr.txt')
                    $inspectionResults.Add([pscustomobject] @{
                        tool = 'dumpbin'
                        operation = $entry.option
                        input_path = $summary.executable
                        result = $inspection
                    })
                }
                for ($index = 0; $index -lt $dlls.Count; $index++) {
                    $dll = $dlls[$index]
                    $prefix = Join-Path $output ('dll-{0:d3}' -f $index)
                    foreach ($option in @('imports', 'exports')) {
                        $inspection = Invoke-CapturedProcess -Executable $DumpbinPath -Arguments @('/nologo', "/$option", $dll.path) `
                            -WorkingDirectory $output -TimeoutSeconds 30 -StdoutPath "$prefix-$option.txt" -StderrPath "$prefix-$option-stderr.txt"
                        $inspectionResults.Add([pscustomobject] @{
                            tool = 'dumpbin'
                            operation = "/$option"
                            input_path = $dll.path
                            result = $inspection
                        })
                    }
                }
            }
            if (Test-DiagnosticTool $MtPath) {
                $manifest = Join-Path $output 'test-executable.manifest'
                $manifestResult = Invoke-CapturedProcess -Executable $MtPath `
                    -Arguments @('-nologo', "-inputresource:$($summary.executable);#1", "-out:$manifest") `
                    -WorkingDirectory $output -TimeoutSeconds 30 -StdoutPath (Join-Path $output 'manifest-stdout.txt') `
                    -StderrPath (Join-Path $output 'manifest-stderr.txt')
                $summary['manifest_exit_code'] = $manifestResult.exit_code
                $inspectionResults.Add([pscustomobject] @{
                    tool = 'mt'
                    operation = 'extract-manifest'
                    input_path = $summary.executable
                    result = $manifestResult
                })
                if (Test-Path -LiteralPath $manifest -PathType Leaf) { $summary.manifest_path = $manifest }
            }
        }
    }

    $failedToStart = ($null -ne $summary.cargo_list_exit_code -and
        ($summary.cargo_list_exit_code -ne 0 -or $summary.cargo_list_timed_out)) -or
        ($null -ne $summary.list_exit_code -and ($summary.list_exit_code -ne 0 -or $summary.list_timed_out))
    $hasTrace = $null -ne $summary.loader_trace -and
        $summary.loader_trace['status'] -in @('completed', 'timed_out')
    if ($failedToStart -and -not $hasTrace) {
        $summary.loader_trace = Invoke-LoaderTrace -TestExecutable $summary.executable -CrateDirectory $crate `
            -CargoPath $cargo -Destination $output `
            -Cdb $CdbPath -Gflags $GflagsPath
    }
} catch {
    $summary['instrumentation_error'] = $_.Exception.Message
} finally {
    if (-not $TraceOnly) {
        ConvertTo-Json -InputObject $inspectionResults.ToArray() -Depth 6 |
            Set-Content -LiteralPath $summary.tool_inspections_path -Encoding utf8
    }
    $summary | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $summaryPath -Encoding utf8
}

Write-Host "$Label : compile=$($summary.compile_exit_code), cargo-list=$($summary.cargo_list_exit_code), child=$($summary.child_exit_code_hex), direct-list=$($summary.list_exit_code_hex). Diagnostics: $summaryPath"
