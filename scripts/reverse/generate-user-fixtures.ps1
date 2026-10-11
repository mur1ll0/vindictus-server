[CmdletBinding()]
param([string]$Dotnet='C:\Program Files\dotnet\dotnet.exe', [string]$Compiler='C:\Program Files\dotnet\sdk\9.0.301\Roslyn\bincore\csc.dll')
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try {
    $taskReferences='C:\Program Files (x86)\Reference Assemblies\Microsoft\Framework\.NETFramework\v4.8'
    $taskSource='legacy/decompiled/LoginServiceCore-1600fb4b593d/LoginServiceCore/User.cs'
    $taskArguments=@($Compiler,'/nologo','/noconfig','/nostdlib+','/langversion:latest','/platform:x86','/out:.cache/framework/UserOracle.exe')
    foreach($taskLibrary in 'mscorlib','System','System.Core','System.Data.Linq','System.Web.Extensions'){
        $taskArguments+=('/r:'+(Join-Path $taskReferences ($taskLibrary+'.dll')))
    }
    $taskArguments+=@('scripts/reverse/framework/UserOracle.cs',$taskSource)
    New-Item -ItemType Directory -Force .cache\framework | Out-Null
    & $Dotnet @taskArguments
    if($LASTEXITCODE -ne 0){throw 'Recovered User DTO oracle build failed.'}
    $taskOutput=& .cache\framework\UserOracle.exe
    if($LASTEXITCODE -ne 0){throw 'Recovered User DTO oracle failed.'}
    $taskData=$taskOutput -join [Environment]::NewLine | ConvertFrom-Json
    $taskData | Add-Member -NotePropertyName source_sha256 -NotePropertyValue (Get-FileHash -Algorithm SHA256 -LiteralPath $taskSource).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText((Join-Path $taskProject 'tests/fixtures/framework-user.json'),($taskData | ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
    Write-Host ('Recovered User DTO fixtures='+$taskData.cases.Count+'; runtime='+$taskData.runtime)
} finally {Pop-Location}