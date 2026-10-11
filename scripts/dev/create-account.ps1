[CmdletBinding()]
param([Parameter(Mandatory)][ValidatePattern('^[A-Za-z0-9_]{3,32}$')][string]$Username)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'docker-process.ps1')
$taskPassword = Read-Host 'Foundation account password (12-128 UTF-8 bytes)' -AsSecureString
$taskPointer = [IntPtr]::Zero
try {
    $taskPointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($taskPassword)
    $taskPlaintext = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($taskPointer)
    $taskResult = Invoke-ProjectDocker -Arguments @('compose','exec','-T','server','/app/vindictus-server','create-account','--username',$Username,'--password-stdin') -InputText $taskPlaintext
    if ($taskResult.ExitCode -ne 0) { throw ('Provisioning failed: ' + $taskResult.Stderr) }
    Write-Host $taskResult.Stdout.Trim()
} finally {
    if ($taskPointer -ne [IntPtr]::Zero) { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($taskPointer) }
    $taskPlaintext = $null
    $taskPassword.Dispose()
}
