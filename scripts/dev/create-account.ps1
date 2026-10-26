[CmdletBinding()]
# -LabShortPassword (operator decision 2026-10-25): accept 5+ bytes for a loopback laboratory account.
param([Parameter(Mandatory)][ValidatePattern('^[A-Za-z0-9_]{3,32}$')][string]$Username,[switch]$LabShortPassword)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'docker-process.ps1')
$taskPassword = Read-Host ($(if ($LabShortPassword) { 'Account password (5-128 UTF-8 bytes, laboratory only)' } else { 'Account password (12-128 UTF-8 bytes)' })) -AsSecureString
$taskPointer = [IntPtr]::Zero
try {
    $taskPointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($taskPassword)
    $taskPlaintext = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($taskPointer)
    $taskResult = Invoke-ProjectDocker -Arguments (@('compose','exec','-T','server','/app/vindictus-server','create-account','--username',$Username,'--password-stdin') + $(if ($LabShortPassword) { @('--lab-short-password') } else { @() })) -InputText $taskPlaintext
    if ($taskResult.ExitCode -ne 0) { throw ('Provisioning failed: ' + $taskResult.Stderr) }
    Write-Host $taskResult.Stdout.Trim()
} finally {
    if ($taskPointer -ne [IntPtr]::Zero) { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($taskPointer) }
    $taskPlaintext = $null
    $taskPassword.Dispose()
}
