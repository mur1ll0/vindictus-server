param()
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskSchema=Get-Content (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json') -Raw|ConvertFrom-Json
$taskModules=@()
$taskBodies=@{}
foreach($taskName in 'GetCharacters','GetLatestConnectedDate'){
 $taskObj=$taskSchema.objects|Where-Object name -eq $taskName
 $taskDef=($taskSchema.sql_modules|Where-Object object_id -eq $taskObj.object_id).definition
 $taskNormalized=$taskDef.Replace([string][char]13,'')
 $taskIndex=$taskNormalized.IndexOf([char]10+'AS'+[char]10)
 if($taskIndex -lt 0){throw 'Recovered procedure body marker missing'}
 $taskBody=$taskNormalized.Substring($taskIndex+4).Replace('dbo.CharacterInfo','#CharacterInfo').Replace('[CharacterInfo]','#CharacterInfo').Replace('[LastConnectInfo]','#LastConnectInfo')
 $taskBodies[$taskName]=$taskBody.Replace("'","''")
 $taskModules += [ordered]@{name=$taskName;definition_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($taskDef))).ToLowerInvariant()}
}
$taskSql=@"
SET NOCOUNT ON;
CREATE TABLE #CharacterInfo(ID bigint,UID int,OrderNum tinyint NULL,Status int,DeleteTime datetime NULL);
CREATE TABLE #LastConnectInfo(ID bigint,LastConnectDate datetime NOT NULL,LastCafeLogout datetime NULL);
CREATE TABLE #Listed(Seq int IDENTITY,ID bigint,UID int,OrderNum tinyint NULL,Status int,DeleteTime datetime NULL);
CREATE TABLE #LatestCases(case_id int,connect_units bigint,logout_units bigint NULL,result_units bigint NULL);
EXEC(N'CREATE PROCEDURE #GetCharacters @UID int AS $($taskBodies.GetCharacters)');
EXEC(N'CREATE PROCEDURE #Latest @uid int,@latestDate datetime OUTPUT AS $($taskBodies.GetLatestConnectedDate)');
INSERT #CharacterInfo VALUES(4,8,1,400,NULL),(1,7,255,100,NULL),(3,7,0,300,CONVERT(datetime,''19000101'')),(2,7,NULL,200,NULL);
INSERT #LastConnectInfo VALUES(1,CONVERT(datetime,0x0000000000000001),NULL),(2,CONVERT(datetime,0x0000000000000002),CONVERT(datetime,0x0000000000000004)),(4,CONVERT(datetime,0x0000000000000004),NULL);
INSERT #Listed(ID,UID,OrderNum,Status,DeleteTime) EXEC #GetCharacters @UID=7;
DECLARE @bundle datetime;
EXEC #Latest @uid=7,@latestDate=@bundle OUTPUT;
UPDATE #CharacterInfo SET UID=9 WHERE ID=1;
DECLARE @cases TABLE(case_id int,connect_binary binary(8),logout_binary binary(8) NULL);
INSERT @cases VALUES(0,0x0000000000000001,NULL),(1,0x0000000000000002,0x0000000000000004),(2,0x0000000000000004,0x0000000000000002),(3,0x0000000000000004,0x0000000000000004),(4,0xFFFFFFFF018B81FF,0x0000000000000000),(5,0x0000000000000000,0xFFFFFFFF018B81FF);
DECLARE @id int,@c binary(8),@l binary(8),@result datetime;
DECLARE task_cases CURSOR LOCAL FAST_FORWARD FOR SELECT case_id,connect_binary,logout_binary FROM @cases ORDER BY case_id;
OPEN task_cases;
FETCH NEXT FROM task_cases INTO @id,@c,@l;
WHILE @@FETCH_STATUS=0 BEGIN
 UPDATE #LastConnectInfo SET LastConnectDate=CONVERT(datetime,@c),LastCafeLogout=CONVERT(datetime,@l) WHERE ID=1;
 SET @result=NULL; EXEC #Latest @uid=9,@latestDate=@result OUTPUT;
 INSERT #LatestCases VALUES(@id,
 CONVERT(bigint,CONVERT(int,SUBSTRING(@c,1,4)))*25920000+CONVERT(int,SUBSTRING(@c,5,4)),
 CONVERT(bigint,CONVERT(int,SUBSTRING(@l,1,4)))*25920000+CONVERT(int,SUBSTRING(@l,5,4)),
 CONVERT(bigint,CONVERT(int,SUBSTRING(CONVERT(binary(8),@result),1,4)))*25920000+CONVERT(int,SUBSTRING(CONVERT(binary(8),@result),5,4)));
 FETCH NEXT FROM task_cases INTO @id,@c,@l;
