$ErrorActionPreference='Stop'
. ./scripts/dev/docker-process.ps1
$taskSourceQuery="SET NOCOUNT ON; SELECT CONVERT(varchar(64),HASHBYTES('SHA2_256',OBJECT_DEFINITION(OBJECT_ID('dbo.ChangedNameCheck'))),2);"
$taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
$taskReference=Invoke-ProjectDocker -Arguments @('compose','--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'history-reference-readonly',$taskSourceQuery)
if($taskReference.ExitCode-ne0-or$taskReference.Stdout.Trim()-notmatch'^[0-9a-fA-F]{64}$'){throw 'Reference history hash unavailable'}
$taskSql=@"
SET NOCOUNT ON;
WITH periods AS (SELECT period FROM(VALUES(-30),(0),(1),(30),(100))v(period)),
bounds AS (SELECT period,CONVERT(datetime,'2026-10-13T00:00:00.007',126) AS now_date,DATEADD(day,-period,CONVERT(datetime,'2026-10-13T00:00:00.007',126)) AS threshold FROM periods),
rows AS (SELECT period,now_date,threshold,offset_ms,DATEADD(millisecond,offset_ms,threshold) AS log_date FROM bounds CROSS JOIN(VALUES(-3),(0),(1),(2),(3))v(offset_ms))
SELECT period,offset_ms,CONVERT(varchar(16),CONVERT(binary(8),now_date),2) AS now_binary,CONVERT(varchar(16),CONVERT(binary(8),threshold),2) AS threshold_binary,CONVERT(varchar(16),CONVERT(binary(8),log_date),2) AS log_binary,CONVERT(bit,CASE WHEN log_date>threshold THEN 1 ELSE 0 END) AS blocked FROM rows ORDER BY period,offset_ms FOR JSON PATH;
"@
$taskResult=Invoke-ProjectDocker -Arguments @('compose','--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'history-synthetic-readonly',$taskSql)
if($taskResult.ExitCode-ne0){throw 'History SELECT oracle failed; values suppressed'}
$taskFirst=$taskResult.Stdout.IndexOf('[');$taskLast=$taskResult.Stdout.LastIndexOf(']')
if($taskFirst-lt0-or$taskLast-lt$taskFirst){throw 'History oracle JSON unavailable'}
$taskCases=$taskResult.Stdout.Substring($taskFirst,$taskLast-$taskFirst+1).Replace([string][char]13,'').Replace([string][char]10,'')|ConvertFrom-Json
if($taskCases.Count-ne25){throw 'History oracle count differs'}
[ordered]@{cases=$taskCases;source_read_only=$true;synthetic_only=$true;source_procedure_utf16_sha256=$taskReference.Stdout.Trim().ToLowerInvariant();generator_sha256=(Get-FileHash scripts/reverse/generate-name-history-fixtures.ps1).Hash.ToLowerInvariant();scope='Temporal DATEADD and strict greater-than with matched old names; Unicode equality and NOLOCK isolation not claimed'}|ConvertTo-Json -Depth 7 -Compress|Set-Content tests/fixtures/sql-name-history.json -Encoding utf8NoBOM
Write-Host 'History oracle:25synthetic DATEADD/quantized boundary cases; source read-only.'
