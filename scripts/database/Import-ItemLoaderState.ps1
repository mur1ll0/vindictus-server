param([ValidateSet('Import','Verify')][string]$Mode='Import')
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskPrivate=Join-Path $taskProject '.cache/private-item-loader'
$taskFile=Join-Path $taskPrivate 'item-loader.json'
$taskEvidence=Join-Path $taskProject 'docs/evidence/database/item-loader-import.json'
function Get-TaskHash([string]$Text){[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($Text))).ToLowerInvariant()}
function Get-TaskExport {
 $taskQuery=[IO.File]::ReadAllText((Join-Path $taskProject 'scripts/database/item-loader-source.sql')).Replace('__SCHEMA_SHA256__',(Get-FileHash (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json')).Hash.ToLowerInvariant())
 $sh='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
 $r=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$sh,'readonly-item-loader-export',$taskQuery)
 if($r.ExitCode-ne0){throw 'Read-only item-loader export failed;private details suppressed'}
 $a=$r.Stdout.IndexOf('{');$b=$r.Stdout.LastIndexOf('}');if($a-lt0-or$b-lt$a){throw 'Missing item-loader export'}
 $json=$r.Stdout.Substring($a,$b-$a+1).Replace([string][char]13,'').Replace([string][char]10,'')
 $data=$json|ConvertFrom-Json -DateKind String
 if(@($data.quick_slots).Count-ne261-or@($data.expanded_slots).Count-ne0-or@($data.manufacture).Count-ne0-or@($data.recipes).Count-ne0-or@($data.new_recipes).Count-ne0){throw ('Item loader source counts changed:'+(@(@($data.quick_slots).Count,@($data.expanded_slots).Count,@($data.manufacture).Count,@($data.recipes).Count,@($data.new_recipes).Count)-join ','))}
 return [pscustomobject]@{Json=$json;QuerySha256=Get-TaskHash $taskQuery}
}
function Invoke-TaskImport([string]$Json,[bool]$Verify){
 $taskArgs=@('compose','--project-directory',$taskProject,'exec','-T','server','/app/vindictus-server','import-item-loader-state','--input-stdin');if($Verify){$taskArgs+='--verify-only'}
 $r=Invoke-ProjectDocker -Arguments $taskArgs -InputText $Json
 if($r.ExitCode-ne0){throw 'Item loader preservation failed;private values suppressed'}
 try{$summary=$r.Stdout.Trim()|ConvertFrom-Json -DateKind String}catch{throw 'Invalid sanitized item-loader summary'}
 if(($summary.rows -join ',')-ne'261,0,0,0,0'-or($summary.fields -join ',')-ne'4,3,5,4,4'-or$summary.verified-ne261-or$summary.identities_initialized-ne5-or$summary.character_creation_available){throw 'Item loader summary mismatch'}
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
 $replay=Invoke-TaskImport $export.Json $false;if($replay.inserted-ne0){throw 'Item loader replay inserted rows'}
 $verified=Invoke-TaskImport $export.Json $true
 $after=Get-TaskExport;if((Get-TaskHash $after.Json)-ne(Get-TaskHash $export.Json)){throw 'Item loader reference changed during read-only preservation'}
 $report=[ordered]@{stage='M15.1b.2e.10b.22d';verified_at_utc=[DateTimeOffset]::UtcNow.ToString('o');source_read_only=$true;source_export_unchanged=$true;source_rows=@(261,0,0,0,0);source_columns=@(4,3,5,4,4);identities=5;export_sha256=Get-TaskHash $export.Json;query_sha256=$export.QuerySha256;schema_sha256=(Get-FileHash (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json')).Hash.ToLowerInvariant();private_acl='current Windows user and SYSTEM only';imported=$imported;replay=$replay;verified=$verified;post_restart_verified=$false;character_creation_available=$false}
}else{
 $report=Get-Content $taskEvidence -Raw|ConvertFrom-Json -DateKind String
 $json=[IO.File]::ReadAllText($taskFile);if((Get-TaskHash $json)-ne$report.export_sha256){throw 'Private item-loader export digest changed'}
 $verified=Invoke-TaskImport $json $true
 $report.post_restart_verified=$true
 $report|Add-Member -Force -NotePropertyName post_restart_verified_at_utc -NotePropertyValue ([DateTimeOffset]::UtcNow.ToString('o'))
 $report|Add-Member -Force -NotePropertyName post_restart_summary -NotePropertyValue $verified
}
$report|ConvertTo-Json -Depth 7|Set-Content $taskEvidence -Encoding utf8NoBOM
Write-Host ('Item loader '+$Mode+' PASS:261 rows/20 fields/five identities;source unchanged;creation=false.')
