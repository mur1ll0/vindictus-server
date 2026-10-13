$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskCatalog=Get-Content (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json') -Raw|ConvertFrom-Json -DateKind String
$taskObject=@($taskCatalog.objects|Where-Object {$_.name-ceq'GetHomeCafeBonusInfo'-and$_.type.Trim()-eq'P'})
if($taskObject.Count-ne1){throw 'Recovered HomeCafe procedure missing'}
$taskModule=@($taskCatalog.sql_modules|Where-Object object_id -eq $taskObject[0].object_id)
if($taskModule.Count-ne1){throw 'Recovered HomeCafe definition missing'}
$taskDefinition=[string]$taskModule[0].definition
$taskHash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($taskDefinition))).ToLowerInvariant()
if($taskHash-cne'1e2d7d8a5f4e46126ad9b91238e1b8d42ee90571a164178abfda7527fb91ae5d'-or@($taskDefinition.ToCharArray()|Where-Object {[int]$_-gt127}).Count-ne0){throw 'HomeCafe source changed; re-review required'}
$taskBegin=$taskDefinition.IndexOf('IF @cid IS NULL');$taskEnd=$taskDefinition.IndexOf('ELSE IF @bonusType IS NULL',$taskBegin)
if($taskBegin-lt0-or$taskEnd-le$taskBegin){throw 'HomeCafe source branch boundaries changed'}
$taskBranch=$taskDefinition.Substring($taskBegin,$taskEnd-$taskBegin)
$taskSynthetic='(VALUES (44,CONVERT(bigint,0),3,@rowExpiry,CASE WHEN (@mask & 1)<>0 THEN 1 ELSE 0 END),
(44,0,4,@rowExpiry,CASE WHEN (@mask & 2)<>0 THEN 1 ELSE 0 END),
(44,0,2,@rowExpiry,CASE WHEN (@mask & 4)<>0 THEN 1 ELSE 0 END),
(45,0,3,@rowExpiry,1),(44,7,3,@rowExpiry,1),(44,0,1,@rowExpiry,1)) AS synthetic([UID],CID,BonusType,ExpiredTime,Enabled)'
if([regex]::Matches($taskBranch,'dbo\.\[HomeCafeBonus\] WITH\(NOLOCK\)').Count-ne3){throw 'Source lookup shape changed'}
$taskBranch=$taskBranch.Replace('dbo.[HomeCafeBonus] WITH(NOLOCK)',$taskSynthetic).Replace('WHERE [UID] = @uid','WHERE Enabled=1 AND [UID] = @uid')
$taskQuery=@"
SET NOCOUNT ON;
DECLARE @uid int=44,@cid bigint=NULL,@bonusType tinyint,@expiredTime datetime,@rowExpiry datetime,
@initial tinyint,@mask int,@case int=0,@out nvarchar(max)=N'',@before datetime=CONVERT(datetime,'2000-01-01T00:00:00',126);
WHILE @case<64 BEGIN
 SET @mask=@case%8;
 SET @initial=CASE (@case/8)%4 WHEN 0 THEN NULL WHEN 1 THEN 0 WHEN 2 THEN 7 ELSE 255 END;
 SET @bonusType=@initial;SET @expiredTime=@before;
 SET @rowExpiry=CASE WHEN @case<32 THEN CONVERT(datetime,'1753-01-01',126) ELSE CONVERT(datetime,'2999-01-01',126) END;
$taskBranch
 SET @out=@out+(SELECT @case AS case_id,@mask AS mask,@initial AS initial_bonus_type,@bonusType AS bonus_type,
 CONVERT(varchar(23),@rowExpiry,126) AS row_expiry,CONVERT(varchar(20),CONVERT(bigint,CONVERT(int,SUBSTRING(CONVERT(binary(8),@rowExpiry),1,4)))*25920000+CONVERT(int,SUBSTRING(CONVERT(binary(8),@rowExpiry),5,4))) AS row_expiry_units300,CONVERT(varchar(23),@expiredTime,126) AS expired_output,
 CAST(CASE WHEN @expiredTime=@before THEN 1 ELSE 0 END AS bit) AS expired_output_unchanged FOR JSON PATH,WITHOUT_ARRAY_WRAPPER,INCLUDE_NULL_VALUES)+N',';
 SET @case=@case+1;
END;
SELECT LOWER(CONVERT(varchar(64),HASHBYTES('SHA2_256',CONVERT(varchar(max),OBJECT_DEFINITION(OBJECT_ID('dbo.GetHomeCafeBonusInfo')))),2)) AS source_definition_sha256,
JSON_QUERY(N'['+LEFT(@out,LEN(@out)-1)+N']') AS cases FOR JSON PATH,WITHOUT_ARRAY_WRAPPER;
"@
$taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
$taskResult=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'home-cafe-synthetic-select-only',$taskQuery)
if($taskResult.ExitCode-ne0){throw 'HomeCafe synthetic SELECT oracle failed; diagnostics suppressed'}
$taskData=$taskResult.Stdout.Trim().Replace([string][char]13,'').Replace([string][char]10,'')|ConvertFrom-Json -DateKind String
if($taskData.source_definition_sha256-cne$taskHash-or@($taskData.cases).Count-ne64-or@($taskData.cases|Where-Object {-not$_.expired_output_unchanged}).Count-ne0){throw 'HomeCafe original hash/output oracle mismatch'}
$taskFixture=[ordered]@{source_definition_sha256=$taskHash;query_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($taskQuery))).ToLowerInvariant();cases=$taskData.cases;method='Original CID=NULL branch against synthetic VALUES only; assignment/ROWCOUNT ordering retained; no source rows read/exported or persistent writes';sqlserver_persistent_writes=$false;source_rows_exported=$false}
$taskFixture|ConvertTo-Json -Depth 5|Set-Content (Join-Path $taskProject 'tests/fixtures/sql-home-cafe-bonus.json') -Encoding utf8NoBOM
Write-Host 'HomeCafe original branch:64 synthetic cases;3/4/2 preference, absence/null/255, unrelated owners/CIDs, past/future expiration and unchanged output; source hash equal; no persistent writes.'
