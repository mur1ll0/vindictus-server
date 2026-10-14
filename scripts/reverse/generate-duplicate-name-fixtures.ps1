$ErrorActionPreference='Stop'
$taskSource='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore.Functions/CharacterMaker.cs'
$taskTemplate='scripts/reverse/framework/DuplicateNameOracle.cs.in'
$taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskSource))
$taskStart=$taskCode.IndexOf('private bool CheckDuplicateName(')
if($taskStart-lt0){throw 'Recovered duplicate method unavailable'}
$taskOpen=$taskCode.IndexOf('{',$taskStart);$taskDepth=1;$taskEnd=$taskOpen+1
while($taskDepth-gt0-and$taskEnd-lt$taskCode.Length){if($taskCode[$taskEnd]-eq'{'){$taskDepth++};if($taskCode[$taskEnd]-eq'}'){$taskDepth--};$taskEnd++}
if($taskDepth-ne0){throw 'Incomplete recovered method'}
$taskMethod=$taskCode.Substring($taskStart,$taskEnd-$taskStart).Replace('DateTime.UtcNow','Clock')
$taskHarness='.cache/framework/DuplicateNameOracle.cs'
[IO.File]::WriteAllText((Join-Path $PWD $taskHarness),([IO.File]::ReadAllText((Join-Path $PWD $taskTemplate))).Replace('__RECOVERED_METHOD__',$taskMethod),[Text.UTF8Encoding]::new($false))
$taskProfiles=@()
foreach($taskArch in 'x86','x64'){
 $taskExe='.cache/framework/DuplicateName-'+$taskArch+'.exe'
 $taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
 foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLib+'.dll')}
 $taskArgs+=$taskHarness
 & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs *> ('.cache/verification/duplicate-name-build-'+$taskArch+'.log')
 if($LASTEXITCODE-ne0){throw 'Duplicate oracle compilation failed; local log only'}
 if(Select-String ('.cache/verification/duplicate-name-build-'+$taskArch+'.log') -Pattern 'warning [A-Z]+[0-9]+:'){throw 'Duplicate oracle compiler warnings'}
 & (Join-Path $PWD $taskExe) *> ('.cache/verification/duplicate-name-'+$taskArch+'.json')
 if($LASTEXITCODE-ne0){throw 'Duplicate oracle execution failed; local log only'}
 $taskProfile=Get-Content ('.cache/verification/duplicate-name-'+$taskArch+'.json') -Raw|ConvertFrom-Json
 if($taskProfile.cases.Count-ne480){throw 'Duplicate oracle count differs'}
 $taskProfiles+=$taskProfile
}
if(($taskProfiles[0].cases|ConvertTo-Json -Depth 8 -Compress)-cne($taskProfiles[1].cases|ConvertTo-Json -Depth 8 -Compress)){throw 'Duplicate oracle architecture disagreement'}
[ordered]@{profiles=$taskProfiles;sources=@(($taskSource,$taskTemplate,'scripts/reverse/generate-duplicate-name-fixtures.ps1')|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});scope='Temporal decision after a matched SQL name rowset; synthetic ordinal harness does not recover SQL Unicode equality';original_services_executed=$false}|ConvertTo-Json -Depth 10 -Compress|Set-Content tests/fixtures/framework-duplicate-name.json -Encoding utf8NoBOM
Write-Host 'Duplicate temporal oracle:480cases per architecture;0differences; SQL name matching remains separate.'
