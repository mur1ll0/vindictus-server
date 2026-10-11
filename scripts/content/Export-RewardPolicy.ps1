[CmdletBinding()]
param()
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$taskPrivate=Join-Path $taskProject '.cache/private-content'
New-Item -ItemType Directory -Force -Path $taskPrivate | Out-Null
$taskDirectory=[IO.DirectoryInfo]::new($taskPrivate)
$taskAcl=[IO.FileSystemAclExtensions]::GetAccessControl($taskDirectory,[Security.AccessControl.AccessControlSections]::Access)
$taskAcl.SetAccessRuleProtection($true,$false)
foreach($taskRule in $taskAcl.GetAccessRules($true,$false,[Security.Principal.SecurityIdentifier])){$taskAcl.RemoveAccessRuleSpecific($taskRule)}
$taskIdentity=[Security.Principal.WindowsIdentity]::GetCurrent().User
foreach($taskPrincipal in @($taskIdentity,[Security.Principal.SecurityIdentifier]::new('S-1-5-18'))){
 $taskAcl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($taskPrincipal,'FullControl','ContainerInherit,ObjectInherit','None','Allow'))
}
[IO.FileSystemAclExtensions]::SetAccessControl($taskDirectory,$taskAcl)
Push-Location $taskProject
try { & py -3.13 scripts/content/export_reward_policy.py --output .cache/private-content/reward-policy.json; if($LASTEXITCODE -ne 0){throw 'Private content export failed'} }
finally {Pop-Location}