[CmdletBinding()]
param([switch]$DatabaseIntegration, [switch]$DatabaseIntegrationDocker)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$taskProject = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
if ($DatabaseIntegrationDocker -and -not $DatabaseIntegration) {
    throw 'DatabaseIntegrationDocker requires DatabaseIntegration.'
}
if ($DatabaseIntegration -and -not $DatabaseIntegrationDocker -and [string]::IsNullOrWhiteSpace($env:TEST_DATABASE_URL)) {
    throw 'DatabaseIntegration requires TEST_DATABASE_URL pointing to PostgreSQL with CREATE SCHEMA permission. Its value will not be printed.'
}
$taskEvidence = Join-Path $taskProject '.cache\verification'
New-Item -ItemType Directory -Force -Path $taskEvidence | Out-Null
$taskChecks = [Collections.Generic.List[object]]::new()
function Invoke-VerificationCheck {
    param([string]$Name, [string]$Executable, [string[]]$Arguments)
    $taskLog = Join-Path $taskEvidence ($Name + '.log')
    & $Executable @Arguments *> $taskLog
    $taskExit = $LASTEXITCODE
    $taskOutput = [IO.File]::ReadAllText($taskLog)
    $taskPassed = 0; $taskFailed = 0; $taskIgnored = 0
    foreach ($taskMatch in [regex]::Matches($taskOutput, 'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;')) {
        $taskPassed += [int]$taskMatch.Groups[1].Value
        $taskFailed += [int]$taskMatch.Groups[2].Value
        $taskIgnored += [int]$taskMatch.Groups[3].Value
    }
    if ($Name -like '*-python') {
        $taskMatch = [regex]::Match($taskOutput, 'Ran (\d+) tests')
        if ($taskExit -eq 0 -and $taskMatch.Success) { $taskPassed = [int]$taskMatch.Groups[1].Value }
    }
    $taskWarnings = [regex]::Matches($taskOutput, '(?m)^warning:').Count
    $taskChecks.Add([ordered]@{
        name = $Name; command = $Executable + ' ' + ($Arguments -join ' ');
        exit_code = $taskExit; passed = $taskPassed; failed = $taskFailed; ignored = $taskIgnored;
        warnings = $taskWarnings; log = '.cache/verification/' + $Name + '.log';
        log_sha256 = (Get-FileHash -LiteralPath $taskLog -Algorithm SHA256).Hash.ToLowerInvariant()
    })
    $taskSummary = if ($Name -like '*tests*' -or $Name -eq 'postgres-integration' -or $Name -like '*-python') {
        '; passed=' + $taskPassed + '; failed=' + $taskFailed + '; ignored=' + $taskIgnored
    } else { '' }
    Write-Host ($Name + ': exit=' + $taskExit + $taskSummary + '; warnings=' + $taskWarnings)
    if ($taskExit -ne 0) {
        # Keep complete diagnostics locally; only a small failure excerpt goes into the chat.
        $taskLines = [IO.File]::ReadAllLines($taskLog)
        $taskIndex = 0
        for ($taskLineIndex=0; $taskLineIndex -lt $taskLines.Length; $taskLineIndex++) {
            if ($taskLines[$taskLineIndex] -match '^error(\[|:)|^FAIL:|^ERROR:|^failures:') { $taskIndex = $taskLineIndex; break }
        }
        $taskLines | Select-Object -Skip $taskIndex -First 12 | ForEach-Object { Write-Host $_ }
        throw ('Verification failed: ' + $Name + '. Full diagnostics: ' + $taskLog)
    }
}
Push-Location $taskProject
try {
    Invoke-VerificationCheck -Name 'format' -Executable cargo -Arguments @('fmt','--all','--','--check')
    Invoke-VerificationCheck -Name 'clippy' -Executable cargo -Arguments @('clippy','--all-targets','--all-features','--locked','--','-D','warnings')
    Invoke-VerificationCheck -Name 'rust-tests' -Executable cargo -Arguments @('test','--all-targets','--locked')
    Invoke-VerificationCheck -Name 'doc-tests' -Executable cargo -Arguments @('test','--doc','--locked')
    Invoke-VerificationCheck -Name 'build' -Executable cargo -Arguments @('build','--locked')
    if ($DatabaseIntegration) {
        if ($DatabaseIntegrationDocker) {
            $taskCompose = @('compose','-f','compose.yaml','-f','scripts/dev/compose.tests.yaml')
            Invoke-VerificationCheck -Name 'postgres-test-build' -Executable docker -Arguments ($taskCompose + @('build','--quiet','postgres-tests'))
            Invoke-VerificationCheck -Name 'postgres-integration' -Executable docker -Arguments ($taskCompose + @('run','--rm','--no-deps','-T','postgres-tests'))
        } else {
        Invoke-VerificationCheck -Name 'postgres-integration' -Executable cargo -Arguments @('test','--test','postgres','--locked','--','--ignored','--test-threads=1','--nocapture')
        }
    } else {
        Write-Host 'PostgreSQL integration NOT RUN. Use -DatabaseIntegration with TEST_DATABASE_URL.'
    }
    Invoke-VerificationCheck -Name 'client-python' -Executable py -Arguments @('-3.13','-m','unittest','discover','-s','scripts/client','-p','test_prepare_lab_client.py','-v')
    Invoke-VerificationCheck -Name 'content-python' -Executable py -Arguments @('-3.13','-m','unittest','discover','-s','scripts/content','-p','test_export_*.py','-v')
    $taskResult = [ordered]@{
        verified_at_utc = [DateTimeOffset]::UtcNow.ToString('o');
        database_integration_executed = [bool]$DatabaseIntegration;
        database_integration_runtime = $(if ($DatabaseIntegrationDocker) { 'linux-docker-backend' } elseif ($DatabaseIntegration) { 'native' } else { 'not-run' });
        checks = @($taskChecks.ToArray()); game_compatibility_verified = $false
    }
    $taskResult | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $taskProject 'docs/evidence/verification/gate.json') -Encoding utf8
    Write-Host 'PASS: requested checks. Logs saved locally; game compatibility remains unverified.'
} finally { Pop-Location }