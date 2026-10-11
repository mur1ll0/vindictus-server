[CmdletBinding()]
param([string]$Dotnet='C:\Program Files\dotnet\dotnet.exe')
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try {
    $env:DOTNET_CLI_HOME=Join-Path $taskProject '.cache\dotnet'
    $env:NUGET_PACKAGES=Join-Path $taskProject '.cache\nuget'
    $env:DOTNET_CLI_TELEMETRY_OPTOUT='1'
    & $Dotnet build scripts\reverse\metadata\Metadata.csproj -o .cache\metadata --nologo
    if($LASTEXITCODE -ne 0){throw 'Metadata reader build failed'}
    $taskSource=Join-Path $taskProject '..\ServidorVindictus\Server\zh-tw-x-gm\Bin\ServiceCore.dll'
    $taskEvidence=& $Dotnet .cache\metadata\Metadata.dll $taskSource
    if($LASTEXITCODE -ne 0){throw 'Metadata export failed'}
    New-Item -ItemType Directory -Force docs\evidence\reverse | Out-Null
    [IO.File]::WriteAllText((Join-Path $taskProject 'docs\evidence\reverse\messages.json'),($taskEvidence -join "`n"),[Text.UTF8Encoding]::new($false))
    $taskJson=$taskEvidence -join "`n" | ConvertFrom-Json
    Write-Host ('Read-only messages: '+$taskJson.messages.Count+'; categories are candidates, not verified opcodes.')
} finally {Pop-Location}
