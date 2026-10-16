param([ValidateSet('Import','Verify')][string]$Mode='Import',[switch]$ExportOnly)
$ErrorActionPreference='Stop'
if($ExportOnly-and$Mode-ne'Import'){throw 'ExportOnly requires Import mode'}
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskPrivate=Join-Path $taskProject '.cache/private-character-state'
$taskFile=Join-Path $taskPrivate 'character-state.json'
$taskEvidence=Join-Path $taskProject 'docs/evidence/database/character-state-import.json'
function Get-TaskHash([string]$Text){[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($Text))).ToLowerInvariant()}
function Get-TaskExport {
 $taskQuery=@"
SET NOCOUNT ON;
SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;
BEGIN TRANSACTION;
SELECT 1 AS format_version,'study_heroes.character-initialization' AS source,
JSON_QUERY(ISNULL((SELECT CONVERT(varchar(20),CID) AS cid,TitleID AS title_id,Acquired AS acquired,CONVERT(varchar(16),CONVERT(varbinary(8),AcquiredTime),2) AS acquired_time,CONVERT(varchar(max),CONVERT(varbinary(max),AcquiredQuest),2) AS acquired_quest_utf16,CONVERT(varchar(20),DATEDIFF_BIG(day,CONVERT(date,'00010101'),CONVERT(date,ExpireDateTime))*864000000000+DATEDIFF_BIG(nanosecond,CONVERT(datetime2(7),CONVERT(date,ExpireDateTime)),ExpireDateTime)/100) AS expire_ticks FROM dbo.Title ORDER BY CID,TitleID FOR JSON PATH,INCLUDE_NULL_VALUES),'[]')) AS titles,
JSON_QUERY(ISNULL((SELECT CONVERT(varchar(20),CID) AS cid,TitleGoalID AS title_goal_id,Progress AS progress FROM dbo.TitleGoalProgress ORDER BY CID,TitleGoalID FOR JSON PATH),'[]')) AS goals,
JSON_QUERY(ISNULL((SELECT TitleID AS title_id,ClassRestriction AS class_restriction FROM dbo.TitlePreset ORDER BY TitleID FOR JSON PATH),'[]')) AS presets,
JSON_QUERY(ISNULL((SELECT CONVERT(varchar(20),CID) AS cid,MicroPlayCount AS micro_play_count,CONVERT(varchar(20),MicroPlayTime) AS micro_play_time,SuccessMicroPlayCount AS success_micro_play_count,CONVERT(varchar(20),SuccessMicroPlayTime) AS success_micro_play_time,DailyMicroPlayCount AS daily_micro_play_count,DailyFreePlayCount AS daily_free_play_count,CONVERT(varchar(16),CONVERT(varbinary(8),LastMicroPlayStarted),2) AS last_micro_play_started,DailyReportCount AS daily_report_count,CONVERT(varchar(16),CONVERT(varbinary(8),LastReportedTime),2) AS last_reported_time,CONVERT(varchar(max),CONVERT(varbinary(max),CurrentQuest),2) AS current_quest_utf16,ReportReceivedCount AS report_received_count FROM dbo.MicroPlayStatus ORDER BY CID FOR JSON PATH,INCLUDE_NULL_VALUES),'[]')) AS micro,
JSON_QUERY(ISNULL((SELECT CONVERT(varchar(20),CID) AS cid,CompletedMissionCount AS completed_mission_count,CONVERT(varchar(20),DATEDIFF_BIG(day,CONVERT(date,'00010101'),CONVERT(date,LastClearTime))*864000000000+DATEDIFF_BIG(nanosecond,CONVERT(datetime2(7),CONVERT(date,LastClearTime)),LastClearTime)/100) AS last_clear_ticks FROM dbo.CharacterMissionInfo ORDER BY CID FOR JSON PATH,INCLUDE_NULL_VALUES),'[]')) AS missions,
JSON_QUERY(ISNULL((SELECT CONVERT(varchar(20),CID) AS cid,CONVERT(varchar(20),ID) AS id,CONVERT(varchar(max),CONVERT(varbinary(max),MID),2) AS mission_utf16,CompletionProgress AS completion_progress,CONVERT(varchar(20),DATEDIFF_BIG(day,CONVERT(date,'00010101'),CONVERT(date,AssignmentDate))*864000000000+DATEDIFF_BIG(nanosecond,CONVERT(datetime2(7),CONVERT(date,AssignmentDate)),AssignmentDate)/100) AS assignment_ticks,CONVERT(varchar(20),DATEDIFF_BIG(day,CONVERT(date,'00010101'),CONVERT(date,RemovedDate))*864000000000+DATEDIFF_BIG(nanosecond,CONVERT(datetime2(7),CONVERT(date,RemovedDate)),RemovedDate)/100) AS removed_ticks,Progress0 AS progress0,Progress1 AS progress1,Progress2 AS progress2,Progress3 AS progress3,Progress4 AS progress4 FROM dbo.MissionAssignment ORDER BY CID,ID FOR JSON PATH,INCLUDE_NULL_VALUES),'[]')) AS assignments
FOR JSON PATH,INCLUDE_NULL_VALUES,WITHOUT_ARRAY_WRAPPER;
COMMIT;
"@
 $sh='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
 $r=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$sh,'readonly-character-state-export',$taskQuery)
 if($r.ExitCode-ne0){throw 'Readonly character state export failed;private output suppressed'}
 $a=$r.Stdout.IndexOf('{');$b=$r.Stdout.LastIndexOf('}');if($a-lt0-or$b-lt$a){throw 'Missing character state export'}
 $json=$r.Stdout.Substring($a,$b-$a+1).Replace([string][char]13,'').Replace([string][char]10,'')
 if([Text.Encoding]::UTF8.GetByteCount($json)-gt64*1024*1024){throw 'Source state export exceeds bound'}
 $data=$json|ConvertFrom-Json -DateKind String
 $counts=@(foreach($name in @('titles','goals','presets','micro','missions','assignments')){if($null-eq$data.$name){throw 'Required source array missing'};@($data.$name|Where-Object{$null-ne$_}).Count})
 if(@($counts|Where-Object{$_-gt100000}).Count){throw 'Source row bounds changed'}
 return [pscustomobject]@{Json=$json;Counts=$counts;QuerySha256=Get-TaskHash $taskQuery}
}
function Invoke-TaskImport([string]$Json,[bool]$Verify,[object[]]$Counts){
 $args=@('compose','--project-directory',$taskProject,'exec','-T','server','/app/vindictus-server','import-character-state','--input-stdin');if($Verify){$args+='--verify-only'}
 $r=Invoke-ProjectDocker -Arguments $args -InputText $Json
 if($r.ExitCode-ne0){throw 'Character state preservation failed;private values suppressed'}
 try{$summary=$r.Stdout.Trim()|ConvertFrom-Json -DateKind String}catch{throw 'Invalid aggregate character state summary'}
 if(($summary.source_rows-join',')-cne($Counts-join',')-or($summary.verified-join',')-cne($Counts-join',')-or-not$summary.preservation_initialized-or$summary.character_creation_available){throw 'Character state summary mismatch'}
 return $summary
}
if($Mode-eq'Import'){
 $export=Get-TaskExport
 New-Item -ItemType Directory -Force -Path $taskPrivate|Out-Null
 $acl=[Security.AccessControl.DirectorySecurity]::new();$acl.SetAccessRuleProtection($true,$false)
 foreach($sid in @([Security.Principal.WindowsIdentity]::GetCurrent().User,[Security.Principal.SecurityIdentifier]::new('S-1-5-18'))){$acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($sid,'FullControl','ContainerInherit,ObjectInherit','None','Allow'))}
 [IO.FileSystemAclExtensions]::SetAccessControl([IO.DirectoryInfo]::new($taskPrivate),$acl)
 [IO.File]::WriteAllText($taskFile,$export.Json,[Text.UTF8Encoding]::new($false))
 if($ExportOnly){Write-Output ('Readonly private character-state snapshot captured;rows='+($export.Counts-join'/')+';not imported;reference SELECT-only.');return}
 $imported=Invoke-TaskImport $export.Json $false $export.Counts
 $replay=Invoke-TaskImport $export.Json $false $export.Counts;if(@($replay.inserted|Where-Object{$_-ne0}).Count){throw 'Character state replay inserted rows'}
 $verified=Invoke-TaskImport $export.Json $true $export.Counts
 $after=Get-TaskExport;if((Get-TaskHash $after.Json)-cne(Get-TaskHash $export.Json)){throw 'Readonly character state reference changed during preservation'}
 $report=[ordered]@{stage='M15.1b.2e.10a';verified_at_utc=[DateTimeOffset]::UtcNow.ToString('o');source_read_only=$true;source_export_unchanged=$true;source_rows=$export.Counts;source_columns=@(6,3,2,12,3,11);export_sha256=Get-TaskHash $export.Json;query_sha256=$export.QuerySha256;schema_sha256=(Get-FileHash (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json')).Hash.ToLowerInvariant();private_acl='current Windows user and SYSTEM only';imported=$imported;replay=$replay;verified=$verified;post_restart_verified=$false;character_creation_available=$false}
}else{
 $report=Get-Content $taskEvidence -Raw|ConvertFrom-Json -DateKind String
 $json=[IO.File]::ReadAllText($taskFile);if((Get-TaskHash $json)-cne$report.export_sha256){throw 'Private character state snapshot digest changed'}
 $verified=Invoke-TaskImport $json $true $report.source_rows
 $report.post_restart_verified=$true
 $report|Add-Member -Force -NotePropertyName post_restart_verified_at_utc -NotePropertyValue ([DateTimeOffset]::UtcNow.ToString('o'))
 $report|Add-Member -Force -NotePropertyName post_restart_summary -NotePropertyValue $verified
}
$report|ConvertTo-Json -Depth 7|Set-Content $taskEvidence -Encoding utf8NoBOM
Write-Output ('Character state '+$Mode+' PASS;source rows='+($report.source_rows-join'/')+';full typed comparison;private values suppressed;character creation=false.')
