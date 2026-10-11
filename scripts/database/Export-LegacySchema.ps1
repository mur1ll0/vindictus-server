[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][ValidatePattern('^study_[A-Za-z0-9_]{1,110}$')][string]$DatabaseName
)
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..')).Path
$outputDirectory = Join-Path $projectRoot 'docs\evidence\database'
New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
$containerCommand = 'SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d "$1" -y 0 -w 65535 -f 65001 -i /migration-scripts/schema-catalog.sql'
$output = & docker compose --project-directory $projectRoot --profile legacy exec -T legacy-sqlserver bash -c $containerCommand -- $DatabaseName
if ($LASTEXITCODE -ne 0) { throw "Schema export failed for $DatabaseName (exit $LASTEXITCODE)." }
# sqlcmd chunks long FOR JSON results across output rows; concatenate without inserting characters.
$jsonLines = @($output)
$firstJsonLine = -1
for ($index = 0; $index -lt $jsonLines.Count; $index++) {
    if ($jsonLines[$index].StartsWith('{')) { $firstJsonLine = $index; break }
}
if ($firstJsonLine -lt 0) { throw 'No JSON object returned by SQL Server.' }
# sqlcmd -y 0 preserves complete values but cannot combine with -h -1; skip its header.
$json = ($jsonLines[$firstJsonLine..($jsonLines.Count - 1)] | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }) -join ''
$null = ConvertFrom-Json -InputObject $json
$outputPath = Join-Path $outputDirectory "$DatabaseName.schema.json"
[IO.File]::WriteAllText($outputPath, $json, [Text.UTF8Encoding]::new($false))
Write-Output $outputPath

