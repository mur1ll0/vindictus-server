$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
Push-Location $taskRoot
try {
 $taskSource='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/Character.cs'
 $taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskSource))
 $taskMatch=[regex]::Match($taskCode,'(?s)\t\tint num2 = ServiceCore.FeatureMatrix.GetInteger\("DeleteCharacterDelay"\) \* 60;.*?(?=\t\treturn new CharacterSummary)')
 if(-not$taskMatch.Success){throw 'Original MakeSummary arithmetic not found'}
 $taskBlock=$taskMatch.Value.Replace('ServiceCore.FeatureMatrix.GetInteger("DeleteCharacterDelay")','delay').Replace('DateTime.UtcNow','utc()').Replace('CharacterInfo.DeleteTime','deleted')
 $taskHarness='scripts/reverse/framework/MinimumDeleteWaitOracle.cs'
 $taskCopy='.cache/framework/MinimumDeleteWaitOracle.cs'
 [IO.File]::WriteAllText((Join-Path $PWD $taskCopy),([IO.File]::ReadAllText((Join-Path $PWD $taskHarness))).Replace('// SOURCE_BLOCK',$taskBlock),[Text.UTF8Encoding]::new($false))
 $taskProfiles=@()
 foreach($taskArch in 'x86','x64'){
  $taskExe='.cache/framework/MinimumDeleteWait-'+$taskArch+'.exe'
  $taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
  foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLib+'.dll')}
  $taskArgs+=$taskCopy
  & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs *> ('.cache/verification/minimum-delete-build-'+$taskArch+'.log')
  if($LASTEXITCODE-ne0){throw 'Arithmetic oracle compile failed;local log'}
  & (Join-Path $PWD $taskExe) > ('.cache/verification/minimum-delete-'+$taskArch+'.json')
  if($LASTEXITCODE-ne0){throw 'Arithmetic oracle failed'}
  $taskProfiles+=Get-Content ('.cache/verification/minimum-delete-'+$taskArch+'.json') -Raw|ConvertFrom-Json
 }
 if(($taskProfiles[0].cases|ConvertTo-Json -Compress)-cne($taskProfiles[1].cases|ConvertTo-Json -Compress)){throw 'Architecture disagreement'}
 [ordered]@{profiles=$taskProfiles;sources=@($taskSource,$taskHarness,$MyInvocation.MyCommand.Path|ForEach-Object{[ordered]@{path=[IO.Path]::GetRelativePath($PWD,$(Resolve-Path $_));sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});method='Original MakeSummary delete arithmetic block;only content,CharacterInfo and UTC boundaries injected;no original service execution.'}|ConvertTo-Json -Depth 12 -Compress|Set-Content tests/fixtures/framework-minimum-delete-wait.json -Encoding utf8NoBOM
 Write-Output ('MakeSummary arithmetic PASS;'+$taskProfiles[0].cases.Count+' cases per architecture')
} finally {Pop-Location}
