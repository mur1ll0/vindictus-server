param()
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskSql=@'
SET NOCOUNT ON;
WITH bytes AS (SELECT 0 AS n UNION ALL SELECT n+1 FROM bytes WHERE n<255)
SELECT CONVERT(int,COLLATIONPROPERTY('SQL_Latin1_General_CP1_CI_AS','CodePage')) AS code_page,
 JSON_QUERY((SELECT n AS value,UNICODE(CONVERT(nvarchar(1),CONVERT(varchar(1),CONVERT(binary(1),n)) COLLATE SQL_Latin1_General_CP1_CI_AS)) AS unit
 FROM bytes ORDER BY n FOR JSON PATH)) AS cases
FOR JSON PATH,WITHOUT_ARRAY_WRAPPER OPTION(MAXRECURSION 256);
'@
$taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
$taskResult=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'oracle',$taskSql)
if($taskResult.ExitCode-ne0){throw 'Read-only token oracle failed; credentials/output omitted'}
$taskStart=$taskResult.Stdout.IndexOf('{');$taskEnd=$taskResult.Stdout.LastIndexOf('}')
if($taskStart-lt0 -or $taskEnd-lt$taskStart){throw 'Missing token oracle JSON'}
[IO.File]::WriteAllText((Join-Path $taskProject '.cache/verification/token-codepage-oracle.log'),$taskResult.Stdout)
# sqlcmd can split FOR JSON at its 2033-character row boundary. All values here are numeric.
$taskJson=$taskResult.Stdout.Substring($taskStart,$taskEnd-$taskStart+1).Replace("`r",'').Replace("`n",'')
$taskData=$taskJson|ConvertFrom-Json -DateKind String
if($taskData.code_page-ne1252 -or $taskData.cases.Count-ne256){throw 'Unexpected token profile'}
$taskData|Add-Member -NotePropertyName method -NotePropertyValue 'SELECT-only synthetic byte values and COLLATIONPROPERTY; no user token values read or exported'
$taskData|Add-Member -NotePropertyName verified_at_utc -NotePropertyValue ([DateTimeOffset]::UtcNow.ToString('o'))
$taskData|Add-Member -NotePropertyName query_sha256 -NotePropertyValue ([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($taskSql))).ToLowerInvariant())
$taskData|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $taskProject 'docs/evidence/database/token-codepage.json') -Encoding utf8NoBOM
Write-Host 'Token SQL oracle: codepage=1252; synthetic bytes=256; writes=0'
