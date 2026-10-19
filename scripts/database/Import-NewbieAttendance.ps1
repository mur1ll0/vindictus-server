param([ValidateSet('Import','Verify')][string]$Mode='Import',[string]$RequireEmptyOwner='')
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskPrivate=Join-Path $taskProject '.cache/private-newbie-attendance'
$taskFile=Join-Path $taskPrivate 'newbie-attendance.json'
$taskEvidence=Join-Path $taskProject 'docs/evidence/database/newbie-attendance-import.json'
function Get-TaskHash([string]$Text){[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($Text))).ToLowerInvariant()}
function Get-TaskExport {
 $taskQuery=@"
SET NOCOUNT ON;
SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;
BEGIN TRANSACTION;
SELECT 1 AS format_version,'study_heroes.newbie-attendance' AS source,
JSON_QUERY((SELECT UID AS uid,EventVersion AS event_version,
CONVERT(varchar(20),DATEDIFF_BIG(SECOND,CONVERT(datetime2(7),'0001-01-01T00:00:00'),StartTime)*10000000+DATEPART(NANOSECOND,StartTime)/100) AS start_ticks,
CONVERT(varchar(20),DATEDIFF_BIG(SECOND,CONVERT(datetime2(7),'0001-01-01T00:00:00'),EndTime)*10000000+DATEPART(NANOSECOND,EndTime)/100) AS end_ticks
FROM dbo.AttendanceNewbieInfo ORDER BY UID FOR JSON PATH,INCLUDE_NULL_VALUES)) AS rows
FOR JSON PATH,INCLUDE_NULL_VALUES,WITHOUT_ARRAY_WRAPPER;
COMMIT;
"@
 $sh='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
 $r=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$sh,'readonly-newbie-attendance-export',$taskQuery)
 if($r.ExitCode-ne0){throw 'Read-only newbie-attendance export failed;private details suppressed'}
 $a=$r.Stdout.IndexOf('{');$b=$r.Stdout.LastIndexOf('}');if($a-lt0-or$b-lt$a){throw 'Missing newbie-attendance export'}
 $json=$r.Stdout.Substring($a,$b-$a+1).Replace([string][char]13,'').Replace([string][char]10,'')
 $data=$json|ConvertFrom-Json -DateKind String
 if(@($data.rows).Count-ne1){throw 'AttendanceNewbieInfo source profile changed;inspect privately'}
 return [pscustomobject]@{Json=$json;QuerySha256=Get-TaskHash $taskQuery}
}
function Invoke-TaskImport([string]$Json,[bool]$Verify){
 $taskArgs=@('compose','--project-directory',$taskProject,'exec','-T','server','/app/vindictus-server','import-newbie-attendance','--input-stdin');if($RequireEmptyOwner){$taskArgs+=@('--require-empty-owner',$RequireEmptyOwner)};if($Verify){$taskArgs+='--verify-only'}
 $r=Invoke-ProjectDocker -Arguments $taskArgs -InputText $Json
 if($r.ExitCode-ne0){throw 'AttendanceNewbieInfo preservation failed;private values suppressed'}
 try{$summary=$r.Stdout.Trim()|ConvertFrom-Json -DateKind String}catch{throw 'Invalid sanitized newbie-attendance summary'}
 if($summary.summary.rows-ne1-or$summary.summary.verified-ne1-or$summary.summary.fields-ne4-or-not$summary.summary.initialized-or($RequireEmptyOwner-and-not$summary.empty_owner_verified)-or$summary.summary.character_creation_available){throw 'Newbie attendance summary mismatch'}
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
 $replay=Invoke-TaskImport $export.Json $false;if($replay.summary.inserted-ne0){throw 'AttendanceNewbieInfo replay inserted rows'}
 $verified=Invoke-TaskImport $export.Json $true
 $after=Get-TaskExport;if((Get-TaskHash $after.Json)-ne(Get-TaskHash $export.Json)){throw 'AttendanceNewbieInfo reference changed during read-only preservation'}
 $report=[ordered]@{stage='M15.1b.2e.10b.22c';verified_at_utc=[DateTimeOffset]::UtcNow.ToString('o');source_read_only=$true;source_export_unchanged=$true;source_rows=1;source_columns=4;source_identity=$false;source_foreign_keys=0;empty_owner_verified=[bool]$RequireEmptyOwner;export_sha256=Get-TaskHash $export.Json;query_sha256=$export.QuerySha256;schema_sha256=(Get-FileHash (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json')).Hash.ToLowerInvariant();private_acl='current Windows user and SYSTEM only';imported=$imported;replay=$replay;verified=$verified;post_restart_verified=$false;character_creation_available=$false}
}else{
 $report=Get-Content $taskEvidence -Raw|ConvertFrom-Json -DateKind String
 $json=[IO.File]::ReadAllText($taskFile);if((Get-TaskHash $json)-ne$report.export_sha256){throw 'Private newbie-attendance export digest changed'}
 $verified=Invoke-TaskImport $json $true
 $report.post_restart_verified=$true
 $report|Add-Member -Force -NotePropertyName post_restart_verified_at_utc -NotePropertyValue ([DateTimeOffset]::UtcNow.ToString('o'))
 $report|Add-Member -Force -NotePropertyName post_restart_summary -NotePropertyValue $verified
 $report|Add-Member -Force -NotePropertyName post_restart_candidate_absence_requested -NotePropertyValue ([bool]$RequireEmptyOwner)
}
$report|ConvertTo-Json -Depth 7|Set-Content $taskEvidence -Encoding utf8NoBOM
Write-Host ('Newbie attendance '+$Mode+' PASS:source1 row/4 fields;datetime2(7);candidate_absence_requested='+[bool]$RequireEmptyOwner+';source unchanged;character creation=false.')
