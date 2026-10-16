[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'docker-process.ps1')
$taskProject = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$taskSettings = @{}
foreach ($taskLine in [IO.File]::ReadAllLines((Join-Path $taskProject '.env'))) {
    if ($taskLine -match '^([A-Z_]+)=(.*)$') { $taskSettings[$Matches[1]] = $Matches[2] }
}
$taskPort = if ($taskSettings.ContainsKey('HTTP_HOST_PORT')) { [int]$taskSettings.HTTP_HOST_PORT } else { 8080 }
$taskBaseUrl = 'http://127.0.0.1:' + $taskPort
function Assert-DockerCommand {
    param([string[]]$Arguments, [AllowNull()][string]$InputText = $null)
    $taskResult = Invoke-ProjectDocker -Arguments $Arguments -InputText $InputText
    if ($taskResult.ExitCode -ne 0) { throw ('Docker verification command failed: ' + $taskResult.Stderr) }
    return $taskResult
}
function Get-AppQuery {
    param([string]$Query)
    $taskShell = 'PGPASSWORD="$APP_DATABASE_PASSWORD" psql --username "$APP_DATABASE_USER" --dbname "$POSTGRES_DB" --no-psqlrc --quiet --tuples-only --no-align --set ON_ERROR_STOP=1 --command "$1"'
    (Assert-DockerCommand -Arguments @('compose','exec','-T','postgres','bash','-c',$taskShell,'check',$Query)).Stdout.Trim()
}
function Wait-Ready {
    param([int]$TimeoutSeconds = 45)
    $taskDeadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        try {
            $taskReady = Invoke-RestMethod ($taskBaseUrl + '/health/ready') -TimeoutSec 3
            if ($taskReady.status -eq 'ready') { return }
        } catch { }
        Start-Sleep -Seconds 1
    } while ([DateTime]::UtcNow -lt $taskDeadline)
    throw ('Readiness did not return ready within ' + $TimeoutSeconds + ' seconds.')
}
$taskUp = Assert-DockerCommand -Arguments @('compose','up','-d','--wait','server')
Wait-Ready
$taskCapabilities = Invoke-RestMethod ($taskBaseUrl + '/capabilities') -TimeoutSec 3
if ($taskCapabilities.game_client_compatible -or $taskCapabilities.legacy_migration_complete -or $taskCapabilities.game_authentication_available) { throw 'Foundation is incorrectly claiming game compatibility.' }
$taskUser = 'smoke_' + [Guid]::NewGuid().ToString('N').Substring(0,26)
$taskPassword = [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(24))
try {
    $taskCreated = Assert-DockerCommand -Arguments @('compose','exec','-T','server','/app/vindictus-server','create-account','--username',$taskUser,'--password-stdin') -InputText $taskPassword
    $taskDuplicate = Invoke-ProjectDocker -Arguments @('compose','exec','-T','server','/app/vindictus-server','create-account','--username',$taskUser.ToUpperInvariant(),'--password-stdin') -InputText $taskPassword
    if ($taskDuplicate.ExitCode -eq 0 -or $taskDuplicate.Stderr -notmatch 'account name already exists') { throw 'Case-insensitive duplicate account was not rejected.' }
    $taskBefore = Get-AppQuery -Query ("SELECT id FROM accounts WHERE normalized_name = '" + $taskUser + "';")
    if ($taskBefore -notmatch '^[0-9a-f-]{36}$') { throw 'Created account was not persisted.' }
    $taskGameId = -[Security.Cryptography.RandomNumberGenerator]::GetInt32(1, [int]::MaxValue)
    $taskGameName = 'Game_' + [Guid]::NewGuid().ToString('N').Substring(0,16)
    # Synthetic PostgreSQL row only: the Rust use case is tested in isolated schemas.
    $taskGameInsert = Get-AppQuery -Query (
        "INSERT INTO game_users(id,name,next_character_sn,service_id,channeling_code) VALUES(" +
        $taskGameId + ",'" + $taskGameName + "',0,42,255) ON CONFLICT DO NOTHING RETURNING id;"
    )
    if ($taskGameInsert -ne [string]$taskGameId) { throw 'Synthetic game-user INSERT did not create the expected row.' }
    $taskFullFields = Get-AppQuery -Query (
        "UPDATE game_users SET ip_lock_time_ticks=3155378975999999999,second_password_last_fail_ticks=1," +
        "is_dormant_user_event_units300=-1,is_dormant_user_units300=-1391644800000,create_time_units300=76683386879999," +
        "last_login_ip_utf16=decode('00D80000','hex'),second_password_utf16=decode(repeat('00D8',256),'hex')," +
        "second_password_fail_count=-1,facebook_token_bytes=decode('0080FF','hex')," +
        "legacy_last_update=decode('FFEEDDCCBBAA0099','hex') WHERE id=" + $taskGameId + ";"
    )
    $taskRevision = Get-AppQuery -Query ("SELECT local_revision FROM game_users WHERE id=" + $taskGameId + ";")
    if ($taskRevision -ne '1') { throw 'Revision trigger did not increment the synthetic UPDATE.' }
    $taskBanInsert = Get-AppQuery -Query (
        "INSERT INTO game_user_bans(name,status,expire_time_units300,reason_utf16,source_rowversion) VALUES('" +
        $taskGameName + "',2,NULL,decode('52006500610073006F006E00','hex'),decode('0102030405060708','hex'));"
    )
    $taskBanDigestQuery = "SELECT md5(row(name,name_key,status,expire_time_units300,reason_utf16,source_rowversion)::text) FROM game_user_bans WHERE name='" + $taskGameName + "';"
    $taskBanBefore = Get-AppQuery -Query $taskBanDigestQuery
    if ($taskBanBefore -notmatch '^[0-9a-f]{32}$') { throw 'Synthetic ban snapshot was not found.' }
    $taskGameDigestQuery = "SELECT md5(row(id,name,name_key,next_character_sn,service_id,channeling_code,persisted_at,last_login_ip_utf16,ip_lock_time_ticks,facebook_token_bytes,is_dormant_user_event_units300,second_password_utf16,second_password_fail_count,second_password_last_fail_ticks,legacy_last_update,is_dormant_user_units300,create_time_units300,local_revision)::text) FROM game_users WHERE id=" + $taskGameId + ";"
    $taskGameBefore = Get-AppQuery -Query $taskGameDigestQuery
    if ($taskGameBefore -notmatch '^[0-9a-f]{32}$') { throw 'Synthetic game-user snapshot was not found.' }
    $taskLimitedHex = [Convert]::ToHexString([Text.Encoding]::Unicode.GetBytes($taskGameName))
    $taskLimitedInsert = Get-AppQuery -Query ("INSERT INTO game_limited_accounts(nexon_id_utf16) VALUES(decode('" + $taskLimitedHex + "','hex'));")
    $taskLimitedDigestQuery = "SELECT md5(row(nexon_id_utf16,name_key)::text) FROM game_limited_accounts WHERE nexon_id_utf16=decode('" + $taskLimitedHex + "','hex');"
    $taskLimitedBefore = Get-AppQuery -Query $taskLimitedDigestQuery
    if ($taskLimitedBefore -notmatch '^[0-9a-f]{32}$') { throw 'Synthetic LimitedAccount snapshot was not found.' }
    $taskEffectOperation = [Guid]::NewGuid().ToString()
    $taskEffectInsert = Get-AppQuery -Query (
        "INSERT INTO game_friend_loyalty(recommender_uid,recommender_cid,friend_cid,recommend_flag,update_time_units300) VALUES(" +
        $taskGameId + ",-9223372036854775808,9223372036854775807,-2147483648,-1);" +
        "INSERT INTO game_user_achievements(id,uid,name_utf16,achievement_utf16) VALUES(" +
        $taskGameId + ",-1,decode('" + $taskLimitedHex + "','hex'),decode('00D80000','hex'));" +
        "INSERT INTO game_login_account_effect_runs(operation_id,uid,name_key,request,revision_after,is_dormant,is_return) VALUES('" +
        $taskEffectOperation + "'," + $taskGameId + ",'" + $taskGameName.ToLowerInvariant() + "','synthetic Docker persistence receipt',1,false,true);"
    )
    $taskEffectDigestQuery = "SELECT md5(row(f.recommender_uid,f.recommender_cid,f.friend_cid,f.recommend_flag,f.update_time_units300,a.id,a.uid,a.name_utf16,a.achievement_utf16,a.name_key,r.operation_id,r.uid,r.name_key,r.request,r.revision_after,r.is_dormant,r.is_return)::text) FROM game_friend_loyalty f JOIN game_user_achievements a ON a.id=f.recommender_uid JOIN game_login_account_effect_runs r ON r.uid=f.recommender_uid WHERE f.recommender_uid=" + $taskGameId + " AND r.operation_id='" + $taskEffectOperation + "';"
    $taskEffectBefore = Get-AppQuery -Query $taskEffectDigestQuery
    if ($taskEffectBefore -notmatch '^[0-9a-f]{32}$') { throw 'Synthetic account effects were not found.' }
    $taskPrivileges = Get-AppQuery -Query 'SELECT current_user, rolsuper, rolcreatedb, rolcreaterole, rolreplication FROM pg_roles WHERE rolname = current_user;'
    if ($taskPrivileges -notmatch '^[A-Za-z0-9_]+\|f\|f\|f\|f$') { throw 'Application database role has excess cluster privileges.' }
    $taskContainerId = (Assert-DockerCommand -Arguments @('compose','ps','-q','server')).Stdout.Trim()
    $taskRuntimeUser = (Assert-DockerCommand -Arguments @('inspect','--format','{{.Config.User}}',$taskContainerId)).Stdout.Trim()
    if ($taskRuntimeUser -ne '10001:10001') { throw 'Runtime container is not configured as dedicated non-root user.' }
    # The integration suite can leave many files to fsync. Do not SIGKILL a normal shutdown after 10s.
    $taskRestart = Assert-DockerCommand -Arguments @('compose','restart','--timeout','120','postgres','server')
    Wait-Ready -TimeoutSeconds 120
    $taskAfter = Get-AppQuery -Query ("SELECT id FROM accounts WHERE normalized_name = '" + $taskUser + "';")
    if ($taskAfter -ne $taskBefore) { throw 'Account identity changed or disappeared after PostgreSQL/server restart.' }
    $taskGameAfter = Get-AppQuery -Query $taskGameDigestQuery
    if ($taskGameAfter -ne $taskGameBefore) { throw 'Game-user fields changed or disappeared after PostgreSQL/server restart.' }
    $taskBanAfter = Get-AppQuery -Query $taskBanDigestQuery
    if ($taskBanAfter -ne $taskBanBefore) { throw 'Ban snapshot changed after restart.' }
    $taskLimitedAfter = Get-AppQuery -Query $taskLimitedDigestQuery
    if ($taskLimitedAfter -ne $taskLimitedBefore) { throw "LimitedAccount changed after restart." }
    $taskEffectAfter = Get-AppQuery -Query $taskEffectDigestQuery
    if ($taskEffectAfter -ne $taskEffectBefore) { throw 'Loyalty, achievement or replay receipt changed after restart.' }
    $taskMigrate = Assert-DockerCommand -Arguments @('compose','run','--rm','--no-deps','migrate')
    if ($taskMigrate.Stdout -notmatch 'migrations applied') { throw 'Idempotent migration did not confirm success.' }
    $taskHashes = @{}
    $taskFiles = @(Get-ChildItem -LiteralPath (Join-Path $taskProject 'src') -Recurse -File) + @(Get-ChildItem -LiteralPath (Join-Path $taskProject 'migrations') -File)
    foreach ($taskFile in $taskFiles) { $taskHashes[$taskFile.FullName.Substring($taskProject.Length + 1).Replace('\','/')] = (Get-FileHash -LiteralPath $taskFile.FullName -Algorithm SHA256).Hash }
    foreach ($taskName in 'Cargo.lock','Dockerfile','compose.yaml') { $taskHashes[$taskName] = (Get-FileHash -LiteralPath (Join-Path $taskProject $taskName) -Algorithm SHA256).Hash }
    $taskEvidence = [ordered]@{
        verified_at_utc = [DateTimeOffset]::UtcNow.ToString('o'); readiness = 'ready'; migrate_exit_code = 0;
        account_provisioned = $taskUser; account_id = $taskBefore; duplicate_rejected = $true;
        database_and_server_restart_preserves_account = $true; database_role_flags = $taskPrivileges;
        synthetic_game_user_persisted = $true; synthetic_game_user_source_columns = 15; synthetic_game_user_id = $taskGameId;
        game_user_snapshot_md5 = $taskGameBefore; database_and_server_restart_preserves_game_user = $true;
        game_user_smoke_method = 'SQL INSERT and synthetic values for all fifteen source fields; Rust adapters covered by isolated integration tests';
        database_and_server_restart_preserves_ban = $true; synthetic_ban_snapshot_md5 = $taskBanBefore;
        synthetic_game_user_local_revision = 1;
        database_and_server_restart_preserves_limited_account = $true; limited_account_snapshot_md5 = $taskLimitedBefore;
        database_and_server_restart_preserves_login_account_effects = $true; login_account_effect_snapshot_md5 = $taskEffectBefore;
        login_account_effects_smoke_method = 'SQL synthetic full loyalty5/achievement4/receipt; Rust atomicity, replay and cancellation tested in isolated schemas';
        runtime_user = $taskRuntimeUser; game_client_compatible = $false; file_sha256 = $taskHashes
    }
    $taskEvidenceDirectory = Join-Path $taskProject 'docs/evidence/verification'
    New-Item -ItemType Directory -Force -Path $taskEvidenceDirectory | Out-Null
    $taskEvidence | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $taskEvidenceDirectory 'docker-smoke.json') -Encoding utf8
    Write-Host 'PASS: migrate, readiness, stdin account creation, duplicate rejection, non-root/non-superuser, PostgreSQL/server restart preserves account and synthetic game user, ban, local revision, loyalty, achievement and replay receipt. Game compatibility remains false.'
} finally { $taskPassword = $null }
