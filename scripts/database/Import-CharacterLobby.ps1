param([ValidateSet('Import','Verify')][string]$Mode='Import')
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskPrivate=Join-Path $taskProject '.cache/private-lobby-import'
$taskFile=Join-Path $taskPrivate 'lobby.json'
$taskEvidence=Join-Path $taskProject 'docs/evidence/database/character-lobby-import.json'
function Get-TaskHash([string]$Text){[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($Text))).ToLowerInvariant()}
function Get-TaskExport {
$taskQuery=@"
SET NOCOUNT ON;
SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;
BEGIN TRANSACTION;
SELECT 1 AS format_version,'study_heroes.character-lobby' AS source,
JSON_QUERY(COALESCE((SELECT NexonSN AS nexon_sn,CONVERT(varchar(20),LastLoginCID) AS last_login_cid,ProloguePlayed AS prologue_played FROM dbo.UserInfo ORDER BY NexonSN FOR JSON PATH,INCLUDE_NULL_VALUES),'[]')) AS user_info,
JSON_QUERY(COALESCE((SELECT CONVERT(varchar(20),ID) AS id,NexonSN AS nexon_sn,CONVERT(varchar(max),CONVERT(varbinary(max),ProductName),2) AS product_name_utf16_hex,CONVERT(varchar(max),CONVERT(varbinary(max),Argument),2) AS argument_utf16_hex,CONVERT(varchar(20),DATEDIFF_BIG(day,CONVERT(date,'00010101'),CONVERT(date,ExpireTime))*864000000000+DATEDIFF_BIG(nanosecond,CONVERT(datetime2(7),CONVERT(date,ExpireTime)),ExpireTime)/100) AS expire_time_ticks FROM dbo.UserProducts ORDER BY ID FOR JSON PATH,INCLUDE_NULL_VALUES),'[]')) AS user_products,
JSON_QUERY(COALESCE((SELECT UID AS uid,Type AS type,State AS state,CONVERT(varchar(20),CONVERT(bigint,CONVERT(int,SUBSTRING(CONVERT(binary(8),LastUpdate),1,4)))*25920000+CONVERT(int,SUBSTRING(CONVERT(binary(8),LastUpdate),5,4))) AS last_update_units300 FROM dbo.UserCareInfo ORDER BY UID FOR JSON PATH,INCLUDE_NULL_VALUES),'[]')) AS user_care_info
FOR JSON PATH,WITHOUT_ARRAY_WRAPPER;
COMMIT TRANSACTION;
"@
$taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
$taskResult=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'lobby-readonly-export',$taskQuery)
if($taskResult.ExitCode-ne0){throw 'Read-only reference export failed; private diagnostics suppressed'}
$taskFirst=$taskResult.Stdout.IndexOf('{');$taskLast=$taskResult.Stdout.LastIndexOf('}')
if($taskFirst-lt0-or$taskLast-lt$taskFirst){throw 'Missing reference JSON'}
$taskJson=$taskResult.Stdout.Substring($taskFirst,$taskLast-$taskFirst+1).Replace([string][char]13,'').Replace([string][char]10,'')
$taskData=$taskJson|ConvertFrom-Json -DateKind String
if(@($taskData.user_info).Count-ne25-or@($taskData.user_products).Count-ne0-or@($taskData.user_care_info).Count-ne2){throw 'Reference counts changed; batch refused'}
return [pscustomobject]@{Json=$taskJson;QuerySha256=Get-TaskHash $taskQuery}
}
function Invoke-TaskImport([string]$Json,[bool]$Verify){
 $taskArgs=@('compose','--project-directory',$taskProject,'exec','-T','server','/app/vindictus-server','import-character-lobby','--input-stdin')
 if($Verify){$taskArgs+='--verify-only'}
 $taskResult=Invoke-ProjectDocker -Arguments $taskArgs -InputText $Json
 if($taskResult.ExitCode-ne0){throw 'Character-lobby import/verify failed; private details suppressed'}
 try{$taskSummary=$taskResult.Stdout.Trim()|ConvertFrom-Json -DateKind String}catch{throw 'Invalid sanitized import summary'}
 if(($taskSummary.source_rows-join',')-ne'25,0,2'-or($taskSummary.verified-join',')-ne'25,0,2'-or$taskSummary.game_authentication_available){throw 'Character-lobby summary mismatch'}
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
 if(($taskReplay.inserted-join',')-ne'0,0,0'){throw 'Character-lobby replay inserted unexpected rows'}
 $taskVerified=Invoke-TaskImport $taskExport.Json $true
 $taskAfter=Get-TaskExport
 if((Get-TaskHash $taskAfter.Json)-ne(Get-TaskHash $taskExport.Json)){throw 'Source changed during target preservation; inspect before continuing'}
 $taskReport=[ordered]@{stage='M13.3c';verified_at_utc=[DateTimeOffset]::UtcNow.ToString('o');source_read_only=$true;source_export_unchanged=$true;source_counts=@{user_info=25;user_products=0;user_care_info=2};source_columns=@{user_info=3;user_products=5;user_care_info=4};export_sha256=Get-TaskHash $taskExport.Json;query_sha256=$taskExport.QuerySha256;schema_sha256=(Get-FileHash (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json')).Hash.ToLowerInvariant();private_acl='current Windows user and SYSTEM only';imported=$taskImported;replay=$taskReplay;verified=$taskVerified;post_restart_verified=$false;game_authentication_available=$false}
}else{
 $taskReport=Get-Content $taskEvidence -Raw|ConvertFrom-Json -DateKind String
 $taskJson=[IO.File]::ReadAllText($taskFile)
 if((Get-TaskHash $taskJson)-ne$taskReport.export_sha256){throw 'Private lobby export digest changed'}
 $taskSummary=Invoke-TaskImport $taskJson $true
 $taskReport.post_restart_verified=$true
 $taskReport|Add-Member -Force -NotePropertyName post_restart_verified_at_utc -NotePropertyValue ([DateTimeOffset]::UtcNow.ToString('o'))
 $taskReport|Add-Member -Force -NotePropertyName post_restart_summary -NotePropertyValue $taskSummary
}
$taskReport|ConvertTo-Json -Depth 7|Set-Content $taskEvidence -Encoding utf8NoBOM
Write-Host ('Character-lobby '+$Mode+' PASS:UserInfo25x3/UserProducts0x5/UserCareInfo2x4; typed comparison/replay; private source digest unchanged; authentication=false.')
