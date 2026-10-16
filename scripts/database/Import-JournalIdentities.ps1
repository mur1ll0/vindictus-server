param([ValidateSet('Import','Verify')][string]$Mode='Import')
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskPrivate=Join-Path $taskProject '.cache/private-journals'
$taskFile=Join-Path $taskPrivate 'identities.json'
$taskEvidence=Join-Path $taskProject 'docs/evidence/database/journal-identities-import.json'
function Get-TaskHash([string]$Text){[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($Text))).ToLowerInvariant()}
function Get-TaskExport {
 $taskQuery=@"
SET NOCOUNT ON;
SELECT 1 AS format_version,'study_heroesLog.journal-identities' AS source,
(SELECT CONVERT(varchar(20),last_value) FROM sys.identity_columns WHERE object_id=OBJECT_ID('dbo.HLT_ItemJournalID') AND name='JournalID' AND CONVERT(bigint,seed_value)=611635750 AND CONVERT(bigint,increment_value)=1) AS item_last,
(SELECT CONVERT(varchar(20),last_value) FROM sys.identity_columns WHERE object_id=OBJECT_ID('dbo.HLT_CharacterJournalID') AND name='JournalID' AND CONVERT(bigint,seed_value)=4419625 AND CONVERT(bigint,increment_value)=1) AS character_last
FOR JSON PATH,INCLUDE_NULL_VALUES,WITHOUT_ARRAY_WRAPPER;
"@
 $sh='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroesLog -w 65535 -y 0 -Q "$1"'
 $r=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$sh,'readonly-journal-export',$taskQuery)
 if($r.ExitCode-ne0){throw 'Read-only journal export failed;private details suppressed'}
 $a=$r.Stdout.IndexOf('{');$b=$r.Stdout.LastIndexOf('}');if($a-lt0-or$b-lt$a){throw 'Missing journal export'}
 $json=$r.Stdout.Substring($a,$b-$a+1).Replace([string][char]13,'').Replace([string][char]10,'')
 $data=$json|ConvertFrom-Json -DateKind String
 if($null-eq$data.item_last-or$null-eq$data.character_last){throw 'Journal identity metadata missing or changed;inspect privately'}
 return [pscustomobject]@{Json=$json;QuerySha256=Get-TaskHash $taskQuery}
}
function Invoke-TaskImport([string]$Json,[bool]$Verify){
 $taskArgs=@('compose','--project-directory',$taskProject,'exec','-T','server','/app/vindictus-server','import-journal-identities','--input-stdin');if($Verify){$taskArgs+='--verify-only'}
 $r=Invoke-ProjectDocker -Arguments $taskArgs -InputText $Json
 if($r.ExitCode-ne0){throw 'Journal preservation failed;private values suppressed'}
 try{$summary=$r.Stdout.Trim()|ConvertFrom-Json -DateKind String}catch{throw 'Invalid sanitized journal summary'}
 if($summary.initialized-ne2-or$summary.character_creation_available){throw 'Journal preservation summary mismatch'}
 return $summary
}
if($Mode-eq'Import'){
 $export=Get-TaskExport
 New-Item -ItemType Directory -Force -Path $taskPrivate|Out-Null
 $acl=[Security.AccessControl.DirectorySecurity]::new();$acl.SetAccessRuleProtection($true,$false)
 foreach($sid in @([Security.Principal.WindowsIdentity]::GetCurrent().User,[Security.Principal.SecurityIdentifier]::new('S-1-5-18'))){$acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($sid,'FullControl','ContainerInherit,ObjectInherit','None','Allow'))}
 [IO.FileSystemAclExtensions]::SetAccessControl([IO.DirectoryInfo]::new($taskPrivate),$acl)
 [IO.File]::WriteAllText($taskFile,$export.Json,[Text.UTF8Encoding]::new($false))
 $imported=Invoke-TaskImport $export.Json $false
 $replay=Invoke-TaskImport $export.Json $false
 $verified=Invoke-TaskImport $export.Json $true
 $after=Get-TaskExport;if((Get-TaskHash $after.Json)-ne(Get-TaskHash $export.Json)){throw 'Journal reference changed during read-only preservation'}
 $report=[ordered]@{stage='M15.1b.2e.10b.2';verified_at_utc=[DateTimeOffset]::UtcNow.ToString('o');source_read_only=$true;source_export_unchanged=$true;source_counters=2;identity_rows_copied=0;export_sha256=Get-TaskHash $export.Json;query_sha256=$export.QuerySha256;schema_sha256=(Get-FileHash (Join-Path $taskProject 'docs/evidence/database/study_heroesLog.schema.json')).Hash.ToLowerInvariant();private_acl='current Windows user and SYSTEM only';imported=$imported;replay=$replay;verified=$verified;post_restart_verified=$false;character_creation_available=$false}
}else{
 $report=Get-Content $taskEvidence -Raw|ConvertFrom-Json -DateKind String
 $json=[IO.File]::ReadAllText($taskFile);if((Get-TaskHash $json)-ne$report.export_sha256){throw 'Private journal export digest changed'}
 $verified=Invoke-TaskImport $json $true
 $report.post_restart_verified=$true
 $report|Add-Member -Force -NotePropertyName post_restart_verified_at_utc -NotePropertyValue ([DateTimeOffset]::UtcNow.ToString('o'))
 $report|Add-Member -Force -NotePropertyName post_restart_summary -NotePropertyValue $verified
}
$report|ConvertTo-Json -Depth 7|Set-Content $taskEvidence -Encoding utf8NoBOM
Write-Host ('Journal identities '+$Mode+' PASS:2 independent counters;source unchanged;replay never reseeds;character creation=false.')
