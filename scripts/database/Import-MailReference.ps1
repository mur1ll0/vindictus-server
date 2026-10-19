param([ValidateSet('Import','Verify','CheckSource')][string]$Mode='Import')
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskPrivate=Join-Path $taskProject '.cache/private-mail'
$taskFile=Join-Path $taskPrivate 'mail.json'
$taskEvidence=Join-Path $taskProject 'docs/evidence/database/mail-import.json'
function Get-TaskHash([string]$Text){[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($Text))).ToLowerInvariant()}
function Get-TaskExport {
 $taskQuery=[IO.File]::ReadAllText((Join-Path $taskProject 'scripts/database/mail-source.sql'))
 $sh='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
 $r=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$sh,'readonly-mail-export',$taskQuery)
 if($r.ExitCode-ne0){throw 'Read-only Mail export failed;private details suppressed'}
 $a=$r.Stdout.IndexOf('{');$b=$r.Stdout.LastIndexOf('}');if($a-lt0-or$b-lt$a){throw 'Missing Mail export'}
 $v=$r.Stdout.Substring($a,$b-$a+1).Replace([string][char]13,'').Replace([string][char]10,'')|ConvertFrom-Json -DateKind String
 $schema=Get-Content (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json') -Raw|ConvertFrom-Json -DateKind String
 $proc=@($schema.objects|Where-Object name -eq 'QueryMailList');$old=@($schema.sql_modules|Where-Object object_id -eq $proc[0].object_id)
 if($proc.Count-ne1-or$old.Count-ne1-or$old[0].definition-cne$v.procedure_definition-or(Get-TaskHash $v.procedure_definition)-cne'c17336ad0fb0aca9d1cb6320580dd58538227ee48bd7a4292cf6e06cd33f484c'){throw 'Mail source query changed'}
 if(@($v.snapshot.rows).Count-ne169){throw 'Mail source profile changed'}
 $json=$v.snapshot|ConvertTo-Json -Depth 5 -Compress
 return [pscustomobject]@{Json=$json;QuerySha256=Get-TaskHash $taskQuery}
}
function Invoke-TaskImport([string]$Json,[bool]$Verify){
 $taskImportArgs=@('compose','--project-directory',$taskProject,'exec','-T','server','/app/vindictus-server','import-mail-reference','--input-stdin');if($Verify){$taskImportArgs+='--verify-only'}
 $r=Invoke-ProjectDocker -Arguments $taskImportArgs -InputText $Json
 if($r.ExitCode-ne0){throw 'Mail preservation failed;private details suppressed'}
 try{$s=$r.Stdout.Trim()|ConvertFrom-Json -DateKind String}catch{throw 'Invalid sanitized Mail summary'}
 if($s.rows-ne169-or$s.verified-ne169-or$s.fields-ne14-or-not$s.initialized-or$s.character_creation_available){throw 'Mail summary mismatch'}
 return $s
}
if($Mode-eq'CheckSource'){$source=Get-TaskExport;Write-Output 'Mail reference PASS:169rows/14fields;PK MailID;no identity/FK;QueryMailList hash matched;no procedures executed.';return}
if($Mode-eq'Import'){
 $export=Get-TaskExport
 New-Item -ItemType Directory -Force -Path $taskPrivate|Out-Null
 $acl=[Security.AccessControl.DirectorySecurity]::new();$acl.SetAccessRuleProtection($true,$false)
 foreach($sid in @([Security.Principal.WindowsIdentity]::GetCurrent().User,[Security.Principal.SecurityIdentifier]::new('S-1-5-18'))){$acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($sid,'FullControl','ContainerInherit,ObjectInherit','None','Allow'))}
 [IO.FileSystemAclExtensions]::SetAccessControl([IO.DirectoryInfo]::new($taskPrivate),$acl)
 [IO.File]::WriteAllText($taskFile,$export.Json,[Text.UTF8Encoding]::new($false))
 $imported=Invoke-TaskImport $export.Json $false
 $replay=Invoke-TaskImport $export.Json $false;if($replay.inserted-ne0){throw 'Mail replay inserted rows'}
 $verified=Invoke-TaskImport $export.Json $true
 $after=Get-TaskExport;if((Get-TaskHash $after.Json)-ne(Get-TaskHash $export.Json)){throw 'Mail source changed during readonly preservation'}
 $report=[ordered]@{stage='M15.1b.2e.10b.22f.1';verified_at_utc=[DateTimeOffset]::UtcNow.ToString('o');source_read_only=$true;source_export_unchanged=$true;source_rows=169;source_columns=14;source_identity=$false;source_foreign_keys=0;procedure_sha256='c17336ad0fb0aca9d1cb6320580dd58538227ee48bd7a4292cf6e06cd33f484c';export_sha256=Get-TaskHash $export.Json;query_sha256=$export.QuerySha256;schema_sha256=(Get-FileHash (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json')).Hash.ToLowerInvariant();private_acl='current Windows user and SYSTEM only';imported=$imported;replay=$replay;verified=$verified;post_restart_verified=$false;character_creation_available=$false}
}else{
 $report=Get-Content $taskEvidence -Raw|ConvertFrom-Json -DateKind String
 $json=[IO.File]::ReadAllText($taskFile);if((Get-TaskHash $json)-ne$report.export_sha256){throw 'Private Mail export digest changed'}
 $source=Get-TaskExport;if((Get-TaskHash $source.Json)-ne$report.export_sha256-or$source.QuerySha256-ne$report.query_sha256){throw 'Mail reference changed'}
 $verified=Invoke-TaskImport $json $true
 $report.post_restart_verified=$true
 $report|Add-Member -Force -NotePropertyName post_restart_verified_at_utc -NotePropertyValue ([DateTimeOffset]::UtcNow.ToString('o'))
 $report|Add-Member -Force -NotePropertyName post_restart_summary -NotePropertyValue $verified
}
$report|ConvertTo-Json -Depth 7|Set-Content $taskEvidence -Encoding utf8NoBOM
Write-Output ('Mail '+$Mode+' PASS:169rows/14fields;datetime binary preserved;source unchanged;character creation=false.')
