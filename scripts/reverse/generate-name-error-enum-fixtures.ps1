$ErrorActionPreference='Stop'
$taskName='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/CharacterNameCheckResult.cs'
$taskCreate='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/CreateCharacterResult.cs'
$taskTemplate='scripts/reverse/framework/NameErrorEnumOracle.cs.in'
$taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskTemplate)).Replace('__NAME_ENUM__',[IO.File]::ReadAllText((Join-Path $PWD $taskName)).Replace('namespace ServiceCore.EndPointNetwork;','')).Replace('__CREATE_ENUM__',[IO.File]::ReadAllText((Join-Path $PWD $taskCreate)).Replace('namespace ServiceCore.EndPointNetwork;',''))
[IO.File]::WriteAllText((Join-Path $PWD '.cache/framework/NameErrorEnumOracle.cs'),$taskCode,[Text.UTF8Encoding]::new($false))
$taskProfiles=@()
foreach($taskArch in 'x86','x64'){
 $taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+',('/platform:'+$taskArch),('/out:.cache/framework/NameErrorEnumOracle-'+$taskArch+'.exe'))
 foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLib+'.dll')}
 $taskArgs+='.cache/framework/NameErrorEnumOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs *> ('.cache/verification/name-error-enum-build-'+$taskArch+'.log')
 if($LASTEXITCODE-ne0){throw 'Recovered enum oracle build failed; local log only'}
 if(Select-String ('.cache/verification/name-error-enum-build-'+$taskArch+'.log') -Pattern 'warning [A-Z]+[0-9]+:'){throw 'Enum oracle compiler warnings'}
 & (Join-Path $PWD ('.cache/framework/NameErrorEnumOracle-'+$taskArch+'.exe')) *> ('.cache/verification/name-error-enum-'+$taskArch+'.json')
 if($LASTEXITCODE-ne0){throw 'Recovered enum oracle failed; local log only'}
 $taskProfile=Get-Content ('.cache/verification/name-error-enum-'+$taskArch+'.json') -Raw|ConvertFrom-Json
 if($taskProfile.cases.Count-ne7-or@($taskProfile.cases|Where-Object {$null-ne$_.error}).Count-ne0){throw 'Recovered enum result differs'}
 $taskProfiles+=$taskProfile
}
if(($taskProfiles[0].cases|ConvertTo-Json -Depth 7 -Compress)-cne($taskProfiles[1].cases|ConvertTo-Json -Depth 7 -Compress)){throw 'Enum architecture disagreement'}
[ordered]@{profiles=$taskProfiles;sources=@(($taskName,$taskCreate,$taskTemplate,'scripts/reverse/generate-name-error-enum-fixtures.ps1')|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});original_services_executed=$false}|ConvertTo-Json -Depth 8 -Compress|Set-Content tests/fixtures/framework-name-error-enum.json -Encoding utf8NoBOM
Write-Output 'Name error enum conversion:7cases per architecture;0differences.'
