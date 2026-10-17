param([ValidateSet('Import','Verify')][string]$Mode='Import',[string]$RequireEmptyOwner='')
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskPrivate=Join-Path $taskProject '.cache/private-shared-items'
$taskFile=Join-Path $taskPrivate 'shared-items.json'
$taskEvidence=Join-Path $taskProject 'docs/evidence/database/shared-items-import.json'
function Get-TaskHash([string]$Text){[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($Text))).ToLowerInvariant()}
function Get-TaskExport {
 $taskQuery=@"
SET NOCOUNT ON;
SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;
BEGIN TRANSACTION;
SELECT 1 AS format_version,'study_heroes.shared-items' AS source,
(SELECT CONVERT(varchar(20),last_value) FROM sys.identity_columns WHERE object_id=OBJECT_ID('dbo.ItemSharedStorageStatus') AND name='ID' AND CONVERT(bigint,seed_value)=1 AND CONVERT(bigint,increment_value)=1) AS last_value,
JSON_QUERY((SELECT CONVERT(varchar(20),ID) AS id,OwnerUID AS owner_uid,CONVERT(int,Collection) AS collection,Slot AS slot FROM dbo.ItemShared ORDER BY ID FOR JSON PATH,INCLUDE_NULL_VALUES)) AS items,
JSON_QUERY((SELECT CONVERT(varchar(20),ID) AS id,OwnerUID AS owner_uid,CONVERT(int,StorageType) AS storage_type,CONVERT(int,Storage) AS storage,CONVERT(varchar(16),CONVERT(binary(8),ExpireTime),2) AS expire_binary FROM dbo.ItemSharedStorageStatus ORDER BY ID FOR JSON PATH,INCLUDE_NULL_VALUES)) AS storage
FOR JSON PATH,INCLUDE_NULL_VALUES,WITHOUT_ARRAY_WRAPPER;
COMMIT;
"@
 $sh='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
 $r=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$sh,'readonly-shared-export',$taskQuery)
 if($r.ExitCode-ne0){throw 'Read-only shared export failed;private details suppressed'}
 $a=$r.Stdout.IndexOf('{');$b=$r.Stdout.LastIndexOf('}');if($a-lt0-or$b-lt$a){throw 'Missing shared export'}
 $json=$r.Stdout.Substring($a,$b-$a+1).Replace([string][char]13,'').Replace([string][char]10,'')
 $data=$json|ConvertFrom-Json -DateKind String
 if(@($data.items).Count-ne8-or@($data.storage).Count-ne5-or$null-eq$data.last_value){throw 'Shared reference profile changed;inspect privately'}
 return [pscustomobject]@{Json=$json;QuerySha256=Get-TaskHash $taskQuery}
}
function Invoke-TaskImport([string]$Json,[bool]$Verify){
 $taskArgs=@('compose','--project-directory',$taskProject,'exec','-T','server','/app/vindictus-server','import-shared-items','--input-stdin');if($Verify){$taskArgs+='--verify-only'};if($RequireEmptyOwner){$taskArgs+=@('--require-empty-owner',$RequireEmptyOwner)}
 $r=Invoke-ProjectDocker -Arguments $taskArgs -InputText $Json
 if($r.ExitCode-ne0){throw 'Shared preservation failed;private values suppressed'}
 try{$summary=$r.Stdout.Trim()|ConvertFrom-Json -DateKind String}catch{throw 'Invalid sanitized shared summary'}
 if($summary.items-ne8-or$summary.storage-ne5-or-not$summary.identity_initialized-or$summary.character_creation_available-or($RequireEmptyOwner-and-not$summary.empty_owner_verified)){throw 'Shared preservation summary mismatch'}
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
 $after=Get-TaskExport;if((Get-TaskHash $after.Json)-ne(Get-TaskHash $export.Json)){throw 'Shared reference changed during read-only preservation'}
 $report=[ordered]@{stage='M15.1b.2e.10b.4';verified_at_utc=[DateTimeOffset]::UtcNow.ToString('o');source_read_only=$true;source_export_unchanged=$true;source_items=8;source_storage=5;source_columns=@(4,5);export_sha256=Get-TaskHash $export.Json;query_sha256=$export.QuerySha256;schema_sha256=(Get-FileHash (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json')).Hash.ToLowerInvariant();private_acl='current Windows user and SYSTEM only';imported=$imported;replay=$replay;verified=$verified;post_restart_verified=$false;character_creation_available=$false}
}else{
 $report=Get-Content $taskEvidence -Raw|ConvertFrom-Json -DateKind String
 $json=[IO.File]::ReadAllText($taskFile);if((Get-TaskHash $json)-ne$report.export_sha256){throw 'Private shared export digest changed'}
 $verified=Invoke-TaskImport $json $true
 $report.post_restart_verified=$true
 $report|Add-Member -Force -NotePropertyName post_restart_verified_at_utc -NotePropertyValue ([DateTimeOffset]::UtcNow.ToString('o'))
 $report|Add-Member -Force -NotePropertyName post_restart_summary -NotePropertyValue $verified
}
$report|ConvertTo-Json -Depth 7|Set-Content $taskEvidence -Encoding utf8NoBOM
Write-Host ('Shared items '+$Mode+' PASS:8/5 records;4/5 fields;identity replay never reseeds;reference unchanged;character creation=false.')
