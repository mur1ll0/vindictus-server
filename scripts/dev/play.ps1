#requires -Version 7.4
# Game login, step one (B4.2): trade the account's login and password for a one-time play code
# on the admin panel listener, copy it to the clipboard and, with -StartClient, open the
# laboratory client copy. Type (or paste) the code in the ID field of the game's login screen
# and leave the password empty. The client cannot send the password itself: with NGM_disable
# only its ID field reaches the server, and -passport goes through the Nexon SSO.
[CmdletBinding()]
param(
 [Parameter(Mandatory)][ValidatePattern('^[A-Za-z0-9_]{3,32}$')][string]$Username,
 [ValidateRange(1,65535)][int]$AdminPort=27090,
 [switch]$StartClient
)
$ErrorActionPreference='Stop'
$project=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$taskPassword=Read-Host ('Password of '+$Username) -AsSecureString
$taskPointer=[IntPtr]::Zero
try{
 $taskPointer=[Runtime.InteropServices.Marshal]::SecureStringToBSTR($taskPassword)
 $taskBody=@{username=$Username;password=[Runtime.InteropServices.Marshal]::PtrToStringBSTR($taskPointer)}|ConvertTo-Json -Compress
 try{
  $taskResult=Invoke-RestMethod -Method Post -Uri ('http://127.0.0.1:'+$AdminPort+'/api/play/code') -ContentType 'application/json' -Body $taskBody -TimeoutSec 15
 }catch{
  $taskMessage=$null
  try{$taskMessage=($_.ErrorDetails.Message|ConvertFrom-Json).message}catch{}
  if(-not$taskMessage){$taskMessage='admin panel listener unavailable on port '+$AdminPort}
  throw ('Play code refused: '+$taskMessage)
 }
}finally{
 if($taskPointer-ne[IntPtr]::Zero){[Runtime.InteropServices.Marshal]::ZeroFreeBSTR($taskPointer)}
 $taskBody=$null
 $taskPassword.Dispose()
}
Set-Clipboard -Value $taskResult.code
Write-Host ('Play code (copied to the clipboard, valid once for '+[int]($taskResult.expires_in_seconds/60)+' minutes): '+$taskResult.code)
Write-Host 'Type it in the ID field of the game login screen; leave the password empty.'
if($StartClient){
 $client=Join-Path $project '.cache/play-client'
 if(@(Get-Process -Name 'vindictus' -ErrorAction SilentlyContinue).Count-gt0){Write-Host 'The client is already running.'}
 else{Start-Process -FilePath (Join-Path $client 'vindictus.exe') -ArgumentList @('-stage','-dev','-lang','en-eu-x-gm','-noupdate','-threads','4') -WorkingDirectory $client}
}
