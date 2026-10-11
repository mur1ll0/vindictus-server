[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..')).Path
$backupRoot = (Resolve-Path -LiteralPath (Join-Path $projectRoot '..\ServidorVindictus\database')).Path
$evidenceRoot = Join-Path $projectRoot 'docs\evidence\database'
$backupNames = @('heroes.bak','HeroesExcept.bak','heroesLog.bak','heroesMarketPlace.bak','heroesRank.bak','heroesShare.bak')
$checks = @(foreach ($backupName in $backupNames) {
    $databaseName = 'study_' + [IO.Path]::GetFileNameWithoutExtension($backupName)
    $containerCommand = 'SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d "$1" -i /migration-scripts/checkdb-legacy.sql'
    $result = & docker compose --project-directory $projectRoot --profile legacy exec -T legacy-sqlserver bash -c $containerCommand -- $databaseName
    if ($LASTEXITCODE -ne 0) { throw "CHECKDB failed for $databaseName (exit $LASTEXITCODE)." }
    if (($result -join "`n") -notmatch 'CHECKDB_OK') { throw "Missing CHECKDB success marker for $databaseName." }
    $backupPath = Join-Path $backupRoot $backupName
    [ordered]@{
        database = $databaseName
        backup = $backupName
        backup_sha256 = (Get-FileHash -LiteralPath $backupPath -Algorithm SHA256).Hash.ToLowerInvariant()
        backup_bytes = (Get-Item -LiteralPath $backupPath).Length
        schema_sha256 = (Get-FileHash -LiteralPath (Join-Path $evidenceRoot "$databaseName.schema.json") -Algorithm SHA256).Hash.ToLowerInvariant()
        checkdb = 'passed'
        checked_at_utc = [DateTimeOffset]::UtcNow.ToString('o')
    }
    Write-Information "$databaseName CHECKDB passed" -InformationAction Continue
})
$outputPath = Join-Path $evidenceRoot 'verification.json'
[IO.File]::WriteAllText($outputPath, (ConvertTo-Json -InputObject $checks -Depth 4), [Text.UTF8Encoding]::new($false))
Write-Output $outputPath
