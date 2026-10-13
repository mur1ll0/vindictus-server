$ErrorActionPreference='Stop'
. ./scripts/dev/docker-process.ps1
$taskSql=@"
SET NOCOUNT ON;
WITH masks AS (SELECT m FROM (VALUES(0),(1),(2),(3),(4),(5),(6),(7),(8),(9),(10),(11),(12),(13),(14),(15))v(m)),
owners AS (SELECT who FROM (VALUES(N'Owner'),(N'OWNER '),(N'Other'),(N''),(N'   '),(N'OwnerX'))v(who))
SELECT m AS mask,who,
JSON_QUERY(COALESCE((SELECT CONVERT(varchar(max),CONVERT(varbinary(max),v.character_name),2) AS utf16_hex FROM
(VALUES(1,N'Owner',-51,N'LabFiona',m&1),(2,N'OWNER ',777,N'Éλ',m&2),(3,N'Other',31,N'OtherChar',0),(4,N'owner',-91,N'LabFiona',m&1),(5,N'',1,N'EmptyOwner',0),(6,N'   ',2,N'SpaceOwner',0),(7,REPLICATE(N'Z',50),3,N'LongOwner',0))v(row_id,nexon_id,nexon_sn,character_name,created)
WHERE v.nexon_id COLLATE SQL_Latin1_General_CP1_CI_AS=who COLLATE SQL_Latin1_General_CP1_CI_AS AND v.created=0 ORDER BY v.row_id FOR JSON PATH),'[]')) AS names,
JSON_QUERY(COALESCE((SELECT v.title_id FROM
(VALUES(1,N'Owner',-51,-2147483648,m&4),(2,N'OWNER ',777,2147483647,m&8),(3,N'Other',31,42,0),(4,N'',1,-1,0),(5,N'   ',2,0,0),(6,REPLICATE(N'Z',50),3,17,0))v(row_id,nexon_id,nexon_sn,title_id,title_given)
WHERE v.nexon_id COLLATE SQL_Latin1_General_CP1_CI_AS=who COLLATE SQL_Latin1_General_CP1_CI_AS AND v.title_given=0 ORDER BY v.row_id FOR JSON PATH),'[]')) AS titles
FROM masks CROSS JOIN owners ORDER BY m,who COLLATE Latin1_General_BIN2 FOR JSON PATH;
"@
$taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
$taskResult=Invoke-ProjectDocker -Arguments @('compose','--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'reservation-synthetic-read-only-oracle',$taskSql)
if($taskResult.ExitCode-ne0){throw 'Read-only synthetic reservation oracle failed; values suppressed'}
$taskFirst=$taskResult.Stdout.IndexOf('[');$taskLast=$taskResult.Stdout.LastIndexOf(']')
if($taskFirst-lt0-or$taskLast-lt$taskFirst){throw 'Synthetic oracle JSON missing'}
$taskJson=$taskResult.Stdout.Substring($taskFirst,$taskLast-$taskFirst+1).Replace([string][char]13,'').Replace([string][char]10,'')
$taskCases=$taskJson|ConvertFrom-Json -DateKind String
if($taskCases.Count-ne96){throw 'Synthetic oracle count differs'}
[ordered]@{cases=$taskCases;source_read_only=$true;synthetic_only=$true;ordering_used_for_membership_comparison_only=$true;sources=@(('legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore.Processor/QueryReservedInfoProcessor.cs','legacy/decompiled/FrontendServiceCore-6be475728091/FrontendServiceCore.States/CharacterList.cs','scripts/reverse/generate-reservation-query-fixtures.ps1')|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}})}|ConvertTo-Json -Depth 9 -Compress|Set-Content tests/fixtures/sql-reservation-query.json -Encoding utf8NoBOM
[IO.File]::WriteAllText((Join-Path $PWD '.cache/verification/reservation-query-oracle.log'),"SELECT only;96synthetic cases; no original records queried or changed.`n",[Text.UTF8Encoding]::new($false))
Write-Host 'Reservation SELECT oracle:96synthetic cases; only virtual VALUES; no original data read or changed.'