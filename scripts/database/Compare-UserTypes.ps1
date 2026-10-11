[CmdletBinding()]
param()
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskSql=@"
SET NOCOUNT ON;
WITH dates AS (
 SELECT v.input, CONVERT(datetime, v.input, 126) AS dt
 FROM (VALUES
 ('1753-01-01T00:00:00.000'),('1899-12-31T23:59:59.997'),
 ('1900-01-01T00:00:00.000'),('2024-02-29T12:34:56.001'),
 ('2024-02-29T12:34:56.002'),('2024-02-29T12:34:56.005'),
 ('2024-02-29T23:59:59.999'),('9999-12-31T23:59:59.997')
 ) v(input)
), parts AS (
 SELECT input, dt,
 CONVERT(int,SUBSTRING(CONVERT(binary(8),dt),1,4)) AS days,
 CONVERT(int,SUBSTRING(CONVERT(binary(8),dt),5,4)) AS time300
 FROM dates
), dates2 AS (
 SELECT v.input, CONVERT(datetime2(7),v.input,126) AS dt
 FROM (VALUES
 ('0001-01-01T00:00:00.0000000'),('0001-01-01T00:00:00.0000001'),
 ('1900-01-01T00:00:00.0000000'),('2024-02-29T12:34:56.1234567'),
 ('2024-02-29T12:34:56.1234568'),('9999-12-31T23:59:59.9999999')
 ) v(input)
)
SELECT JSON_QUERY((SELECT input, CONVERT(varchar(23),dt,126) AS canonical,
 CONVERT(varchar(16),CONVERT(binary(8),dt),2) AS binary_hex, days, time300,
 CONVERT(bigint,days)*25920000+time300 AS units300,
 CAST(CASE WHEN CONVERT(datetime,CONVERT(binary(4),days)+CONVERT(binary(4),time300))=dt THEN 1 ELSE 0 END AS bit) AS binary_roundtrip_equal
 FROM parts FOR JSON PATH)) AS datetime_cases,
 JSON_QUERY((SELECT input, CONVERT(varchar(27),dt,126) AS canonical,
 DATEDIFF_BIG(day,CONVERT(date,'00010101'),CONVERT(date,dt))*864000000000+
 DATEDIFF_BIG(nanosecond,CONVERT(datetime2(7),CONVERT(date,dt)),dt)/100 AS ticks
 FROM dates2 FOR JSON PATH)) AS datetime2_cases,
 JSON_QUERY((SELECT label,CONVERT(varchar(max),CONVERT(varbinary(max),value),2) AS binary_hex,DATALENGTH(value) AS byte_length
 FROM (VALUES
 ('empty',CONVERT(nvarchar(50),N'')),('nul',CONVERT(nvarchar(50),NCHAR(0))),
 ('supplementary',CONVERT(nvarchar(50),0x3DD800DE)),
 ('unpaired-surrogate',CONVERT(nvarchar(50),0x00D8)),('accent',CONVERT(nvarchar(50),N'é'))
 ) s(label,value) FOR JSON PATH)) AS nvarchar_cases,
 JSON_QUERY((SELECT label,CONVERT(varchar(max),CONVERT(varbinary(max),value),2) AS binary_hex,DATALENGTH(value) AS byte_length
 FROM (VALUES
 ('empty',CONVERT(varchar(255),'')),('high-byte',CONVERT(varchar(255),0x80FF)),
 ('nul',CONVERT(varchar(255),0x00))
 ) s(label,value) FOR JSON PATH)) AS varchar_cases
FOR JSON PATH,WITHOUT_ARRAY_WRAPPER;
"@
$taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
$taskResult=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'oracle',$taskSql)
if($taskResult.ExitCode-ne0){throw ('Read-only type oracle failed: '+$taskResult.Stderr)}
$taskStart=$taskResult.Stdout.IndexOf('{');$taskEnd=$taskResult.Stdout.LastIndexOf('}')
if($taskStart-lt0 -or $taskEnd-lt$taskStart){throw 'Missing oracle JSON'}
$taskData=$taskResult.Stdout.Substring($taskStart,$taskEnd-$taskStart+1)|ConvertFrom-Json -DateKind String
if(@($taskData.datetime_cases|Where-Object binary_roundtrip_equal -ne $true).Count-ne0){throw 'Datetime binary roundtrip mismatch'}
$taskData|Add-Member -NotePropertyName method -NotePropertyValue 'SELECT-only synthetic SQL values; no source user values exported and no database writes'
$taskData|Add-Member -NotePropertyName verified_at_utc -NotePropertyValue ([DateTimeOffset]::UtcNow.ToString('o'))
$taskData|Add-Member -NotePropertyName query_sha256 -NotePropertyValue ([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($taskSql))).ToLowerInvariant())
$taskData|Add-Member -NotePropertyName source_schema_sha256 -NotePropertyValue (Get-FileHash -LiteralPath (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json') -Algorithm SHA256).Hash.ToLowerInvariant()
$taskData|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $taskProject 'docs/evidence/database/user-types-oracle.json') -Encoding utf8
Write-Host ('Type oracle: datetime='+$taskData.datetime_cases.Count+'; datetime2='+$taskData.datetime2_cases.Count+'; nvarchar='+$taskData.nvarchar_cases.Count+'; varchar='+$taskData.varchar_cases.Count+'; binary roundtrip mismatches=0')
