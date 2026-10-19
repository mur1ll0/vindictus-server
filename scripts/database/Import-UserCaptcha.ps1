param([ValidateSet('Import','Verify')][string]$Mode='Import',[string]$RequireEmptyOwner='lab169_a')
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskPrivate=Join-Path $taskProject '.cache/private-user-captcha'
$taskFile=Join-Path $taskPrivate 'user-captcha.json'
$taskEvidence=Join-Path $taskProject 'docs/evidence/database/user-captcha-import.json'
function Get-TaskHash([string]$Text){[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($Text))).ToLowerInvariant()}
function Get-TaskExport {
 $taskQuery=@"
SET NOCOUNT ON;
SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;
BEGIN TRANSACTION;
SELECT 1 AS format_version,'study_heroes.user-captcha' AS source,
JSON_QUERY((SELECT UID AS uid,PunishedCount AS punished_count,FailureCount AS failure_count,
CONVERT(varchar(20),CONVERT(bigint,CONVERT(int,SUBSTRING(CONVERT(binary(8),LastFailureTime),1,4)))*25920000+CONVERT(int,SUBSTRING(CONVERT(binary(8),LastFailureTime),5,4))) AS last_failure_units300,
CONVERT(varchar(20),CONVERT(bigint,CONVERT(int,SUBSTRING(CONVERT(binary(8),LastSucceedTime),1,4)))*25920000+CONVERT(int,SUBSTRING(CONVERT(binary(8),LastSucceedTime),5,4))) AS last_succeed_units300,
[Condition] AS condition FROM dbo.UserCaptcha ORDER BY UID FOR JSON PATH,INCLUDE_NULL_VALUES)) AS rows
FOR JSON PATH,INCLUDE_NULL_VALUES,WITHOUT_ARRAY_WRAPPER;
COMMIT;
"@
 $sh='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
 $r=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$sh,'readonly-user-captchas-export',$taskQuery)
 if($r.ExitCode-ne0){throw 'Read-only user-captcha export failed;private details suppressed'}
 $a=$r.Stdout.IndexOf('{');$b=$r.Stdout.LastIndexOf('}');if($a-lt0-or$b-lt$a){throw 'Missing user-captcha export'}
 $json=$r.Stdout.Substring($a,$b-$a+1).Replace([string][char]13,'').Replace([string][char]10,'')
 $data=$json|ConvertFrom-Json -DateKind String
 if(@($data.rows).Count-ne24){throw 'UserCaptcha source profile changed;inspect privately'}
 return [pscustomobject]@{Json=$json;QuerySha256=Get-TaskHash $taskQuery}
}
function Invoke-TaskImport([string]$Json,[bool]$Verify){
 $taskArgs=@('compose','--project-directory',$taskProject,'exec','-T','server','/app/vindictus-server','import-user-captcha','--input-stdin','--require-empty-owner',$RequireEmptyOwner);if($Verify){$taskArgs+='--verify-only'}
 $r=Invoke-ProjectDocker -Arguments $taskArgs -InputText $Json
 if($r.ExitCode-ne0){throw 'UserCaptcha preservation failed;private values suppressed'}
 try{$summary=$r.Stdout.Trim()|ConvertFrom-Json -DateKind String}catch{throw 'Invalid sanitized user-captcha summary'}
 if($summary.rows-ne24-or$summary.verified-ne24-or-not$summary.initialized-or-not$summary.empty_owner_verified-or$summary.character_creation_available){throw 'UserCaptcha summary mismatch'}
 return $summary
}
if($Mode-eq'Import'){
 $export=Get-TaskExport
 New-Item -ItemType Directory -Force -Path $taskPrivate|Out-Null
 $acl=[Security.AccessControl.DirectorySecurity]::new();$acl.SetAccessRuleProtection($true,$false)
 foreach($sid in @([Security.Principal.WindowsIdentity]::GetCurrent().User,[Security.Principal.SecurityIdentifier]::new('S-1-5-18'))){$acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($sid,'FullControl','ContainerInherit,ObjectInherit','None','Allow'))}
 [IO.FileSystemAclExtensions]::SetAccessControl([IO.DirectoryInfo]::new($taskPrivate),$acl)
 [IO.File]::WriteAllText($taskFile,$export.Json,[Text.UTF8Encoding]::new($false))
 $imported=Invoke-TaskImport $export.Json $false
 $replay=Invoke-TaskImport $export.Json $false;if($replay.inserted-ne0){throw 'UserCaptcha replay inserted rows'}
 $verified=Invoke-TaskImport $export.Json $true
 $after=Get-TaskExport;if((Get-TaskHash $after.Json)-ne(Get-TaskHash $export.Json)){throw 'UserCaptcha reference changed during read-only preservation'}
 $report=[ordered]@{stage='M15.1b.2e.10b.21a';verified_at_utc=[DateTimeOffset]::UtcNow.ToString('o');source_read_only=$true;source_export_unchanged=$true;source_rows=24;source_columns=6;source_identity=$false;source_foreign_keys=0;empty_owner_verified=$true;export_sha256=Get-TaskHash $export.Json;query_sha256=$export.QuerySha256;schema_sha256=(Get-FileHash (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json')).Hash.ToLowerInvariant();private_acl='current Windows user and SYSTEM only';imported=$imported;replay=$replay;verified=$verified;post_restart_verified=$false;character_creation_available=$false}
}else{
 $report=Get-Content $taskEvidence -Raw|ConvertFrom-Json -DateKind String
 $json=[IO.File]::ReadAllText($taskFile);if((Get-TaskHash $json)-ne$report.export_sha256){throw 'Private user-captcha export digest changed'}
 $verified=Invoke-TaskImport $json $true
 $report.post_restart_verified=$true
 $report|Add-Member -Force -NotePropertyName post_restart_verified_at_utc -NotePropertyValue ([DateTimeOffset]::UtcNow.ToString('o'))
 $report|Add-Member -Force -NotePropertyName post_restart_summary -NotePropertyValue $verified
}
$report|ConvertTo-Json -Depth 7|Set-Content $taskEvidence -Encoding utf8NoBOM
Write-Host ('UserCaptcha '+$Mode+' PASS:24 rows;6 fields;full typed comparison;candidate absence verified;no identity;source unchanged;character creation=false.')
