$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskQuery=@"
SET NOCOUNT ON;
SELECT p.name,OBJECT_DEFINITION(p.object_id) AS definition,
(SELECT compatibility_level FROM sys.databases WHERE name=DB_NAME()) AS compatibility_level,
DATEDIFF(minute,SYSUTCDATETIME(),SYSDATETIME()) AS civil_offset_minutes
FROM sys.procedures p WHERE p.name IN('getItemJournalID','getCharacterJournalID','AddItemJournal')
ORDER BY p.name FOR JSON PATH,INCLUDE_NULL_VALUES;
"@
$taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroesLog -w 65535 -y 0 -Q "$1"'
$r=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'readonly-journal-contract',$taskQuery)
if($r.ExitCode-ne0){throw 'Read-only journal contract recovery failed;details suppressed'}
$a=$r.Stdout.IndexOf('[');$b=$r.Stdout.LastIndexOf(']');if($a-lt0-or$b-lt$a){throw 'Missing journal procedure metadata'}
$raw=$r.Stdout.Substring($a,$b-$a+1).Replace([string][char]13,'').Replace([string][char]10,'')
$rows=@($raw|ConvertFrom-Json -DateKind String)
if($rows.Count-ne3){throw 'Journal procedure count changed'}
$procedures=@()
foreach($p in $rows){
 if(-not$p.definition-or$p.compatibility_level-ne100-or$p.civil_offset_minutes-ne0){throw 'Measured journal source clock/profile changed'}
 if($p.name-eq'AddItemJournal'){
  if($p.definition-notmatch'(?is)exec\s+getCharacterJournalID\s+@JournalID\s+output'-or$p.definition-notmatch'(?is)datediff\(dd,\s*''2011-01-01'',getdate\(\)\)%2\s*=\s*0'-or$p.definition-notmatch'(?is)INSERT\s+INTO\s+ItemJournal01\(ID,Category\)'-or$p.definition-notmatch'(?is)INSERT\s+INTO\s+ItemJournal02\(ID,Category\)'){throw 'AddItemJournal source contract changed'}
 }else{
  $table=if($p.name-eq'getItemJournalID'){'HLT_ItemJournalID'}else{'HLT_CharacterJournalID'}
  if($p.definition-notmatch('(?is)insert\s+into\s+'+$table+'\(Journal\)\s+values\(1\).*SCOPE_IDENTITY\(\).*rollback')){throw 'Journal allocator source contract changed'}
 }
 $hash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($p.definition))).ToLowerInvariant()
 $procedures+=@{name=$p.name;sha256=$hash}
}
$source='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/ItemLog.cs'
$report=[ordered]@{stage='M15.1b.2e.10b.2';measured_at_utc=[DateTimeOffset]::UtcNow.ToString('o');reference_read_only=$true;source_allocators_executed=$false;compatibility_level=100;civil_offset_minutes=0;procedures=$procedures;csharp=@{path=$source;sha256=(Get-FileHash (Join-Path $taskProject $source)).Hash.ToLowerInvariant()};schema_sha256=(Get-FileHash (Join-Path $taskProject 'docs/evidence/database/study_heroesLog.schema.json')).Hash.ToLowerInvariant();get_item_counter='HLT_ItemJournalID';add_item_journal_counter='HLT_CharacterJournalID';rollback_consumes_identity=$true;failure_result=-1;history_preservation_complete=$false;character_creation_available=$false}
$report|ConvertTo-Json -Depth 6|Set-Content (Join-Path $taskProject 'docs/evidence/auth/item-journal-source.json') -Encoding utf8NoBOM
Write-Output 'Readonly journal contract PASS:3 procedures;two distinct counters;rollback consumption;clock100/offset0;no private values exported.'
