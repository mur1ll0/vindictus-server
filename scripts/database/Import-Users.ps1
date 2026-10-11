param(
    [ValidateSet('Import','Verify')][string]$Mode = 'Import',
    [int]$ExpectedRows = 29
)
$ErrorActionPreference = 'Stop'
$taskProject = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskPrivateDir = Join-Path $taskProject '.cache/private-user-import'
$taskPrivateFile = Join-Path $taskPrivateDir 'users.json'
$taskEvidenceFile = Join-Path $taskProject 'docs/evidence/database/user-import.json'
function Assert-TaskDocker($Result) {
    if ($Result.ExitCode -ne 0) {
        # Do not echo SQL/client errors: source rows and secret fields could appear.
        throw 'User migration command failed. Source values and detailed errors are suppressed.'
    }
}
function Get-TaskTargetCount {
    $taskShell='PGPASSWORD="$APP_DATABASE_PASSWORD" psql --username "$APP_DATABASE_USER" --dbname "$POSTGRES_DB" --no-psqlrc --quiet --tuples-only --no-align --set ON_ERROR_STOP=1 --command "SELECT COUNT(*) FROM game_users"'
    $taskResult=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'exec','-T','postgres','bash','-c',$taskShell)
    Assert-TaskDocker $taskResult
    if($taskResult.Stdout.Trim() -notmatch '^[0-9]+$'){throw 'Invalid target count.'}
    [long]$taskResult.Stdout.Trim()
}
function Get-TaskExport {
    $taskQuery = @"
SET NOCOUNT ON;
SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;
BEGIN TRANSACTION;
DECLARE @count int=(SELECT COUNT(*) FROM dbo.[User]);
DECLARE @export nvarchar(max)=(
SELECT 1 AS format_version,'study_heroes.dbo.User' AS source,@count AS exported_row_count,
JSON_QUERY((
SELECT ID AS id,Name AS name,NextCharacterSN AS next_character_sn,ServiceID AS service_id,
ChannelingCode AS channeling_code,
CONVERT(varchar(max),CONVERT(varbinary(max),LastLoginIP),2) AS last_login_ip_utf16_hex,
CONVERT(varchar(20),DATEDIFF_BIG(day,CONVERT(date,'00010101'),CONVERT(date,IPLockTime))*864000000000+
 DATEDIFF_BIG(nanosecond,CONVERT(datetime2(7),CONVERT(date,IPLockTime)),IPLockTime)/100) AS ip_lock_time_ticks,
CONVERT(varchar(max),CONVERT(varbinary(max),FacebookToken),2) AS facebook_token_hex,
CONVERT(varchar(20),CONVERT(bigint,CONVERT(int,SUBSTRING(CONVERT(binary(8),IsDormantUserEvent),1,4)))*25920000+
 CONVERT(int,SUBSTRING(CONVERT(binary(8),IsDormantUserEvent),5,4))) AS is_dormant_user_event_units300,
CONVERT(varchar(max),CONVERT(varbinary(max),SecondPassword),2) AS second_password_utf16_hex,
SecondPasswordFailCount AS second_password_fail_count,
CONVERT(varchar(20),DATEDIFF_BIG(day,CONVERT(date,'00010101'),CONVERT(date,SecondPasswordLastFail))*864000000000+
 DATEDIFF_BIG(nanosecond,CONVERT(datetime2(7),CONVERT(date,SecondPasswordLastFail)),SecondPasswordLastFail)/100) AS second_password_last_fail_ticks,
CONVERT(varchar(16),CONVERT(binary(8),LastUpdate),2) AS last_update_hex,
CONVERT(varchar(20),CONVERT(bigint,CONVERT(int,SUBSTRING(CONVERT(binary(8),IsDormantUser),1,4)))*25920000+
 CONVERT(int,SUBSTRING(CONVERT(binary(8),IsDormantUser),5,4))) AS is_dormant_user_units300,
CONVERT(varchar(20),CONVERT(bigint,CONVERT(int,SUBSTRING(CONVERT(binary(8),CreateTime),1,4)))*25920000+
 CONVERT(int,SUBSTRING(CONVERT(binary(8),CreateTime),5,4))) AS create_time_units300
FROM dbo.[User] ORDER BY ID
FOR JSON PATH,INCLUDE_NULL_VALUES
)) AS rows FOR JSON PATH,WITHOUT_ARRAY_WRAPPER);
SELECT @export;
COMMIT TRANSACTION;
"@
    $taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
    $taskResult=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'export',$taskQuery)
    Assert-TaskDocker $taskResult
    $taskFirst=$taskResult.Stdout.IndexOf('{'); $taskLast=$taskResult.Stdout.LastIndexOf('}')
    if($taskFirst -lt 0 -or $taskLast -lt $taskFirst){throw 'Source did not return an export document.'}
    # sqlcmd can wrap a wide nvarchar result; JSON string control characters are escaped.
    $taskJson=$taskResult.Stdout.Substring($taskFirst,$taskLast-$taskFirst+1).Replace([string][char]13,'').Replace([string][char]10,'')
    [pscustomobject]@{Json=$taskJson;QuerySha256=([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($taskQuery))).ToLowerInvariant())}
}
function Get-TaskHash([string]$Value) {
    [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($Value))).ToLowerInvariant()
}
function Invoke-TaskImport([string]$Json,[bool]$VerifyOnly) {
    $taskArgs=@('compose','--project-directory',$taskProject,'exec','-T','server','/app/vindictus-server','import-users','--input-stdin')
    if($VerifyOnly){$taskArgs+='--verify-only'}
    $taskResult=Invoke-ProjectDocker -Arguments $taskArgs -InputText $Json
    Assert-TaskDocker $taskResult
    try { $taskSummary=$taskResult.Stdout.Trim()|ConvertFrom-Json -DateKind String }
    catch { throw 'Invalid import summary; detailed output suppressed.' }
    if($taskSummary.source_rows -ne $ExpectedRows -or $taskSummary.verified -ne $ExpectedRows -or $taskSummary.game_authentication_available -ne $false){throw 'Import count or capability mismatch.'}
    $taskSummary
}
if($Mode -eq 'Import'){
    $taskExport=Get-TaskExport
    try { $taskData=$taskExport.Json|ConvertFrom-Json -DateKind String }
    catch { throw 'Invalid source export JSON; detailed errors suppressed.' }
    if($taskData.exported_row_count -ne $ExpectedRows -or @($taskData.rows).Count -ne $ExpectedRows){throw 'Source row count changed; batch refused.'}
    New-Item -ItemType Directory -Path $taskPrivateDir -Force|Out-Null
    # Protect the export before writing it; never publish or link the private file.
    $taskAcl=[Security.AccessControl.DirectorySecurity]::new()
    $taskAcl.SetAccessRuleProtection($true,$false)
    foreach($taskSid in @([Security.Principal.WindowsIdentity]::GetCurrent().User,[Security.Principal.SecurityIdentifier]::new('S-1-5-18'))){
        $taskRule=[Security.AccessControl.FileSystemAccessRule]::new($taskSid,'FullControl','ContainerInherit,ObjectInherit','None','Allow')
        $taskAcl.AddAccessRule($taskRule)
    }
    Set-Acl -LiteralPath $taskPrivateDir -AclObject $taskAcl
    [IO.File]::WriteAllText($taskPrivateFile,$taskExport.Json,[Text.UTF8Encoding]::new($false))
    $taskCountBefore=Get-TaskTargetCount
    $taskImported=Invoke-TaskImport $taskExport.Json $false
    $taskCountAfter=Get-TaskTargetCount
    if(($taskCountAfter-$taskCountBefore) -ne $taskImported.inserted){throw 'Target count delta differs from inserted rows.'}
    $taskReplay=Invoke-TaskImport $taskExport.Json $false
    if($taskReplay.inserted -ne 0 -or $taskReplay.existing -ne $ExpectedRows){throw 'Replay was not idempotent.'}
    $taskVerify=Invoke-TaskImport $taskExport.Json $true
    # Re-read the source to detect any drift during migration; no source writes.
    $taskAfter=Get-TaskExport
    if((Get-TaskHash $taskAfter.Json) -ne (Get-TaskHash $taskExport.Json)){throw 'Source changed during import; inspect the batch before continuing.'}
    $taskReport=[ordered]@{
        stage='M14.3b.2'; exported_at_utc=[DateTimeOffset]::UtcNow.ToString('o')
        source='study_heroes.dbo.User'; source_read_only=$true; source_export_unchanged=$true
        export_sha256=(Get-TaskHash $taskExport.Json); export_query_sha256=$taskExport.QuerySha256
        schema_sha256=(Get-FileHash -LiteralPath (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json')).Hash.ToLowerInvariant()
        private_export_acl='current Windows user and SYSTEM; inheritance disabled on private directory'
        target_total_before=$taskCountBefore; target_total_after=$taskCountAfter; target_delta_verified=$true
        source_rows=$ExpectedRows; source_columns=15; imported=$taskImported; replay=$taskReplay; verified=$taskVerify
        comparison='Rust equality of all fifteen typed source fields inside transaction; no ID rehash or UPDATE'
        game_authentication_available=$false; post_restart_verified=$false
    }
    $taskReport|ConvertTo-Json -Depth 6|Set-Content -LiteralPath $taskEvidenceFile -Encoding utf8
    Write-Host ('User import: source='+$ExpectedRows+'; inserted='+$taskImported.inserted+'; existing='+$taskImported.existing+'; replay inserted=0; verified='+$taskVerify.verified+'; fields=15; source unchanged=true')
}else{
    $taskJson=[IO.File]::ReadAllText($taskPrivateFile)
    $taskReport=Get-Content -LiteralPath $taskEvidenceFile -Raw|ConvertFrom-Json -DateKind String
    if((Get-TaskHash $taskJson) -ne $taskReport.export_sha256){throw 'Private export digest changed.'}
    $taskSummary=Invoke-TaskImport $taskJson $true
    $taskReport.post_restart_verified=$true
    $taskReport|Add-Member -Force -NotePropertyName post_restart_verified_at_utc -NotePropertyValue ([DateTimeOffset]::UtcNow.ToString('o'))
    $taskReport|Add-Member -Force -NotePropertyName post_restart_summary -NotePropertyValue $taskSummary
    $taskReport|ConvertTo-Json -Depth 6|Set-Content -LiteralPath $taskEvidenceFile -Encoding utf8
    Write-Host ('User verify after restart: matched='+$taskSummary.verified+'; fields=15; inserted=0; export digest unchanged=true')
}
