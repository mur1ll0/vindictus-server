#requires -Version 7.4
# Give, change or remove (none) the admin panel role of an existing account (audited). Runs inside the server container like create-account.ps1; no password is involved.
[CmdletBinding()]
param(
 [Parameter(Mandatory)][ValidatePattern('^[A-Za-z0-9_]{3,32}$')][string]$Username,
 [Parameter(Mandatory)][ValidateSet('admin','gm','leitura','none')][string]$Role
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'docker-process.ps1')
$taskResult=Invoke-ProjectDocker -Arguments @('compose','exec','-T','server','/app/vindictus-server','account-set-role','--username',$Username,'--role',$Role)
if($taskResult.ExitCode-ne0){throw ('Role change failed: '+$taskResult.Stderr)}
Write-Host $taskResult.Stdout.Trim()
