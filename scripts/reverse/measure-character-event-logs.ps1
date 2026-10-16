$ErrorActionPreference='Stop'
. ./scripts/dev/docker-process.ps1
$query=@"
SET NOCOUNT ON;
SELECT OBJECT_DEFINITION(OBJECT_ID('dbo.AddCharacterSkillLog')) AS skill_definition,
OBJECT_DEFINITION(OBJECT_ID('dbo.AddCharacterTitleLog')) AS title_definition,
DATEPART(tzoffset,SYSDATETIMEOFFSET()) AS civil_offset_minutes,
(SELECT compatibility_level FROM sys.databases WHERE database_id=DB_ID()) AS compatibility_level,
JSON_QUERY((SELECT n.input_ms,CONVERT(varchar(16),CONVERT(binary(8),d.source_datetime),2) AS source_binary,
CONVERT(varchar(20),DATEDIFF_BIG(day,CONVERT(date,'00010101'),CONVERT(date,t.converted))*864000000000+DATEDIFF_BIG(nanosecond,CONVERT(datetime2(7),CONVERT(date,t.converted)),t.converted)/100) AS converted_ticks
FROM (SELECT TOP(256) CONVERT(int,ROW_NUMBER()OVER(ORDER BY object_id)-1) AS input_ms FROM sys.all_objects) n
CROSS APPLY(SELECT DATEADD(millisecond,n.input_ms,CONVERT(datetime,'20110101')) AS source_datetime)d
CROSS APPLY(SELECT CONVERT(datetime2(7),d.source_datetime) AS converted)t
ORDER BY n.input_ms FOR JSON PATH)) AS synthetic_clock
FOR JSON PATH,WITHOUT_ARRAY_WRAPPER;
"@
$sh='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroesLog -w 65535 -y 0 -Q "$1"'
$r=Invoke-ProjectDocker -Arguments @('compose','--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$sh,'readonly-character-logs',$query)
if($r.ExitCode-ne0){throw 'Readonly log procedure/clock measurement failed;details private'}
$a=$r.Stdout.IndexOf('{');$b=$r.Stdout.LastIndexOf('}');if($a-lt0-or$b-lt$a){throw 'Readonly log result missing'}
$json=$r.Stdout.Substring($a,$b-$a+1).Replace([string][char]13,'').Replace([string][char]10,'')
$v=$json|ConvertFrom-Json -DateKind String
if(-not$v.skill_definition-or-not$v.title_definition-or$v.civil_offset_minutes-ne0-or$v.synthetic_clock.Count-ne256){throw 'Recovered logging profile not admitted'}
[IO.File]::WriteAllText((Join-Path $PWD '.cache/private-content/creation-character-log-procedures.json'),$json,[Text.UTF8Encoding]::new($false))
$utf8=[Text.UTF8Encoding]::new($false)
function Hash([string]$s){[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($utf8.GetBytes($s))).ToLowerInvariant()}
$fixture=[ordered]@{source='SQLServer.CONVERT.datetime2';compatibility_level=$v.compatibility_level;query_sha256=(Hash $query);skill_procedure_sha256=(Hash $v.skill_definition);title_procedure_sha256=(Hash $v.title_definition);civil_offset_minutes=0;cases=$v.synthetic_clock}
[IO.File]::WriteAllText((Join-Path $PWD 'tests/fixtures/sql-character-log-clock.json'),($fixture|ConvertTo-Json -Depth 6 -Compress),$utf8)
Write-Output 'Readonly original log procedures recovered privately;256 synthetic clock conversions;offset0.'