END
CLOSE task_cases; DEALLOCATE task_cases;
DECLARE @missing datetime=NULL; EXEC #Latest @uid=-1,@latestDate=@missing OUTPUT;
SELECT JSON_QUERY((SELECT CONVERT(varchar(20),ID) AS id FROM #Listed ORDER BY Seq FOR JSON PATH)) AS listed,
CONVERT(varchar(20),CONVERT(bigint,CONVERT(int,SUBSTRING(CONVERT(binary(8),@bundle),1,4)))*25920000+CONVERT(int,SUBSTRING(CONVERT(binary(8),@bundle),5,4))) AS bundle_latest,
JSON_QUERY((SELECT case_id,CONVERT(varchar(20),connect_units) AS connect,CONVERT(varchar(20),logout_units) AS logout,CONVERT(varchar(20),result_units) AS result FROM #LatestCases ORDER BY case_id FOR JSON PATH,INCLUDE_NULL_VALUES)) AS latest_cases,
CONVERT(varchar(20),@missing) AS missing_uid,
(SELECT COUNT_BIG(*) FROM dbo.[Character]) AS source_characters,
(SELECT COUNT_BIG(*) FROM dbo.CharacterInfo) AS source_character_info,
(SELECT COUNT_BIG(*) FROM dbo.LastConnectInfo) AS source_connections,
(SELECT COUNT_BIG(*) FROM dbo.QueuedItem) AS source_queued_items
FOR JSON PATH,WITHOUT_ARRAY_WRAPPER,INCLUDE_NULL_VALUES;
"@
$taskSql=$taskSql.Replace("''19000101''","'19000101'")
$taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
$taskResult=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'oracle',$taskSql)
if($taskResult.ExitCode -ne 0){[IO.File]::WriteAllText((Join-Path $taskProject '.cache/verification/character-oracle-error.log'),$taskResult.Stdout+$taskResult.Stderr);throw 'Character oracle failed; local diagnostic log only'}
$taskFirst=$taskResult.Stdout.IndexOf('{');$taskLast=$taskResult.Stdout.LastIndexOf('}')
if($taskFirst -lt 0 -or $taskLast -lt $taskFirst){throw 'Oracle JSON missing'}
$taskData=$taskResult.Stdout.Substring($taskFirst,$taskLast-$taskFirst+1).Replace([string][char]13,'').Replace([string][char]10,'')|ConvertFrom-Json -DateKind String
$taskData|Add-Member -NotePropertyMembers @{
 listed_ids=@($taskData.listed|ForEach-Object id);verified_at_utc=[DateTimeOffset]::UtcNow.ToString('o');
 source_persistent_writes=$false;temporary_procedures_created=$true;original_services_executed=$false;
 modules=$taskModules;schema_sha256=(Get-FileHash (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json')).Hash.ToLowerInvariant();
 query_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($taskSql))).ToLowerInvariant();
}
$taskData|ConvertTo-Json -Depth 7|Set-Content (Join-Path $taskProject 'docs/evidence/database/character-login-oracle.json') -Encoding utf8NoBOM
Write-Host ('Character oracle PASS: listed=3; latest cases='+$taskData.latest_cases.Count+'; persistent source writes=false; source counts='+$taskData.source_characters+'/'+$taskData.source_character_info+'/'+$taskData.source_connections+'/'+$taskData.source_queued_items)
