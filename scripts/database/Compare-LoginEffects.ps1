param()
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskSql=@"
SET NOCOUNT ON;
WITH Samples AS (
 SELECT case_id,row_id,CONVERT(datetime,connect_value) AS last_connect,
 CONVERT(datetime,cafe_value) AS last_cafe_logout
 FROM (VALUES
 (1,1,NULL,NULL),
 (2,1,NULL,'19000101 00:00:00.007'),
 (3,1,'19000101 00:00:00.003',NULL),
 (4,1,'19000101 00:00:00.003','19000101 00:00:00.007'),
 (5,1,'19000101 00:00:00.010','19000101 00:00:00.003'),
 (6,1,NULL,'19000101 00:00:00.010'),
 (6,2,'19000101 00:00:00.003','19000101 00:00:00.007'),
 (6,3,'19000101 00:00:00.010',NULL),
 (7,1,'18991231 23:59:59.997',NULL),
 (8,1,'19000101 00:00:00.007','19000101 00:00:00.007')
 )v(case_id,row_id,connect_value,cafe_value)
), Cases AS (SELECT case_id FROM (VALUES(0),(1),(2),(3),(4),(5),(6),(7),(8))v(case_id))
SELECT c.case_id,
 JSON_QUERY((SELECT
 CONVERT(varchar(24),CONVERT(bigint,DATEDIFF(day,'19000101',last_connect))*25920000+
 CONVERT(bigint,SUBSTRING(CONVERT(binary(8),last_connect),5,4))) AS connect_units300,
 CONVERT(varchar(24),CONVERT(bigint,DATEDIFF(day,'19000101',last_cafe_logout))*25920000+
 CONVERT(bigint,SUBSTRING(CONVERT(binary(8),last_cafe_logout),5,4))) AS cafe_units300
 FROM Samples s WHERE s.case_id=c.case_id ORDER BY row_id FOR JSON PATH,INCLUDE_NULL_VALUES)) AS rows,
 CONVERT(varchar(24),CONVERT(bigint,DATEDIFF(day,'19000101',latest.value))*25920000+
 CONVERT(bigint,SUBSTRING(CONVERT(binary(8),latest.value),5,4))) AS latest_units300
FROM Cases c OUTER APPLY (SELECT MAX(CASE WHEN last_connect < last_cafe_logout
 THEN last_cafe_logout ELSE last_connect END) AS value FROM Samples s WHERE s.case_id=c.case_id) latest
ORDER BY c.case_id FOR JSON PATH,INCLUDE_NULL_VALUES;
"@
$taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
$taskResult=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'login-effects-oracle',$taskSql)
if($taskResult.ExitCode-ne0){throw 'Read-only latest-connection SQL oracle failed; details suppressed'}
$taskFirst=$taskResult.Stdout.IndexOf('[');$taskLast=$taskResult.Stdout.LastIndexOf(']')
if($taskFirst-lt0-or$taskLast-lt$taskFirst){throw 'Oracle JSON missing'}
$taskCases=@($taskResult.Stdout.Substring($taskFirst,$taskLast-$taskFirst+1).Replace([string][char]13,'').Replace([string][char]10,'')|ConvertFrom-Json -DateKind String)
$taskSchema=Get-Content (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json') -Raw|ConvertFrom-Json
$taskModules=@()
foreach($taskName in @('GetLatestConnectedDate','GetDormantUserData','ReleaseDormant','SetDormantUser','DormantUserEventSelect','DormantUserEventUpdate','GetFriendRecommendFlag','SetFriendRecommendFlag','ReplaceUserAchievementNIDtoUID','SendDormantItem')) {
 $taskObj=$taskSchema.objects|Where-Object name -eq $taskName
 $taskModule=$taskSchema.sql_modules|Where-Object object_id -eq $taskObj.object_id
 if(-not$taskModule.definition){throw 'Required recovered SQL module missing'}
 $taskModules+=[ordered]@{name=$taskName;object_id=$taskObj.object_id;definition_sha256=([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($taskModule.definition))).ToLowerInvariant());implemented_in_postgres=$false}
}
[ordered]@{
 verified_at_utc=[DateTimeOffset]::UtcNow.ToString('o');source_writes=$false;original_procedures_executed=$false;
 cases=$taskCases;modules=$taskModules;
 schema_sha256=(Get-FileHash (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json')).Hash.ToLowerInvariant();
 query_sha256=([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($taskSql))).ToLowerInvariant());
 method='SELECT synthetic datetime CASE/MAX with real SQL Server NULL semantics; catalog module hashes are reference, not implemented procedures';
}|ConvertTo-Json -Depth 6|Set-Content (Join-Path $taskProject 'docs/evidence/database/login-effects-oracle.json') -Encoding utf8NoBOM
Write-Host ('Login effects SQL oracle: CASE/MAX cases='+$taskCases.Count+'; reference modules='+$taskModules.Count+'; source writes=false')

