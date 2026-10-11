[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..')).Path
$evidenceRoot = Join-Path $projectRoot 'docs\evidence\database'
$summary = @(Get-ChildItem -LiteralPath $evidenceRoot -Filter '*.schema.json' | Sort-Object Name | ForEach-Object {
    $catalog = Get-Content -LiteralPath $_.FullName -Raw | ConvertFrom-Json
    $objects = @($catalog.objects)
    [ordered]@{
        database = $catalog.database_name
        collation = $catalog.collation
        compatibility_level = $catalog.compatibility_level
        tables = @($catalog.tables | Where-Object { $null -ne $_ }).Count
        procedures = @($objects | Where-Object { $_.type.Trim() -in @('P', 'PC') }).Count
        functions = @($objects | Where-Object { $_.type.Trim() -in @('FN', 'IF', 'TF', 'FS', 'FT') }).Count
        views = @($objects | Where-Object { $_.type.Trim() -eq 'V' }).Count
        triggers = @($catalog.triggers | Where-Object { $null -ne $_ }).Count
        foreign_keys = @($catalog.foreign_keys | Where-Object { $null -ne $_ }).Count
        indexes = @($catalog.indexes | Where-Object index_id -GT 0).Count
        user_types = @($catalog.user_types | Where-Object { $null -ne $_ }).Count
        clr_modules = @($catalog.assembly_modules | Where-Object { $null -ne $_ }).Count
        modules_without_definition = @($catalog.sql_modules | Where-Object { -not $_.definition }).Count
        approximate_rows = ($catalog.row_counts | Measure-Object -Property approximate_rows -Sum).Sum
        column_types = @($catalog.columns | Group-Object declared_type | Sort-Object Name | ForEach-Object { [ordered]@{ type = $_.Name; count = $_.Count } })
        candidate_account_tables = @($catalog.tables | Where-Object { $_.name -match '(?i)account|user|member|login' } | Select-Object -ExpandProperty name)
    }
})
$outputPath = Join-Path $evidenceRoot 'summary.json'
[IO.File]::WriteAllText($outputPath, (ConvertTo-Json -InputObject $summary -Depth 8), [Text.UTF8Encoding]::new($false))
$summary | ForEach-Object { [PSCustomObject]@{ Database=$_.database; Collation=$_.collation; Tables=$_.tables; Procedures=$_.procedures; Functions=$_.functions; Triggers=$_.triggers; Rows=$_.approximate_rows } } | Format-Table -AutoSize


