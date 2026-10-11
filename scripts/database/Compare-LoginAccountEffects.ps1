param()
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskSchema=Get-Content (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json') -Raw|ConvertFrom-Json
$taskModules=@()
function Get-TaskBody($Name) {
 $taskObj=$taskSchema.objects|Where-Object name -eq $Name
 $taskDefinition=($taskSchema.sql_modules|Where-Object object_id -eq $taskObj.object_id).definition
 $taskNormalized=$taskDefinition.Replace([string][char]13,'')
 $taskIndex=$taskNormalized.IndexOf([char]10+'AS'+[char]10)
 if($taskIndex-lt0){throw 'Source procedure body marker missing'}
 $taskBody=$taskNormalized.Substring($taskIndex+4)
 $taskBody=$taskBody.Replace('[dbo].[FriendLoyaltyInfo]','#FriendLoyaltyInfo').Replace('dbo.[FriendLoyaltyInfo]','#FriendLoyaltyInfo').Replace('[dbo].[UserAchievement]','#UserAchievement').Replace('[dbo].[User]','#AccountUser').Replace('[User]','#AccountUser')
 return $taskBody.Replace("'","''")
}
foreach($taskName in @('SetFriendRecommendFlag','ReplaceUserAchievementNIDtoUID','ReleaseDormant','DormantUserEventUpdate')) {
 $taskObj=$taskSchema.objects|Where-Object name -eq $taskName
 $taskDef=($taskSchema.sql_modules|Where-Object object_id -eq $taskObj.object_id).definition
 $taskModules+=[ordered]@{name=$taskName;definition_sha256=([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($taskDef))).ToLowerInvariant())}
}
$taskFriend=Get-TaskBody 'SetFriendRecommendFlag'
$taskAchievement=Get-TaskBody 'ReplaceUserAchievementNIDtoUID'
$taskRelease=Get-TaskBody 'ReleaseDormant'
$taskEvent=Get-TaskBody 'DormantUserEventUpdate'
$taskFriendJson=@"
SELECT RecommenderUID AS uid,CONVERT(varchar(20),RecommenderCID) AS recommender_cid,
CONVERT(varchar(20),FriendCID) AS friend_cid,RecommendFlag AS flag,
CONVERT(varchar(20),CONVERT(bigint,CONVERT(int,SUBSTRING(CONVERT(binary(8),UpdateTime),1,4)))*25920000+
CONVERT(int,SUBSTRING(CONVERT(binary(8),UpdateTime),5,4))) AS update_units300
FROM #FriendLoyaltyInfo FOR JSON PATH,INCLUDE_NULL_VALUES,WITHOUT_ARRAY_WRAPPER
"@
$taskAchievementJson=@"
SELECT ID AS id,UID AS uid,CONVERT(varchar(max),CONVERT(varbinary(max),name),2) AS name_hex,
CONVERT(varchar(max),CONVERT(varbinary(max),UserAchievement),2) AS achievement_hex
FROM #UserAchievement ORDER BY ID FOR JSON PATH,INCLUDE_NULL_VALUES
"@
$taskUserJson=@"
SELECT ID AS id,
CONVERT(varchar(20),CONVERT(bigint,CONVERT(int,SUBSTRING(CONVERT(binary(8),IsDormantUser),1,4)))*25920000+
CONVERT(int,SUBSTRING(CONVERT(binary(8),IsDormantUser),5,4))) AS dormant_units300,
CONVERT(varchar(20),CONVERT(bigint,CONVERT(int,SUBSTRING(CONVERT(binary(8),IsDormantUserEvent),1,4)))*25920000+
CONVERT(int,SUBSTRING(CONVERT(binary(8),IsDormantUserEvent),5,4))) AS event_units300
FROM #AccountUser FOR JSON PATH,INCLUDE_NULL_VALUES,WITHOUT_ARRAY_WRAPPER
"@
$taskSql=@"
SET NOCOUNT ON;
CREATE TABLE #FriendLoyaltyInfo(RecommenderUID int PRIMARY KEY,RecommenderCID bigint NULL,FriendCID bigint NULL,
RecommendFlag int NOT NULL,UpdateTime datetime NOT NULL DEFAULT '19000101 00:00:00.003');
CREATE TABLE #UserAchievement(ID int PRIMARY KEY,UID int NOT NULL,
name nvarchar(24) COLLATE SQL_Latin1_General_CP1_CI_AS NOT NULL,UserAchievement nvarchar(50) NOT NULL);
CREATE TABLE #AccountUser(ID int PRIMARY KEY,IsDormantUser datetime NULL,IsDormantUserEvent datetime NULL);
EXEC(N'CREATE PROCEDURE #TaskFriend @RecommenderUID bigint,@flag int AS $taskFriend');
EXEC(N'CREATE PROCEDURE #TaskAchievement @NID nvarchar(24),@uid int,@affected bigint OUTPUT AS $taskAchievement SELECT @affected=@@ROWCOUNT;');
EXEC(N'CREATE PROCEDURE #TaskRelease @uid int AS $taskRelease');
EXEC(N'CREATE PROCEDURE #TaskEvent @uid int,@dormantEventTime datetime=NULL AS $taskEvent');
CREATE TABLE #Results(kind varchar(20),case_id int,argument nvarchar(24),flag int,initial nvarchar(max),result nvarchar(max),affected bigint);
DECLARE @before nvarchar(max),@after nvarchar(max),@affected bigint;
DECLARE @case int=0;
WHILE @case<3 BEGIN
 DELETE FROM #FriendLoyaltyInfo;
 IF @case>0 INSERT #FriendLoyaltyInfo VALUES(1,-9223372036854775808,9223372036854775807,CASE WHEN @case=2 THEN -1 ELSE 99 END,'19000101 00:00:00.007');
 SET @before=($taskFriendJson);
 EXEC #TaskFriend @RecommenderUID=1,@flag=-1;
 SET @after=($taskFriendJson);
 INSERT #Results VALUES('friend',@case,NULL,-1,@before,@after,NULL);
 SET @case=@case+1;
