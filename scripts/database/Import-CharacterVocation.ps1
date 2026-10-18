param([ValidateSet('Import','Verify')][string]$Mode='Import')
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskPrivate=Join-Path $taskProject '.cache/private-vocation'
$taskFile=Join-Path $taskPrivate 'vocation.json'
$taskEvidence=Join-Path $taskProject 'docs/evidence/database/vocation-import.json'
function Get-TaskHash([string]$Text){[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($Text))).ToLowerInvariant()}
function Get-TaskExport {
 $taskQuery=@"
SET NOCOUNT ON;
SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;
BEGIN TRANSACTION;
SELECT 1 AS format_version,'study_heroes.vocation-state' AS source,
(SELECT CONVERT(varchar(20),last_value) FROM sys.identity_columns WHERE object_id=OBJECT_ID('dbo.VocationSkill') AND name='RowID' AND CONVERT(bigint,seed_value)=1 AND CONVERT(bigint,increment_value)=1) AS source_last_value,
JSON_QUERY((SELECT CONVERT(varchar(20),CID) AS cid,VocationClass AS vocation_class,VocationLevel AS vocation_level,VocationExp AS vocation_exp,
CONVERT(varchar(20),DATEDIFF_BIG(DAY,CONVERT(datetime2,'0001-01-01'),CONVERT(date,LastTransform))*CONVERT(bigint,864000000000)+DATEDIFF_BIG(NANOSECOND,CONVERT(datetime2,CONVERT(date,LastTransform)),LastTransform)/100) AS last_transform_ticks FROM dbo.Vocation ORDER BY CID FOR JSON PATH,INCLUDE_NULL_VALUES)) AS vocations,
JSON_QUERY((SELECT CONVERT(varchar(20),RowID) AS row_id,CONVERT(varchar(20),CID) AS cid,CONVERT(varchar(max),CONVERT(varbinary(max),SkillID),2) AS skill_utf16,Rank AS rank FROM dbo.VocationSkill ORDER BY RowID FOR JSON PATH,INCLUDE_NULL_VALUES)) AS skills
FOR JSON PATH,INCLUDE_NULL_VALUES,WITHOUT_ARRAY_WRAPPER;
COMMIT;
"@
 $sh='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
 $r=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$sh,'readonly-vocation-export',$taskQuery)
 if($r.ExitCode-ne0){throw 'Read-only vocation export failed;private details suppressed'}
 $a=$r.Stdout.IndexOf('{');$b=$r.Stdout.LastIndexOf('}');if($a-lt0-or$b-lt$a){throw 'Missing vocation export'}
 $json=$r.Stdout.Substring($a,$b-$a+1).Replace([string][char]13,'').Replace([string][char]10,'')
 $data=$json|ConvertFrom-Json -DateKind String
 if(@($data.vocations).Count-ne3-or@($data.skills).Count-ne33-or$data.source_last_value-ne'51'){throw 'Vocation source profile changed;inspect privately'}
 return [pscustomobject]@{Json=$json;QuerySha256=Get-TaskHash $taskQuery}
}
function Invoke-TaskImport([string]$Json,[bool]$Verify){
 $taskArgs=@('compose','--project-directory',$taskProject,'exec','-T','server','/app/vindictus-server','import-character-vocation','--input-stdin');if($Verify){$taskArgs+='--verify-only'}
 $r=Invoke-ProjectDocker -Arguments $taskArgs -InputText $Json
 if($r.ExitCode-ne0){throw 'Vocation preservation failed;private values suppressed'}
 try{$summary=$r.Stdout.Trim()|ConvertFrom-Json -DateKind String}catch{throw 'Invalid sanitized vocation summary'}
 if($summary.vocations-ne3-or$summary.skills-ne33-or$summary.verified-ne36-or-not$summary.identity_initialized-or$summary.character_creation_available){throw 'Vocation summary mismatch'}
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
 $replay=Invoke-TaskImport $export.Json $false;if($replay.inserted-ne0){throw 'Vocation replay inserted rows'}
 $verified=Invoke-TaskImport $export.Json $true
 $after=Get-TaskExport;if((Get-TaskHash $after.Json)-ne(Get-TaskHash $export.Json)){throw 'Vocation reference changed during read-only preservation'}
 $report=[ordered]@{stage='M15.1b.2e.10b.16';verified_at_utc=[DateTimeOffset]::UtcNow.ToString('o');source_read_only=$true;source_export_unchanged=$true;source_vocations=3;source_skills=33;source_columns=@(5,4);export_sha256=Get-TaskHash $export.Json;query_sha256=$export.QuerySha256;schema_sha256=(Get-FileHash (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json')).Hash.ToLowerInvariant();private_acl='current Windows user and SYSTEM only';imported=$imported;replay=$replay;verified=$verified;post_restart_verified=$false;character_creation_available=$false}
}else{
 $report=Get-Content $taskEvidence -Raw|ConvertFrom-Json -DateKind String
 $json=[IO.File]::ReadAllText($taskFile);if((Get-TaskHash $json)-ne$report.export_sha256){throw 'Private vocation export digest changed'}
 $verified=Invoke-TaskImport $json $true
 $report.post_restart_verified=$true
 $report|Add-Member -Force -NotePropertyName post_restart_verified_at_utc -NotePropertyValue ([DateTimeOffset]::UtcNow.ToString('o'))
 $report|Add-Member -Force -NotePropertyName post_restart_summary -NotePropertyValue $verified
}
$report|ConvertTo-Json -Depth 7|Set-Content $taskEvidence -Encoding utf8NoBOM
Write-Host ('Vocation/VocationSkill '+$Mode+' PASS:3/33 rows;5/4 fields;full typed comparison;identity replay never reseeds;source unchanged;character creation=false.')
