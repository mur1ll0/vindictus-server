param([ValidateSet('Import','Verify')][string]$Mode='Import')
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskPrivateDir=Join-Path $taskProject '.cache/private-character-import'
$taskPrivateFile=Join-Path $taskPrivateDir 'bundle.json'
$taskEvidenceFile=Join-Path $taskProject 'docs/evidence/database/character-import.json'
function Get-TaskHash([string]$Value){[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($Value))).ToLowerInvariant()}
function Get-TaskExport {
 $taskQuery=[IO.File]::ReadAllText((Join-Path $taskProject 'scripts/database/export-character-login.sql'))
 $taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
 $taskResult=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'export',$taskQuery)
 if($taskResult.ExitCode -ne 0){throw 'Source export failed; details suppressed'}
 $taskFirst=$taskResult.Stdout.IndexOf('{');$taskLast=$taskResult.Stdout.LastIndexOf('}')
 if($taskFirst -lt 0 -or $taskLast -lt $taskFirst){throw 'Source export JSON missing'}
 $taskJson=$taskResult.Stdout.Substring($taskFirst,$taskLast-$taskFirst+1).Replace([string][char]13,'').Replace([string][char]10,'')
 if([Text.Encoding]::UTF8.GetByteCount($taskJson) -gt 6MB){throw 'Source export too large'}
 try{$taskData=$taskJson|ConvertFrom-Json -DateKind String}catch{throw 'Source export invalid; details suppressed'}
 foreach($taskName in 'characters','character_info','connections','queued_items'){
  $taskExpected=if($taskName -eq 'queued_items'){140}else{49}
  if($taskData.source_counts.$taskName -ne $taskExpected -or @($taskData.$taskName).Count -ne $taskExpected){throw 'Recovered source count changed; batch refused'}
 }
 $taskJson
}
function Invoke-TaskImport([string]$Json,[bool]$VerifyOnly){
 $taskArgs=@('compose','--project-directory',$taskProject,'exec','-T','server','/app/vindictus-server','import-characters','--input-stdin')
 if($VerifyOnly){$taskArgs+='--verify-only'}
 $taskResult=Invoke-ProjectDocker -Arguments $taskArgs -InputText $Json
 if($taskResult.ExitCode -ne 0){throw 'Character import/verification failed; values suppressed'}
 try{$taskResult.Stdout.Trim()|ConvertFrom-Json -DateKind String}catch{throw 'Invalid import summary; details suppressed'}
}
function Get-TaskCounts {
 $taskShell='PGPASSWORD="$APP_DATABASE_PASSWORD" psql -h localhost -U "$APP_DATABASE_USER" -d "$POSTGRES_DB" -tA -c "$1"'
 $taskQuery="SELECT json_build_object('characters',(SELECT COUNT(*) FROM game_characters),'character_info',(SELECT COUNT(*) FROM game_character_info),'connections',(SELECT COUNT(*) FROM game_last_connect_info),'queued_items',(SELECT COUNT(*) FROM game_queued_items));"
 $taskResult=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'exec','-T','postgres','bash','-c',$taskShell,'counts',$taskQuery)
 if($taskResult.ExitCode -ne 0){throw 'Target count failed; details suppressed'}
 try{$taskResult.Stdout.Trim()|ConvertFrom-Json}catch{throw 'Invalid target counts'}
}
function Assert-TaskSummary($Result,[bool]$RequireNoInsert){
 if($Result.game_authentication_available -ne $false){throw 'Unsupported authentication capability'}
 foreach($taskName in 'characters','character_info','connections','queued_items'){
  $taskExpected=if($taskName -eq 'queued_items'){140}else{49}
  if($Result.summary.source_rows.$taskName -ne $taskExpected -or $Result.summary.verified.$taskName -ne $taskExpected){throw 'Import result count mismatch'}
  if($RequireNoInsert -and $Result.summary.inserted.$taskName -ne 0){throw 'Verification/replay unexpectedly inserted rows'}
 }
}
if($Mode -eq 'Import'){
 $taskJson=Get-TaskExport
 if(Test-Path -LiteralPath $taskPrivateFile){
  $taskExisting=[IO.File]::ReadAllText($taskPrivateFile)
  if((Get-TaskHash $taskExisting) -ne (Get-TaskHash $taskJson)){throw 'Preserved private export differs; reconcile explicitly before importing another source snapshot'}
 }
 New-Item -ItemType Directory -Force -Path $taskPrivateDir|Out-Null
 $taskAcl=[Security.AccessControl.DirectorySecurity]::new()
 $taskAcl.SetAccessRuleProtection($true,$false)
 foreach($taskSid in @([Security.Principal.WindowsIdentity]::GetCurrent().User,[Security.Principal.SecurityIdentifier]::new('S-1-5-18'))){
  $taskRule=[Security.AccessControl.FileSystemAccessRule]::new($taskSid,'FullControl','ContainerInherit,ObjectInherit','None','Allow');$taskAcl.AddAccessRule($taskRule)
 }
 Set-Acl -LiteralPath $taskPrivateDir -AclObject $taskAcl
 [IO.File]::WriteAllText($taskPrivateFile,$taskJson,[Text.UTF8Encoding]::new($false))
 $taskBefore=Get-TaskCounts
 $taskImported=Invoke-TaskImport $taskJson $false;Assert-TaskSummary $taskImported $false
 $taskAfter=Get-TaskCounts
 foreach($taskName in 'characters','character_info','connections','queued_items'){if(($taskAfter.$taskName-$taskBefore.$taskName) -ne $taskImported.summary.inserted.$taskName){throw 'Target delta mismatch'}}
 $taskReplay=Invoke-TaskImport $taskJson $false;Assert-TaskSummary $taskReplay $true
 $taskVerify=Invoke-TaskImport $taskJson $true;Assert-TaskSummary $taskVerify $true
 if((Get-TaskHash (Get-TaskExport)) -ne (Get-TaskHash $taskJson)){throw 'Source changed during import'}
 $taskReport=[ordered]@{
  stage='M13.2d.2b.1';exported_at_utc=[DateTimeOffset]::UtcNow.ToString('o');source='study_heroes.character-login-bundle';
  source_read_only=$true;source_export_unchanged=$true;source_columns=64;source_rows=287;
  export_sha256=Get-TaskHash $taskJson;export_query_sha256=(Get-FileHash (Join-Path $taskProject 'scripts/database/export-character-login.sql')).Hash.ToLowerInvariant();
  source_schema_sha256=(Get-FileHash (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json')).Hash.ToLowerInvariant();
  private_export_acl='current Windows user and SYSTEM; inheritance disabled';
  target_total_before=$taskBefore;target_total_after=$taskAfter;target_delta_verified=$true;
  imported=$taskImported.summary;replay=$taskReplay.summary;verified=$taskVerify.summary;
  comparison='Rust equality of every typed source field, one transaction, recovered FK chain, no UPDATE';
  game_authentication_available=$false;runtime_rewards_implemented=$false;queue_identity_allocator_implemented=$false;post_restart_verified=$false;
 }
 $taskReport|ConvertTo-Json -Depth 7|Set-Content $taskEvidenceFile -Encoding utf8NoBOM
 Write-Host 'Character bundle import PASS: source=49/49/49/140; fields=64; rows=287; replay inserted=0; source unchanged=true'
}else{
 $taskJson=[IO.File]::ReadAllText($taskPrivateFile);$taskReport=Get-Content $taskEvidenceFile -Raw|ConvertFrom-Json -DateKind String
 if((Get-TaskHash $taskJson) -ne $taskReport.export_sha256){throw 'Private export digest changed'}
 $taskVerify=Invoke-TaskImport $taskJson $true;Assert-TaskSummary $taskVerify $true
 if((Get-TaskHash (Get-TaskExport)) -ne $taskReport.export_sha256){throw 'Read-only reexport differs from preserved source'}
 $taskReport.post_restart_verified=$true
 $taskReport|Add-Member -Force -NotePropertyName post_restart_verified_at_utc -NotePropertyValue ([DateTimeOffset]::UtcNow.ToString('o'))
 $taskReport|Add-Member -Force -NotePropertyName post_restart_summary -NotePropertyValue $taskVerify.summary
 $taskReport|ConvertTo-Json -Depth 7|Set-Content $taskEvidenceFile -Encoding utf8NoBOM
 Write-Host 'Character bundle verify PASS: matched=287; fields=64; inserted=0; source unchanged=true'
}
