param([ValidateSet('Import','Verify')][string]$Mode='Import')
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskPrivate=Join-Path $taskProject '.cache/private-home-cafe-import'
$taskFile=Join-Path $taskPrivate 'home-cafe.json'
$taskEvidence=Join-Path $taskProject 'docs/evidence/database/home-cafe-bonus-import.json'
function Get-TaskHash([string]$Text){[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($Text))).ToLowerInvariant()}
function Get-TaskExport {
$taskQuery=@"
SET NOCOUNT ON;
SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;
BEGIN TRANSACTION;
SELECT 1 AS format_version,'study_heroes.home-cafe-bonus' AS source,
JSON_QUERY(COALESCE((SELECT UID AS uid,CONVERT(varchar(20),CID) AS cid,BonusType AS bonus_type,CONVERT(varchar(20),CONVERT(bigint,CONVERT(int,SUBSTRING(CONVERT(binary(8),ExpiredTime),1,4)))*25920000+CONVERT(int,SUBSTRING(CONVERT(binary(8),ExpiredTime),5,4))) AS expired_time_units300 FROM dbo.HomeCafeBonus ORDER BY UID,CID,BonusType FOR JSON PATH,INCLUDE_NULL_VALUES),'[]')) AS home_cafe_bonus
FOR JSON PATH,WITHOUT_ARRAY_WRAPPER;
COMMIT TRANSACTION;
"@
$taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
$taskResult=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'home-cafe-readonly-export',$taskQuery)
if($taskResult.ExitCode-ne0){throw 'Read-only reference export failed; private diagnostics suppressed'}
$taskFirst=$taskResult.Stdout.IndexOf('{');$taskLast=$taskResult.Stdout.LastIndexOf('}')
if($taskFirst-lt0-or$taskLast-lt$taskFirst){throw 'Missing reference JSON'}
$taskJson=$taskResult.Stdout.Substring($taskFirst,$taskLast-$taskFirst+1).Replace([string][char]13,'').Replace([string][char]10,'')
$taskData=$taskJson|ConvertFrom-Json -DateKind String
if(@($taskData.home_cafe_bonus).Count-ne2){throw 'Reference counts changed; batch refused'}
return [pscustomobject]@{Json=$taskJson;QuerySha256=Get-TaskHash $taskQuery}
}
function Invoke-TaskImport([string]$Json,[bool]$Verify){
 $taskArgs=@('compose','--project-directory',$taskProject,'exec','-T','server','/app/vindictus-server','import-home-cafe-bonus','--input-stdin')
 if($Verify){$taskArgs+='--verify-only'}
 $taskResult=Invoke-ProjectDocker -Arguments $taskArgs -InputText $Json
 if($taskResult.ExitCode-ne0){throw 'Home-cafe import/verify failed; private details suppressed'}
 try{$taskSummary=$taskResult.Stdout.Trim()|ConvertFrom-Json -DateKind String}catch{throw 'Invalid sanitized import summary'}
 if($taskSummary.source_rows-ne2-or$taskSummary.verified-ne2-or$taskSummary.game_authentication_available){throw 'Home-cafe summary mismatch'}
 return $taskSummary
}
if($Mode-eq'Import'){
 $taskExport=Get-TaskExport
 New-Item -ItemType Directory -Force -Path $taskPrivate|Out-Null
 $taskAcl=[Security.AccessControl.DirectorySecurity]::new();$taskAcl.SetAccessRuleProtection($true,$false)
 foreach($taskSid in @([Security.Principal.WindowsIdentity]::GetCurrent().User,[Security.Principal.SecurityIdentifier]::new('S-1-5-18'))){$taskAcl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($taskSid,'FullControl','ContainerInherit,ObjectInherit','None','Allow'))}
 # Modify the DACL only; Set-Acl also requests audit privileges unavailable to this user.
 [IO.FileSystemAclExtensions]::SetAccessControl([IO.DirectoryInfo]::new($taskPrivate),$taskAcl)
 [IO.File]::WriteAllText($taskFile,$taskExport.Json,[Text.UTF8Encoding]::new($false))
 $taskImported=Invoke-TaskImport $taskExport.Json $false
 $taskReplay=Invoke-TaskImport $taskExport.Json $false
 if($taskReplay.inserted-ne0){throw 'Home-cafe replay inserted unexpected rows'}
 $taskVerified=Invoke-TaskImport $taskExport.Json $true
 $taskAfter=Get-TaskExport
 if((Get-TaskHash $taskAfter.Json)-ne(Get-TaskHash $taskExport.Json)){throw 'Source changed during target preservation; inspect before continuing'}
 $taskReport=[ordered]@{stage='M13.3d.1';verified_at_utc=[DateTimeOffset]::UtcNow.ToString('o');source_read_only=$true;source_export_unchanged=$true;source_counts=@{home_cafe_bonus=2};source_columns=@{home_cafe_bonus=4};export_sha256=Get-TaskHash $taskExport.Json;query_sha256=$taskExport.QuerySha256;schema_sha256=(Get-FileHash (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json')).Hash.ToLowerInvariant();private_acl='current Windows user and SYSTEM only';imported=$taskImported;replay=$taskReplay;verified=$taskVerified;post_restart_verified=$false;game_authentication_available=$false}
}else{
 $taskReport=Get-Content $taskEvidence -Raw|ConvertFrom-Json -DateKind String
 $taskJson=[IO.File]::ReadAllText($taskFile)
 if((Get-TaskHash $taskJson)-ne$taskReport.export_sha256){throw 'Private home-cafe export digest changed'}
 $taskSummary=Invoke-TaskImport $taskJson $true
 $taskReport.post_restart_verified=$true
 $taskReport|Add-Member -Force -NotePropertyName post_restart_verified_at_utc -NotePropertyValue ([DateTimeOffset]::UtcNow.ToString('o'))
 $taskReport|Add-Member -Force -NotePropertyName post_restart_summary -NotePropertyValue $taskSummary
}
$taskReport|ConvertTo-Json -Depth 7|Set-Content $taskEvidence -Encoding utf8NoBOM
Write-Host ('Home-cafe '+$Mode+' PASS:HomeCafeBonus2x4; typed comparison/replay; private source digest unchanged; authentication=false.')