END;
SET @case=0;
WHILE @case<4 BEGIN
 DELETE FROM #UserAchievement;
 INSERT #UserAchievement VALUES
 (1,-1,N'Fixture',CONVERT(nvarchar(50),0x00D80000)),
 (2,-1,N'FIXTURE ',N''),
 (3,9,N'Fixture',N'AlreadyOwned'),
 (4,-1,N'Other',N'Other');
 DECLARE @name nvarchar(24)=CASE @case WHEN 0 THEN N'fixture' WHEN 1 THEN N'FIXTURE ' WHEN 2 THEN N'Other' ELSE N' Fixture' END;
 SET @before=($taskAchievementJson);
 EXEC #TaskAchievement @NID=@name,@uid=7,@affected=@affected OUTPUT;
 SET @after=($taskAchievementJson);
 INSERT #Results VALUES('achievement',@case,@name,NULL,@before,@after,@affected);
 SET @case=@case+1;
END;
SET @case=0;
WHILE @case<3 BEGIN
 DELETE FROM #AccountUser;
 INSERT #AccountUser VALUES(1,'19000101 00:00:00.003','19000101 00:00:00.007');
 SET @before=($taskUserJson);
 IF @case<>1 EXEC #TaskRelease @uid=1;
 IF @case<>0 EXEC #TaskEvent @uid=1,@dormantEventTime=NULL;
 SET @after=($taskUserJson);
 INSERT #Results VALUES('user',@case,NULL,NULL,@before,@after,NULL);
 SET @case=@case+1;
END;
SELECT JSON_QUERY((SELECT kind,case_id,argument,flag,JSON_QUERY(initial) AS initial,JSON_QUERY(result) AS result,affected
FROM #Results ORDER BY kind,case_id FOR JSON PATH,INCLUDE_NULL_VALUES)) AS cases,
(SELECT COUNT_BIG(*) FROM dbo.FriendLoyaltyInfo) AS source_loyalty_rows,
(SELECT COUNT_BIG(*) FROM dbo.UserAchievement) AS source_achievement_rows,
(SELECT COUNT_BIG(*) FROM dbo.CharacterInfo) AS source_character_rows,
(SELECT COUNT_BIG(*) FROM dbo.LastConnectInfo) AS source_connection_rows,
(SELECT COUNT_BIG(*) FROM dbo.QueuedItem) AS source_queued_rows
FOR JSON PATH,WITHOUT_ARRAY_WRAPPER;
"@
$taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
$taskResult=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'account-effects-oracle',$taskSql)
if($taskResult.ExitCode-ne0){ [IO.File]::WriteAllText((Join-Path $taskProject '.cache/verification/account-oracle-error.log'),$taskResult.Stdout+$taskResult.Stderr); throw 'Temporary-table procedure oracle failed; details saved locally'}
$taskFirst=$taskResult.Stdout.IndexOf('{');$taskLast=$taskResult.Stdout.LastIndexOf('}')
if($taskFirst-lt0-or$taskLast-lt$taskFirst){throw 'Oracle JSON missing'}
$taskData=$taskResult.Stdout.Substring($taskFirst,$taskLast-$taskFirst+1).Replace([string][char]13,'').Replace([string][char]10,'')|ConvertFrom-Json -DateKind String
$taskData|Add-Member -NotePropertyMembers @{
 verified_at_utc=[DateTimeOffset]::UtcNow.ToString('o');source_persistent_writes=$false;temporary_table_writes=$true;temporary_procedures_created=$true;
 original_services_executed=$false;modules=$taskModules;
 query_sha256=([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($taskSql))).ToLowerInvariant());
 schema_sha256=(Get-FileHash (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json')).Hash.ToLowerInvariant();
 method='Recovered SQL bodies with table names redirected only to connection-local temporary tables; explicit synthetic INSERT default clock, NOLOCK/MERGE/UPDATE retained. Persistent reference tables only COUNTed.';
}
$taskData|ConvertTo-Json -Depth 9|Set-Content (Join-Path $taskProject 'docs/evidence/database/login-account-oracle.json') -Encoding utf8NoBOM
if($taskData.source_loyalty_rows-ne0-or$taskData.source_achievement_rows-ne0){throw 'Nonempty loyalty/achievement source requires private bounded import before completion'}
Write-Host ('Account SQL oracle: cases='+$taskData.cases.Count+'; loyalty source='+$taskData.source_loyalty_rows+'; achievement source='+$taskData.source_achievement_rows+'; persistent source writes=false')

