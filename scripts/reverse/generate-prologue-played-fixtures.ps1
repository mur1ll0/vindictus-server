$ErrorActionPreference='Stop'
$taskSource='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/CharacterDataContext.cs'
$taskTemplate='scripts/reverse/framework/ProloguePlayedOracle.cs.in'
$taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskSource))
$taskStart=$taskCode.IndexOf('public void ProloguePlayed(int nexonSN)')
if($taskStart-lt0){throw 'Recovered method unavailable'}
$taskOpen=$taskCode.IndexOf('{',$taskStart);$taskDepth=1;$taskEnd=$taskOpen+1
while($taskDepth-gt0-and$taskEnd-lt$taskCode.Length){if($taskCode[$taskEnd]-eq'{'){$taskDepth++};if($taskCode[$taskEnd]-eq'}'){$taskDepth--};$taskEnd++}
if($taskDepth-ne0){throw 'Incomplete recovered method'}
$taskMethod=$taskCode.Substring($taskStart,$taskEnd-$taskStart)
$taskHarness='.cache/framework/ProloguePlayedOracle.cs'
[IO.File]::WriteAllText((Join-Path $PWD $taskHarness),([IO.File]::ReadAllText((Join-Path $PWD $taskTemplate))).Replace('__RECOVERED_METHOD__',$taskMethod),[Text.UTF8Encoding]::new($false))
$taskProfiles=@()
foreach($taskArch in 'x86','x64'){
 $taskExe='.cache/framework/ProloguePlayed-'+$taskArch+'.exe'
 $taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
 foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLib+'.dll')}
 $taskArgs+=$taskHarness
 & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs *> ('.cache/verification/prologue-build-'+$taskArch+'.log')
 if($LASTEXITCODE-ne0){throw 'Oracle compilation failed; inspect local log'}
 if(Select-String ('.cache/verification/prologue-build-'+$taskArch+'.log') -Pattern 'warning [A-Z]+[0-9]+:'){throw 'Oracle compiler warnings'}
 & (Join-Path $PWD $taskExe) *> ('.cache/verification/prologue-'+$taskArch+'.json')
 if($LASTEXITCODE-ne0){throw 'Oracle execution failed; inspect local log'}
 $taskProfile=Get-Content ('.cache/verification/prologue-'+$taskArch+'.json') -Raw|ConvertFrom-Json
 if($taskProfile.cases.Count-ne72){throw 'Oracle case count mismatch'}
 $taskProfiles+=$taskProfile
}
if(($taskProfiles[0].cases|ConvertTo-Json -Depth 8 -Compress)-cne($taskProfiles[1].cases|ConvertTo-Json -Depth 8 -Compress)){throw 'Oracle architecture disagreement'}
[ordered]@{profiles=$taskProfiles;sources=@(($taskSource,$taskTemplate,'scripts/reverse/generate-prologue-played-fixtures.ps1')|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});original_services_executed=$false;original_databases_written=$false}|ConvertTo-Json -Depth 12 -Compress|Set-Content tests/fixtures/framework-prologue-played.json -Encoding utf8NoBOM
Write-Host 'ProloguePlayed:72synthetic cases per architecture;0differences; no original database or service executed.'
