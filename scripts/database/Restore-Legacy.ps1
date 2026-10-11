[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][ValidatePattern('^[A-Za-z0-9_-]+\.bak$')][string]$BackupName,
    [Parameter(Mandatory = $true)][ValidatePattern('^study_[A-Za-z0-9_]{1,110}$')][string]$DatabaseName
)
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..')).Path
$backupRoot = (Resolve-Path -LiteralPath (Join-Path $projectRoot '..\ServidorVindictus\database')).Path
$backupPath = Join-Path $backupRoot $BackupName
if (-not (Test-Path -LiteralPath $backupPath -PathType Leaf)) { throw "Backup not found: $BackupName" }
# Password stays in the container environment; never place it on the host command line.
$containerCommand = 'SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -i /migration-scripts/restore-legacy.sql -v DatabaseName="$1" BackupName="$2"'
& docker compose --project-directory $projectRoot --profile legacy exec -T legacy-sqlserver bash -c $containerCommand -- $DatabaseName $BackupName
if ($LASTEXITCODE -ne 0) { throw "Restore failed for $DatabaseName (exit $LASTEXITCODE)." }
