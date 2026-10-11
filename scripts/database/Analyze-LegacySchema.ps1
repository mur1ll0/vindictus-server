[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..')).Path
$evidenceRoot = Join-Path $projectRoot 'docs\evidence\database'
$tokens = @('NOLOCK','MERGE','SCOPE_IDENTITY','@@ROWCOUNT','GETDATE','GETUTCDATE','NEWID','TRANSACTION','UPDLOCK','IDENTITY_INSERT')
$analyses = @(Get-ChildItem -LiteralPath $evidenceRoot -Filter '*.schema.json' | Sort-Object Name | ForEach-Object {
    $catalog = Get-Content -LiteralPath $_.FullName -Raw | ConvertFrom-Json
    [ordered]@{
        database = $catalog.database_name
        construct_module_counts = @(foreach ($token in $tokens) { [ordered]@{ token = $token; modules = @($catalog.sql_modules | Where-Object { $_.definition -match [regex]::Escape($token) }).Count } })
        cross_database_dependencies = @($catalog.dependencies | Where-Object referenced_database_name | Select-Object referencing_id,referenced_database_name,referenced_schema_name,referenced_entity_name)
        rowversion_columns = @($catalog.columns | Where-Object declared_type -EQ 'timestamp' | ForEach-Object {
            $column = $_
            $tableName = ($catalog.tables | Where-Object object_id -EQ $column.object_id | Select-Object -First 1).name
            [ordered]@{ table = $tableName; column = $column.name; byte_length = $column.max_length }
        })
        primary_key_count = @($catalog.key_constraints | Where-Object type -EQ 'PK').Count
        foreign_key_count = @($catalog.foreign_keys | Where-Object { $null -ne $_ }).Count
        modules_without_definition = @($catalog.sql_modules | Where-Object { -not $_.definition }).Count
    }
})
$outputPath = Join-Path $evidenceRoot 'analysis.json'
[IO.File]::WriteAllText($outputPath, (ConvertTo-Json -InputObject $analyses -Depth 8), [Text.UTF8Encoding]::new($false))
Write-Output $outputPath
