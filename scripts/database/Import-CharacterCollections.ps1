param([ValidateSet('Import','Verify')][string]$Mode='Import')
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskPrivate=Join-Path $taskProject '.cache/private-character-creation'
$taskFile=Join-Path $taskPrivate 'collections.json'
$taskEvidence=Join-Path $taskProject 'docs/evidence/database/character-collections-import.json'
function Get-TaskHash([string]$Text){[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($Text))).ToLowerInvariant()}
function Get-TaskExport {
$taskQuery=@"
SET NOCOUNT ON;
SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;
BEGIN TRANSACTION;
SELECT 1 AS format_version,'study_heroes.creation-collections' AS source,
JSON_QUERY((SELECT CONVERT(varchar(20),OwnerID) AS owner_id,Permission AS permission FROM dbo.ItemCollection ORDER BY OwnerID FOR JSON PATH,INCLUDE_NULL_VALUES)) AS item_collections,
JSON_QUERY((SELECT CONVERT(varchar(20),OwnerID) AS owner_id,Permission AS permission FROM dbo.QuestCollection ORDER BY OwnerID FOR JSON PATH,INCLUDE_NULL_VALUES)) AS quest_collections,
JSON_QUERY((SELECT CONVERT(varchar(20),OwnerID) AS owner_id,Permission AS permission FROM dbo.MailBox ORDER BY OwnerID FOR JSON PATH,INCLUDE_NULL_VALUES)) AS mailboxes,
JSON_QUERY((SELECT CONVERT(varchar(20),CID) AS cid,Permission AS permission,HintCategory AS hint_category FROM dbo.StoryEntry ORDER BY CID FOR JSON PATH,INCLUDE_NULL_VALUES)) AS story_entries,
JSON_QUERY((SELECT CONVERT(varchar(510),CONVERT(varbinary(255),Code),2) AS code_hex,CONVERT(varchar(510),CONVERT(varbinary(255),Value),2) AS value_hex FROM dbo.MySettings ORDER BY CONVERT(varbinary(255),Code),CONVERT(varbinary(255),Value) FOR JSON PATH,INCLUDE_NULL_VALUES)) AS settings
FOR JSON PATH,WITHOUT_ARRAY_WRAPPER;
COMMIT TRANSACTION;
"@
$taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
$taskResult=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'creation-collections-readonly-export',$taskQuery)
if($taskResult.ExitCode-ne0){throw 'Read-only reference export failed; private diagnostics suppressed'}
$taskFirst=$taskResult.Stdout.IndexOf('{');$taskLast=$taskResult.Stdout.LastIndexOf('}')
if($taskFirst-lt0-or$taskLast-lt$taskFirst){throw 'Missing reference JSON'}
$taskJson=$taskResult.Stdout.Substring($taskFirst,$taskLast-$taskFirst+1).Replace([string][char]13,'').Replace([string][char]10,'')
$taskData=$taskJson|ConvertFrom-Json -DateKind String
foreach($key in 'item_collections','quest_collections','mailboxes','story_entries'){if(@($taskData.$key).Count-ne49){throw 'Reference collection counts changed; inspect before import'}}
if(@($taskData.settings).Count-ne1){throw 'Reference settings count changed; inspect before import'}
return [pscustomobject]@{Json=$taskJson;QuerySha256=Get-TaskHash $taskQuery}
}
function Invoke-TaskImport([string]$Json,[bool]$Verify){
 $taskArgs=@('compose','--project-directory',$taskProject,'exec','-T','server','/app/vindictus-server','import-character-collections','--input-stdin')
 if($Verify){$taskArgs+='--verify-only'}
 $taskResult=Invoke-ProjectDocker -Arguments $taskArgs -InputText $Json
 if($taskResult.ExitCode-ne0){throw 'Creation collection import/verify failed; private details suppressed'}
 try{$taskSummary=$taskResult.Stdout.Trim()|ConvertFrom-Json -DateKind String}catch{throw 'Invalid sanitized import summary'}
 if(($taskSummary.source_rows -join ',')-ne'49,49,49,49,1'-or($taskSummary.verified -join ',')-ne'49,49,49,49,1'-or$taskSummary.character_creation_available){throw 'Creation collection summary mismatch'}
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
 if(@($taskReplay.inserted|Where-Object {$_-ne0}).Count-ne0){throw 'Creation collection replay inserted unexpected rows'}
 $taskVerified=Invoke-TaskImport $taskExport.Json $true
 $taskAfter=Get-TaskExport
 if((Get-TaskHash $taskAfter.Json)-ne(Get-TaskHash $taskExport.Json)){throw 'Source changed during target preservation; inspect before continuing'}
 $taskReport=[ordered]@{stage='M15.1b.1';verified_at_utc=[DateTimeOffset]::UtcNow.ToString('o');source_read_only=$true;source_export_unchanged=$true;source_counts=@{item_collections=49;quest_collections=49;mailboxes=49;story_entries=49;settings=1};source_columns=@{item_collections=2;quest_collections=2;mailboxes=2;story_entries=3;settings=2};export_sha256=Get-TaskHash $taskExport.Json;query_sha256=$taskExport.QuerySha256;schema_sha256=(Get-FileHash (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json')).Hash.ToLowerInvariant();private_acl='current Windows user and SYSTEM only';imported=$taskImported;replay=$taskReplay;verified=$taskVerified;post_restart_verified=$false;character_creation_available=$false}
}else{
 $taskReport=Get-Content $taskEvidence -Raw|ConvertFrom-Json -DateKind String
 $taskJson=[IO.File]::ReadAllText($taskFile)
 if((Get-TaskHash $taskJson)-ne$taskReport.export_sha256){throw 'Private creation collection export digest changed'}
 $taskSummary=Invoke-TaskImport $taskJson $true
 $taskReport.post_restart_verified=$true
 $taskReport|Add-Member -Force -NotePropertyName post_restart_verified_at_utc -NotePropertyValue ([DateTimeOffset]::UtcNow.ToString('o'))
 $taskReport|Add-Member -Force -NotePropertyName post_restart_summary -NotePropertyValue $taskSummary
}
$taskReport|ConvertTo-Json -Depth 7|Set-Content $taskEvidence -Encoding utf8NoBOM
Write-Host ('Creation collection '+$Mode+' PASS:owners49x4/settings1; typed comparison/replay; private source digest unchanged; character creation=false.')
