[CmdletBinding()]
param([string]$Dotnet = 'C:\Program Files\dotnet\dotnet.exe')
$ErrorActionPreference = 'Stop'
$taskProject = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try {
    $env:DOTNET_CLI_HOME = Join-Path $taskProject '.cache\dotnet'
    $env:NUGET_PACKAGES = Join-Path $taskProject '.cache\nuget'
    $env:DOTNET_CLI_TELEMETRY_OPTOUT = '1'
    & $Dotnet build 'scripts\reverse\oracle\Oracle.csproj' -o '.cache\oracle' --nologo
    if($LASTEXITCODE -ne 0) { throw 'Recovered C# oracle build failed; run inventory/decompilation first.' }
    $taskOracle = & $Dotnet '.cache\oracle\Oracle.dll'
    if($LASTEXITCODE -ne 0) { throw 'Recovered C# oracle execution failed.' }
    [IO.File]::WriteAllText((Join-Path $taskProject 'tests\fixtures\legacy-packet.json'),($taskOracle -join "`n"),[Text.UTF8Encoding]::new($false))
    Write-Host 'Fixture regenerated from recovered C# without executing original assemblies.'
} finally { Pop-Location }
