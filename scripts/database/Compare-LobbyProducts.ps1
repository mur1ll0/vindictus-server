$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskQuery=@"
SET NOCOUNT ON;
SELECT i AS case_id,CONVERT(varchar(max),CONVERT(varbinary(max),product_name),2) AS product_name_utf16_hex,
CAST(CASE WHEN product_name COLLATE SQL_Latin1_General_CP1_CI_AS=N'CharacterSlot' THEN 1 ELSE 0 END AS bit) AS character_slot,
CAST(CASE WHEN product_name COLLATE SQL_Latin1_General_CP1_CI_AS=N'PremiumCharacterSlot' THEN 1 ELSE 0 END AS bit) AS premium_slot
FROM (VALUES(0,CONVERT(nvarchar(128),N'CharacterSlot')),(1,N'CHARACTERSLOT'),(2,N'characterslot   '),(3,N' PremiumCharacterSlot'),(4,N'PremiumCharacterSlot'),(5,N'PREMIUMCHARACTERSLOT '),(6,N''),(7,N'CharacterSlot_extra'),(8,N'CharacterSlot'+NCHAR(0))) AS synthetic(i,product_name)
ORDER BY i FOR JSON PATH;
"@
$taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
$taskResult=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'lobby-products-select-only',$taskQuery)
if($taskResult.ExitCode-ne0){throw 'Synthetic product collation SELECT failed; details suppressed'}
$taskData=$taskResult.Stdout.Trim()|ConvertFrom-Json
if(@($taskData).Count-ne9){throw 'Product oracle count mismatch'}
$taskHash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($taskQuery))).ToLowerInvariant()
$taskCases=[ordered]@{cases=$taskData;query_sha256=$taskHash;schema_sha256=(Get-FileHash (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json')).Hash.ToLowerInvariant();method='SELECT synthetic nvarchar comparisons in recovered SQL_Latin1_General_CP1_CI_AS; no procedure executed, no table reads/writes';source_rows_exported=$false;sqlserver_persistent_writes=$false}
$taskCases|ConvertTo-Json -Depth 5|Set-Content (Join-Path $taskProject 'tests/fixtures/sql-lobby-products.json') -Encoding utf8NoBOM
Write-Host 'Product collation SELECT-only oracle:9synthetic cases; no source rows/persistent writes.'
