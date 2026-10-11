[CmdletBinding()]
param([string]$Dotnet='C:\Program Files\dotnet\dotnet.exe', [string]$Compiler='C:\Program Files\dotnet\sdk\9.0.301\Roslyn\bincore\csc.dll')
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try {
    $taskReferences='C:\Program Files (x86)\Reference Assemblies\Microsoft\Framework\.NETFramework\v4.8'
    if(-not (Test-Path -LiteralPath (Join-Path $taskReferences 'mscorlib.dll'))){throw '.NET Framework4.8 reference assemblies required.'}
    $taskArguments=@($Compiler,'/nologo','/noconfig','/nostdlib+','/unsafe+','/langversion:latest','/platform:x86','/out:.cache/framework/FrameworkWire.exe')
    foreach($taskLibrary in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArguments+=('/r:'+ (Join-Path $taskReferences ($taskLibrary+'.dll')))}
    $taskArguments+='scripts/reverse/framework/FrameworkWire.cs'
    $taskRecovered='legacy/decompiled/Core-8942a0c26a9a/'
    foreach($taskFile in 'Devcat.Core.Net.Message/Packet.cs','Devcat.Core.Net/IPacket.cs','Devcat.Core.Net/ICryptoTransform.cs','Devcat.Core.Net.Message/ClassInfo.cs','Devcat.Core.Net.Message/SerializeWriter.cs','Devcat.Core.Net.Message/SerializeWriterHelper.cs','Devcat.Core.Net.Message/SerializedSizeCalculator.cs','Devcat.Core.Net.Message/SerializedSizeCalculatorHelper.cs'){$taskArguments+=($taskRecovered+$taskFile)}
    foreach($taskFile in 'ServiceCore/Isaac.cs','ServiceCore/CryptoTransformHeroes.cs','ServiceCore.EndPointNetwork/IMessage.cs','ServiceCore.EndPointNetwork/SyncFeatureMatrixMessage.cs','ServiceCore.EndPointNetwork/LoginFailMessage.cs'){$taskArguments+=('legacy/decompiled/ServiceCore-be65d59d2385/'+$taskFile)}
    New-Item -ItemType Directory -Force .cache\framework | Out-Null
    & $Dotnet @taskArguments
    if($LASTEXITCODE -ne 0){throw 'Framework serializer oracle build failed.'}
    $taskOriginal=(Resolve-Path -LiteralPath '..\ServidorVindictus\Server\zh-tw-x-gm\Bin\ServiceCore.dll').Path
    $taskOutput=& .cache\framework\FrameworkWire.exe $taskOriginal
    if($LASTEXITCODE -ne 0){throw 'Framework serializer oracle failed.'}
    $taskData=$taskOutput -join "`n" | ConvertFrom-Json
    [IO.File]::WriteAllText((Join-Path $taskProject 'tests/fixtures/framework-converter.json'),($taskData | ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
    Write-Host ('Framework converter fixtures='+$taskData.converters.Count+'; runtime='+$taskData.runtime)
} finally {Pop-Location}
