$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskSchema=Get-Content (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json') -Raw|ConvertFrom-Json -DateKind String
$taskObject=$taskSchema.objects|Where-Object name -eq 'GetUserBanInfo'
$taskDefinition=($taskSchema.sql_modules|Where-Object object_id -eq $taskObject.object_id).definition
$taskPredicate=$taskDefinition.Substring($taskDefinition.IndexOf('WHERE [ID]')).Trim().Substring(6).Replace('GETDATE()','@now')
$taskSql=@"
SET NOCOUNT ON;
DECLARE @id varchar(50)='BanFixture';
DECLARE @now datetime='2024-02-29T12:00:00.000';
WITH Synthetic AS (
 SELECT CONVERT(nvarchar(50),n.name) COLLATE SQL_Latin1_General_CP1_CI_AS AS ID,
 s.status AS Status,CONVERT(datetime,e.expiry) AS ExpireTime,
 e.label AS expiry_label,n.label AS name_label
 FROM (VALUES(-2147483648),(-1),(0),(1),(2),(2147483647))s(status)
 CROSS JOIN (VALUES
 ('NULL',CONVERT(varchar(30),NULL)),('before','2024-02-29T11:59:59.997'),
 ('equal','2024-02-29T12:00:00.000'),('after','2024-02-29T12:00:00.003')
 ) e(label,expiry)
 CROSS JOIN (VALUES
 ('same',N'BanFixture'),('case',N'BANFIXTURE'),('trailing-space',N'BanFixture '),('different',N'Other')
 )n(label,name)
)
SELECT
 CONVERT(varchar(20),CONVERT(bigint,CONVERT(int,SUBSTRING(CONVERT(binary(8),@now),1,4)))*25920000+
 CONVERT(int,SUBSTRING(CONVERT(binary(8),@now),5,4))) AS now_units300,
 (SELECT COUNT(*) FROM dbo.UserBan) AS source_ban_rows,
 DATEPART(TZOFFSET,SYSDATETIMEOFFSET()) AS restored_sql_offset_minutes,
 JSON_QUERY((SELECT
 CONVERT(varchar(20),DATEDIFF_BIG(day,CONVERT(date,'00010101'),CONVERT(date,u.utc))*864000000000+
 DATEDIFF_BIG(nanosecond,CONVERT(datetime2(7),CONVERT(date,u.utc)),u.utc)/100) AS utc_ticks,
 CONVERT(varchar(20),DATEDIFF_BIG(day,CONVERT(date,'00010101'),CONVERT(date,l.local))*864000000000+
 DATEDIFF_BIG(nanosecond,CONVERT(datetime2(7),CONVERT(date,l.local)),l.local)/100) AS local_ticks,
 CONVERT(varchar(20),CONVERT(bigint,CONVERT(int,SUBSTRING(CONVERT(binary(8),d.dt),1,4)))*25920000+
 CONVERT(int,SUBSTRING(CONVERT(binary(8),d.dt),5,4))) AS sql_units300
 FROM (VALUES('1753-01-01T00:00:00.0000000'),('1899-12-31T15:59:59.9983333'),
 ('1899-12-31T15:59:59.9983334'),('1899-12-31T15:59:59.9999999'),
 ('1900-01-01T00:00:00.0000000'),('1970-01-01T00:00:00.0000000'),
 ('2024-02-29T04:00:00.0016666'),('2024-02-29T04:00:00.0016667'),
 ('2024-02-29T04:00:00.1234567'),('9999-12-30T00:00:00.0000000'))v(input)
 CROSS APPLY(SELECT CONVERT(datetime2(7),input) AS utc)u
 CROSS APPLY(SELECT DATEADD(minute,480,u.utc) AS local)l
 CROSS APPLY(SELECT CONVERT(datetime,l.local) AS dt)d FOR JSON PATH)) AS clock_cases,
 JSON_QUERY((SELECT ID AS name,Status AS status,expiry_label,name_label,
 CONVERT(varchar(20),CONVERT(bigint,CONVERT(int,SUBSTRING(CONVERT(binary(8),ExpireTime),1,4)))*25920000+
 CONVERT(int,SUBSTRING(CONVERT(binary(8),ExpireTime),5,4))) AS expiry_units300,
 CAST(CASE WHEN $taskPredicate THEN 1 ELSE 0 END AS bit) AS selected
 FROM Synthetic ORDER BY Status,expiry_label,name_label FOR JSON PATH,INCLUDE_NULL_VALUES)) AS cases
FOR JSON PATH,WITHOUT_ARRAY_WRAPPER;
"@
$taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
$taskResult=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'ban-oracle',$taskSql)
if($taskResult.ExitCode-ne0){throw 'Read-only ban oracle failed; detailed outputs suppressed.'}
$taskFirst=$taskResult.Stdout.IndexOf('{');$taskLast=$taskResult.Stdout.LastIndexOf('}')
$taskJson=$taskResult.Stdout.Substring($taskFirst,$taskLast-$taskFirst+1).Replace([string][char]13,'').Replace([string][char]10,'')
$taskData=$taskJson|ConvertFrom-Json -DateKind String
$taskData|Add-Member -NotePropertyName procedure_sha256 -NotePropertyValue ([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($taskDefinition))).ToLowerInvariant())
$taskData|Add-Member -NotePropertyName query_sha256 -NotePropertyValue ([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($taskSql))).ToLowerInvariant())
$taskData|Add-Member -NotePropertyName original_runtime_timezone_observed -NotePropertyValue $false
$taskData|Add-Member -NotePropertyName verified_at_utc -NotePropertyValue ([DateTimeOffset]::UtcNow.ToString('o'))
$taskData|Add-Member -NotePropertyName method -NotePropertyValue 'Recovered WHERE predicate over synthetic SELECT values; GETDATE replaced by fixed local datetime; source UserBan count only; no source writes'
$taskData|ConvertTo-Json -Depth 6|Set-Content (Join-Path $taskProject 'docs/evidence/database/ban-policy-oracle.json') -Encoding utf8NoBOM
Write-Host ('SQL ban oracle: synthetic cases='+$taskData.cases.Count+'; source bans='+$taskData.source_ban_rows+'; runtime VM not executed; static profile in offline-vm-clock.json; no writes')
