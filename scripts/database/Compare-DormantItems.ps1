param()
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskSchema=Get-Content (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json') -Raw|ConvertFrom-Json
$taskObject=$taskSchema.objects|Where-Object name -eq SendDormantItem
$taskDefinition=($taskSchema.sql_modules|Where-Object object_id -eq $taskObject.object_id).definition
$taskBodyMarker=[regex]::Match($taskDefinition,'(?m)^AS\s*\r?$')
if(-not $taskBodyMarker.Success){throw 'SendDormantItem body unavailable'}
$taskBody=$taskDefinition.Substring($taskBodyMarker.Index+$taskBodyMarker.Length).Replace('[dbo].[QueuedItem]','#QueuedItem').Replace("'","''")
$taskTable=$taskSchema.tables|Where-Object name -eq QueuedItem
$taskIdentity=$taskSchema.identity_columns|Where-Object object_id -eq $taskTable.object_id
if($taskIdentity.seed_value -ne 1 -or $taskIdentity.increment_value -ne 1 -or $taskIdentity.last_value -ne 140){throw 'Reference identity changed; review before bootstrap'}
$taskDefaults=@($taskSchema.default_constraints|Where-Object parent_object_id -eq $taskTable.object_id)
$taskColumns=@($taskSchema.columns|Where-Object object_id -eq $taskTable.object_id|Sort-Object column_id)
$taskDDL=@($taskColumns|ForEach-Object{
 $taskColumn=$_
 $taskType=$taskColumn.declared_type
 if($taskType -eq 'nvarchar'){$taskType+='('+($taskColumn.max_length/2)+')'}
 if($taskType -notmatch '^(bigint|int|bit|datetime|nvarchar\(\d+\))$'){throw 'Unsupported source column type'}
 $taskDefault=$taskDefaults|Where-Object parent_column_id -eq $taskColumn.column_id
 '['+$taskColumn.name+'] '+$taskType+$(if($taskColumn.is_identity){' IDENTITY(1,1)'})+$(if($taskColumn.is_nullable){' NULL'}else{' NOT NULL'})+$(if($taskDefault){' DEFAULT '+$taskDefault.definition})
}) -join ','
$taskSql=@"
SET NOCOUNT ON;
CREATE TABLE #QueuedItem($taskDDL);
EXEC(N'CREATE PROCEDURE #SendDormantItem @cid bigint,@itemClassEX nvarchar(1024),@count int,@isBinded bit,@mailTitle nvarchar(256),@mailContent nvarchar(1024) AS $taskBody');
-- SQL Server's never-used reseed inserts the reseed value itself. Model the used source identity.
EXEC #SendDormantItem @cid=0,@itemClassEX=N'bootstrap',@count=1,@isBinded=0,@mailTitle=NULL,@mailContent=NULL;
DELETE FROM #QueuedItem;
DBCC CHECKIDENT('#QueuedItem',RESEED,140) WITH NO_INFOMSGS;
DECLARE @before datetime=GETUTCDATE();
DECLARE @odd nvarchar(1024)=CONVERT(nvarchar(1024),0x00D800004100);
EXEC #SendDormantItem @cid=-9223372036854775808,@itemClassEX=@odd,@count=-2147483648,@isBinded=1,@mailTitle=NULL,@mailContent=N'';
EXEC #SendDormantItem @cid=9223372036854775807,@itemClassEX=N'',@count=0,@isBinded=0,@mailTitle=N'',@mailContent=NULL;
DECLARE @item nvarchar(1024)=REPLICATE(N'x',1024),@title nvarchar(256)=REPLICATE(N't',256),@content nvarchar(1024)=REPLICATE(N'c',1024);
EXEC #SendDormantItem @cid=-1,@itemClassEX=@item,@count=2147483647,@isBinded=1,@mailTitle=@title,@mailContent=@content;
BEGIN TRANSACTION;
EXEC #SendDormantItem @cid=0,@itemClassEX=N'rollback',@count=1,@isBinded=1,@mailTitle=NULL,@mailContent=NULL;
ROLLBACK TRANSACTION;
EXEC #SendDormantItem @cid=0,@itemClassEX=N'after_rollback',@count=1,@isBinded=1,@mailTitle=NULL,@mailContent=NULL;
DECLARE @after datetime=GETUTCDATE();
SELECT JSON_QUERY((SELECT RowID AS row_id,CONVERT(varchar(20),CID) AS cid,
 CONVERT(varchar(max),CONVERT(varbinary(max),ItemClassEx),2) AS item_class_ex_utf16,[Count] AS count,IsCharacterBinded AS is_character_binded,
 CONVERT(varchar(max),CONVERT(varbinary(max),MailTitle),2) AS mail_title_utf16,CONVERT(varchar(max),CONVERT(varbinary(max),MailContent),2) AS mail_content_utf16,
 Color1 AS color1,Color2 AS color2,Color3 AS color3,ReducedDurability AS reduced_durability,MaxDurabilityBonus AS max_durability_bonus,
 CONVERT(varchar(20),ItemID) AS item_id,Processed AS processed,CONVERT(varchar(max),CONVERT(varbinary(max),EventCode),2) AS event_code_utf16,
 CONVERT(varchar(20),CONVERT(bigint,CONVERT(int,SUBSTRING(CONVERT(binary(8),ProcessedTime),1,4)))*25920000+CONVERT(int,SUBSTRING(CONVERT(binary(8),ProcessedTime),5,4))) AS processed_time_units300,
 CONVERT(varchar(20),CONVERT(bigint,CONVERT(int,SUBSTRING(CONVERT(binary(8),ExpireTime),1,4)))*25920000+CONVERT(int,SUBSTRING(CONVERT(binary(8),ExpireTime),5,4))) AS expire_time_units300,
 CONVERT(varchar(20),CONVERT(bigint,CONVERT(int,SUBSTRING(CONVERT(binary(8),LogDate),1,4)))*25920000+CONVERT(int,SUBSTRING(CONVERT(binary(8),LogDate),5,4))) AS log_date_units300
 FROM #QueuedItem ORDER BY RowID FOR JSON PATH,INCLUDE_NULL_VALUES)) AS rows,
 (SELECT COUNT(*) FROM #QueuedItem WHERE LogDate < @before OR LogDate > @after) AS clock_outside_bounds,
 CONVERT(int,(SELECT last_value FROM sys.identity_columns WHERE object_id=OBJECT_ID('dbo.QueuedItem'))) AS source_identity_last_value,
 (SELECT COUNT_BIG(*) FROM dbo.QueuedItem) AS source_queue_rows
FOR JSON PATH,WITHOUT_ARRAY_WRAPPER,INCLUDE_NULL_VALUES;
"@
$taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
$taskResult=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'oracle',$taskSql)
if($taskResult.ExitCode -ne 0){[IO.File]::WriteAllText((Join-Path $taskProject '.cache/verification/dormant-item-oracle-error.log'),$taskResult.Stdout+$taskResult.Stderr);throw 'Queue oracle failed; local diagnostic log only'}
$taskFirst=$taskResult.Stdout.IndexOf('{');$taskLast=$taskResult.Stdout.LastIndexOf('}')
if($taskFirst -lt 0){throw 'Oracle JSON missing'}
$taskData=$taskResult.Stdout.Substring($taskFirst,$taskLast-$taskFirst+1).Replace([string][char]13,'').Replace([string][char]10,'')|ConvertFrom-Json -DateKind String
if(($taskData.rows.row_id -join ',') -ne '141,142,143,145' -or $taskData.clock_outside_bounds -ne 0 -or $taskData.source_identity_last_value -ne 140 -or $taskData.source_queue_rows -ne 140){Write-Host ('Oracle row_ids='+($taskData.rows.row_id -join ',')+'; clock violations='+$taskData.clock_outside_bounds+'; source counter='+$taskData.source_identity_last_value+'; source rows='+$taskData.source_queue_rows);throw 'Queue oracle contract mismatch'}
$taskSources=@('docs/evidence/database/study_heroes.schema.json','legacy/decompiled/LoginServiceCore-1600fb4b593d/LoginServiceCore.Processor/UserLoginProcessor.cs','legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/DBLoader.cs','legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore.Static/QueuedItemController.cs')
$taskEvidence=[ordered]@{
 verified_at_utc=[DateTimeOffset]::UtcNow.ToString('o');source_identity=[ordered]@{seed=1;increment=1;last_value=140};
 definition_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($taskDefinition))).ToLowerInvariant();
 query_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($taskSql))).ToLowerInvariant();
 sources=@($taskSources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash (Join-Path $taskProject $_)).Hash.ToLowerInvariant()}});
 column_count=$taskColumns.Count;defaults=@($taskDefaults|ForEach-Object{[ordered]@{column=($taskColumns|Where-Object column_id -eq $_.parent_column_id).name;expression=$_.definition}});
 compared_synthetic_rows=4;expected_row_ids=@(141,142,143,145);rollback_consumes_identity=$true;
 clock_outside_bounds=0;source_queue_rows=140;source_identity_last_value=140;source_persistent_writes=$false;original_services_executed=$false;
 method='Unchanged recovered procedure body redirected to a full 18-column temporary table generated from source types/defaults; temporary IDENTITY reseeded from explicit catalog last_value. GETUTCDATE unmodified; its quantized output supplied explicitly to Rust. No persistent reference writes.'
}
$taskData|ConvertTo-Json -Depth 6|Set-Content (Join-Path $taskProject 'tests/fixtures/sql-dormant-items.json') -Encoding utf8NoBOM
$taskEvidence|ConvertTo-Json -Depth 7|Set-Content (Join-Path $taskProject 'docs/evidence/database/dormant-item-oracle.json') -Encoding utf8NoBOM
Write-Host 'Dormant item oracle PASS: 4 synthetic rows/18 columns; identity gap after rollback; UTC bounds=0 violations; reference rows/counter=140/140 unchanged.'
