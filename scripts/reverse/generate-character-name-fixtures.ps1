$ErrorActionPreference='Stop'
& py -3.13 -X utf8 scripts/reverse/read-character-name-profile.py
if($LASTEXITCODE-ne0){throw 'Read-only naming profile recovery failed'}
$taskNamingProfile=Get-Content .cache/verification/character-name-profile.json -Raw|ConvertFrom-Json -DateKind String
$taskSource='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore.Functions/CharacterMaker.cs'
$taskTemplate='scripts/reverse/framework/CharacterNameOracle.cs.in'
$taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskSource))
$taskStart=$taskCode.IndexOf('public bool CheckValidName(string name)')
if($taskStart-lt0){throw 'Recovered name method unavailable'}
$taskOpen=$taskCode.IndexOf('{',$taskStart);$taskDepth=1;$taskEnd=$taskOpen+1
while($taskDepth-gt0-and$taskEnd-lt$taskCode.Length){if($taskCode[$taskEnd]-eq'{'){$taskDepth++};if($taskCode[$taskEnd]-eq'}'){$taskDepth--};$taskEnd++}
if($taskDepth-ne0){throw 'Incomplete recovered method'}
$taskMethod=$taskCode.Substring($taskStart,$taskEnd-$taskStart)
$taskHarness='.cache/framework/CharacterNameOracle.cs'
[IO.File]::WriteAllText((Join-Path $PWD $taskHarness),([IO.File]::ReadAllText((Join-Path $PWD $taskTemplate))).Replace('__RECOVERED_METHOD__',$taskMethod),[Text.UTF8Encoding]::new($false))
$taskProfiles=@()
foreach($taskArch in 'x86','x64'){
 $taskExe='.cache/framework/CharacterName-'+$taskArch+'.exe'
 $taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
 foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLib+'.dll')}
 $taskArgs+=$taskHarness
 & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs *> ('.cache/verification/character-name-build-'+$taskArch+'.log')
 if($LASTEXITCODE-ne0){throw 'Name oracle compilation failed; inspect local log'}
 if(Select-String ('.cache/verification/character-name-build-'+$taskArch+'.log') -Pattern 'warning [A-Z]+[0-9]+:'){throw 'Name oracle compiler warnings'}
 & (Join-Path $PWD $taskExe) *> ('.cache/verification/character-name-'+$taskArch+'.json')
 if($LASTEXITCODE-ne0){throw 'Name oracle execution failed; inspect local log'}
 $taskProfile=Get-Content ('.cache/verification/character-name-'+$taskArch+'.json') -Raw|ConvertFrom-Json
 if($taskProfile.pattern-cne$taskNamingProfile.features.NamingRule){throw 'Oracle pattern differs from original effective content'}
 if($taskProfile.cases.Count-ne1050){throw 'Name oracle case count mismatch'}
 $taskProfiles+=$taskProfile
}
if(($taskProfiles[0].cases|ConvertTo-Json -Depth 8 -Compress)-cne($taskProfiles[1].cases|ConvertTo-Json -Depth 8 -Compress)){throw 'Name oracle architecture disagreement'}
[ordered]@{profiles=$taskProfiles;recovered_profile=$taskNamingProfile;sources=@(($taskSource,$taskTemplate,'scripts/reverse/generate-character-name-fixtures.ps1','scripts/reverse/read-character-name-profile.py')|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});original_services_executed=$false;original_databases_written=$false;scope='CheckValidName only, recovered EN-EU NamingRule; no forbidden/reservation/duplicate checks'}|ConvertTo-Json -Depth 12 -Compress|Set-Content tests/fixtures/framework-character-name.json -Encoding utf8NoBOM
Write-Host 'CheckValidName:1050synthetic cases per architecture;0differences; no original database or service executed.'
