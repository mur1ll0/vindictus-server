$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskSql=@"
SET NOCOUNT ON;
WITH Inputs AS (
 SELECT label,CONVERT(nvarchar(24),value) COLLATE SQL_Latin1_General_CP1_CI_AS AS NexonID
 FROM (VALUES ('same',N'LimitedFixture'),('case',N'LIMITEDFIXTURE'),('space',N'LimitedFixture '),
 ('different',N'Other'),('leading',N' LimitedFixture'),('empty',N''),('spaces',N'   '))v(label,value)
)
SELECT (SELECT COUNT_BIG(*) FROM dbo.LimitedAccount) AS source_rows,
 JSON_QUERY((SELECT label,NexonID AS name,CAST(CASE WHEN NexonID=N'LimitedFixture' THEN 1 ELSE 0 END AS bit) AS matched
 FROM Inputs ORDER BY label FOR JSON PATH)) AS cases,
 (SELECT COUNT(*) FROM sys.key_constraints WHERE parent_object_id=OBJECT_ID('dbo.LimitedAccount')) AS source_keys
FOR JSON PATH,WITHOUT_ARRAY_WRAPPER;
"@
$taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
$taskResult=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'limited-oracle',$taskSql)
if($taskResult.ExitCode-ne0){throw 'Read-only LimitedAccount oracle failed; details suppressed'}
$taskFirst=$taskResult.Stdout.IndexOf('{');$taskLast=$taskResult.Stdout.LastIndexOf('}')
$taskData=$taskResult.Stdout.Substring($taskFirst,$taskLast-$taskFirst+1).Replace([string][char]13,'').Replace([string][char]10,'')|ConvertFrom-Json -DateKind String
$taskData|Add-Member -NotePropertyName source_writes -NotePropertyValue $false
$taskData|Add-Member -NotePropertyName verified_at_utc -NotePropertyValue ([DateTimeOffset]::UtcNow.ToString('o'))
$taskData|Add-Member -NotePropertyName query_sha256 -NotePropertyValue ([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($taskSql))).ToLowerInvariant())
$taskData|ConvertTo-Json -Depth 5|Set-Content (Join-Path $taskProject 'docs/evidence/database/limited-account-oracle.json') -Encoding utf8NoBOM
Write-Host ('LimitedAccount SQL oracle: source rows='+$taskData.source_rows+'; keys='+$taskData.source_keys+'; synthetic cases='+$taskData.cases.Count+'; source writes=false')
if($taskData.source_rows-ne0){throw 'Nonempty source requires private bounded snapshot import before completion; no names exported'}
