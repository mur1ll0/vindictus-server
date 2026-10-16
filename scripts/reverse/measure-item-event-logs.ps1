$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$q=@"
SET NOCOUNT ON;
SELECT p.name,OBJECT_DEFINITION(p.object_id) AS definition,
(SELECT compatibility_level FROM sys.databases WHERE name=DB_NAME()) AS compatibility_level,
DATEDIFF(minute,SYSUTCDATETIME(),SYSDATETIME()) AS civil_offset_minutes
FROM sys.procedures p WHERE p.name IN('AddItemLog','AddItemLedger') ORDER BY p.name FOR JSON PATH,INCLUDE_NULL_VALUES;
"@
$sh='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroesLog -w 65535 -y 0 -Q "$1"'
$r=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$sh,'readonly-item-event-contract',$q)
if($r.ExitCode-ne0){throw 'Readonly item event recovery failed;details suppressed'}
$a=$r.Stdout.IndexOf('[');$b=$r.Stdout.LastIndexOf(']');if($a-lt0-or$b-lt$a){throw 'Missing item log metadata'}
$raw=$r.Stdout.Substring($a,$b-$a+1).Replace([string][char]13,'').Replace([string][char]10,'');$rows=@($raw|ConvertFrom-Json -DateKind String)
if($rows.Count-ne2){throw 'Source procedure count changed'}
$procedures=@()
foreach($p in $rows){
 if(-not$p.definition-or$p.compatibility_level-ne100-or$p.civil_offset_minutes-ne0){throw 'Source clock/profile changed'}
 if($p.definition-notmatch'(?is)datediff\(dd,\s*''2011-01-01'',getdate\(\)\)%2\s*=\s*0'){throw 'Item log routing changed'}
 $table=if($p.name-eq'AddItemLog'){'ItemLog'}else{'ItemLedger'}
 foreach($suffix in '01','02'){if($p.definition-notmatch('(?is)INSERT\s+INTO\s+(\[dbo\]\.)?\[?'+$table+$suffix+'\]?')){throw 'Item log target changed'}}
 if($p.name-eq'AddItemLedger'-and$p.definition-notmatch'(?is)if\(@Event in\(18\)\).*?return 0;'){throw 'Ledger suppression changed'}
 $procedures+=@{name=$p.name;sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($p.definition))).ToLowerInvariant()}
}
$paths=@('legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/ItemLog.cs','legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/LogData.cs','legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/ItemImpl.cs','legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/ItemOwner.cs','legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore.OperationLogSystem/ItemLogManager.cs')
$report=[ordered]@{stage='M15.1b.2e.10b.3';measured_at_utc=[DateTimeOffset]::UtcNow.ToString('o');reference_read_only=$true;source_procedures_executed=$false;compatibility_level=100;civil_offset_minutes=0;procedures=$procedures;sources=@($paths|ForEach-Object{@{path=$_;sha256=(Get-FileHash (Join-Path $taskProject $_)).Hash.ToLowerInvariant()}});schema_sha256=(Get-FileHash (Join-Path $taskProject 'docs/evidence/database/study_heroesLog.schema.json')).Hash.ToLowerInvariant();item_log_journal_minus_one_attempted=$true;item_ledger_journal_minus_one_skipped=$true;item_log_captured_process_civil_time=$true;item_ledger_default_sql_civil_time=$true;historical_log_preservation_complete=$false;character_creation_available=$false}
$report|ConvertTo-Json -Depth 6|Set-Content (Join-Path $taskProject 'docs/evidence/auth/item-event-log-source.json') -Encoding utf8NoBOM
Write-Output 'Readonly item event contract PASS:2 procedures;clock100/offset0;partition and ledger event18 suppression recovered;no private values exported.'
