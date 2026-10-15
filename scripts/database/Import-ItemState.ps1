param([ValidateSet('Import','Verify')][string]$Mode='Import')
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskPrivate=Join-Path $taskProject '.cache/private-item-state'
$taskFile=Join-Path $taskPrivate 'item-state.json'
$taskEvidence=Join-Path $taskProject 'docs/evidence/database/item-state-import.json'
function Get-TaskHash([string]$Text){[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($Text))).ToLowerInvariant()}
function Get-TaskExport {
 $taskQuery=@"
SET NOCOUNT ON;
SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;
BEGIN TRANSACTION;
SELECT 1 AS format_version,'study_heroes.item-state' AS source,
(SELECT CONVERT(varchar(20),last_value) FROM sys.identity_columns WHERE object_id=OBJECT_ID('dbo.ItemAttribute') AND name='ID' AND CONVERT(bigint,seed_value)=1 AND CONVERT(bigint,increment_value)=1) AS attribute_last_value,
(SELECT CONVERT(varchar(20),last_value) FROM sys.identity_columns WHERE object_id=OBJECT_ID('dbo.ItemStorageStatus') AND name='ID' AND CONVERT(bigint,seed_value)=1 AND CONVERT(bigint,increment_value)=1) AS storage_last_value,
JSON_QUERY((SELECT CONVERT(varchar(20),ID) AS id,CONVERT(varchar(200),CONVERT(varbinary(100),ItemClass),2) AS item_class_utf16,CONVERT(varchar(20),OwnerID) AS owner_id,[Count] AS count,Collection AS collection,Slot AS slot,
 CONVERT(varchar(20),DATEDIFF_BIG(day,CONVERT(date,'00010101'),CONVERT(date,ExpireDateTime))*864000000000+DATEDIFF_BIG(nanosecond,CONVERT(datetime2(7),CONVERT(date,ExpireDateTime)),ExpireDateTime)/100) AS expire_ticks,
 Tradable AS tradable,CheckSum AS checksum FROM dbo.Item ORDER BY ID FOR JSON PATH,INCLUDE_NULL_VALUES)) AS items,
JSON_QUERY((SELECT CONVERT(varchar(20),ID) AS id,CONVERT(varchar(20),ItemID) AS item_id,CONVERT(varchar(100),CONVERT(varbinary(50),Attribute),2) AS attribute_bytes,CONVERT(varchar(100),CONVERT(varbinary(50),Value),2) AS value_bytes,Arg AS arg,Arg2 AS arg2 FROM dbo.ItemAttribute ORDER BY ID FOR JSON PATH,INCLUDE_NULL_VALUES)) AS attributes,
JSON_QUERY((SELECT CONVERT(varchar(20),ID) AS id,Part AS part,Color1 AS color1,Color2 AS color2,Color3 AS color3,ReducedDurability AS reduced_durability,MaxDurabilityBonus AS max_durability_bonus FROM dbo.Equippable ORDER BY ID FOR JSON PATH,INCLUDE_NULL_VALUES)) AS equipment,
JSON_QUERY((SELECT CONVERT(varchar(20),ID) AS id,CONVERT(varchar(20),OwnerID) AS owner_id,StorageType AS storage_type,Storage AS storage,
 CONVERT(varchar(20),CONVERT(bigint,CONVERT(int,SUBSTRING(CONVERT(binary(8),ExpireTime),1,4)))*25920000+CONVERT(int,SUBSTRING(CONVERT(binary(8),ExpireTime),5,4))) AS expire_units300,
 CONVERT(varchar(200),CONVERT(varbinary(100),StorageName),2) AS storage_name_utf16,StorageTag AS storage_tag FROM dbo.ItemStorageStatus ORDER BY ID FOR JSON PATH,INCLUDE_NULL_VALUES)) AS storage
FOR JSON PATH,INCLUDE_NULL_VALUES,WITHOUT_ARRAY_WRAPPER;
COMMIT TRANSACTION;
"@
 $taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
 $r=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'item-state-readonly-export',$taskQuery)
 if($r.ExitCode-ne0){throw 'Read-only ItemState export failed; private details suppressed'}
 $a=$r.Stdout.IndexOf('{');$b=$r.Stdout.LastIndexOf('}');if($a-lt0-or$b-lt$a){throw 'Missing ItemState export'}
 $json=$r.Stdout.Substring($a,$b-$a+1).Replace([string][char]13,'').Replace([string][char]10,'')
 $data=$json|ConvertFrom-Json -DateKind String
 if((@($data.items).Count,@($data.attributes).Count,@($data.equipment).Count,@($data.storage).Count -join ',')-ne'2336,209,1032,19'-or$null-eq$data.attribute_last_value-or$null-eq$data.storage_last_value){throw 'ItemState source profile changed; inspect privately'}
 return [pscustomobject]@{Json=$json;QuerySha256=Get-TaskHash $taskQuery}
}
function Invoke-TaskImport([string]$Json,[bool]$Verify){
 $taskArgs=@('compose','--project-directory',$taskProject,'exec','-T','server','/app/vindictus-server','import-item-state','--input-stdin');if($Verify){$taskArgs+='--verify-only'}
 $r=Invoke-ProjectDocker -Arguments $taskArgs -InputText $Json
 if($r.ExitCode-ne0){throw 'ItemState preservation failed; private values suppressed'}
 try{$summary=$r.Stdout.Trim()|ConvertFrom-Json -DateKind String}catch{throw 'Invalid sanitized ItemState summary'}
 if(($summary.source_rows -join ',')-ne'2336,209,1032,19'-or($summary.verified -join ',')-ne'2336,209,1032,19'-or-not$summary.identity_initialized-or$summary.character_creation_available){throw 'ItemState summary mismatch'}
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
 $replay=Invoke-TaskImport $export.Json $false;if(($replay.inserted -join ',')-ne'0,0,0,0'){throw 'ItemState replay inserted rows'}
 $verified=Invoke-TaskImport $export.Json $true
 $after=Get-TaskExport;if((Get-TaskHash $after.Json)-ne(Get-TaskHash $export.Json)){throw 'ItemState reference changed during read-only preservation'}
 $report=[ordered]@{stage='M15.1b.2e.3';verified_at_utc=[DateTimeOffset]::UtcNow.ToString('o');source_read_only=$true;source_export_unchanged=$true;source_rows=@(2336,209,1032,19);source_columns=@(9,6,7,7);export_sha256=Get-TaskHash $export.Json;query_sha256=$export.QuerySha256;schema_sha256=(Get-FileHash (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json')).Hash.ToLowerInvariant();private_acl='current Windows user and SYSTEM only';imported=$imported;replay=$replay;verified=$verified;post_restart_verified=$false;character_creation_available=$false}
 if(Test-Path -LiteralPath $taskEvidence){
  $prior=Get-Content $taskEvidence -Raw|ConvertFrom-Json -DateKind String
  if($prior.export_sha256-eq$report.export_sha256){$report.initial_import_summary=$prior.initial_import_summary;$report.initial_import_verified_at_utc=$prior.initial_import_verified_at_utc}
 }
 if(-not$report.Contains('initial_import_summary')){$report.initial_import_summary=$imported;$report.initial_import_verified_at_utc=$report.verified_at_utc}
}else{
 $report=Get-Content $taskEvidence -Raw|ConvertFrom-Json -DateKind String
 $json=[IO.File]::ReadAllText($taskFile);if((Get-TaskHash $json)-ne$report.export_sha256){throw 'Private ItemState export digest changed'}
 $verified=Invoke-TaskImport $json $true
 $report.post_restart_verified=$true
 $report|Add-Member -Force -NotePropertyName post_restart_verified_at_utc -NotePropertyValue ([DateTimeOffset]::UtcNow.ToString('o'))
 $report|Add-Member -Force -NotePropertyName post_restart_summary -NotePropertyValue $verified
}
$report|ConvertTo-Json -Depth 7|Set-Content $taskEvidence -Encoding utf8NoBOM
Write-Host ('ItemState '+$Mode+' PASS:2336/209/1032/19 rows;9/6/7/7 fields;full typed comparison;identity replay never reseeds;source unchanged;character creation=false.')
