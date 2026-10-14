$ErrorActionPreference='Stop'
. ./scripts/dev/docker-process.ps1
$taskSql=@"
SET NOCOUNT ON;
SELECT CONVERT(varchar(max),CONVERT(varbinary(max),value),2) AS utf16_hex,CONVERT(bit,CASE WHEN value COLLATE SQL_Latin1_General_CP1_CI_AS=N'Murael' COLLATE SQL_Latin1_General_CP1_CI_AS THEN 1 ELSE 0 END) AS equal FROM(VALUES(N'Murael'),(N'MURAEL'),(N'Murael '),(N'murael'),(N'Murael'+NCHAR(10)),(N'OtherName'),(N'λ'),(N''))v(value) FOR JSON PATH;
"@
$taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
$taskResult=Invoke-ProjectDocker -Arguments @('compose','--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'candidate-synthetic-equality-readonly',$taskSql)
if($taskResult.ExitCode-ne0){throw 'Synthetic name equality query failed; values suppressed'}
$taskFirst=$taskResult.Stdout.IndexOf('[');$taskLast=$taskResult.Stdout.LastIndexOf(']')
if($taskFirst-lt0-or$taskLast-lt$taskFirst){throw 'Synthetic equality JSON missing'}
$taskCases=$taskResult.Stdout.Substring($taskFirst,$taskLast-$taskFirst+1).Replace([string][char]13,'').Replace([string][char]10,'')|ConvertFrom-Json
if($taskCases.Count-ne8){throw 'Synthetic equality count differs'}
[ordered]@{candidate='Murael';cases=$taskCases;source_read_only=$true;synthetic_only=$true;collation='SQL_Latin1_General_CP1_CI_AS';generator_sha256=(Get-FileHash scripts/reverse/generate-name-candidate-equality.ps1).Hash.ToLowerInvariant()}|ConvertTo-Json -Depth 6 -Compress|Set-Content tests/fixtures/sql-name-candidate-equality.json -Encoding utf8NoBOM
Write-Output 'Synthetic candidate equality:8measured stored values; no original rows read or changed.'
