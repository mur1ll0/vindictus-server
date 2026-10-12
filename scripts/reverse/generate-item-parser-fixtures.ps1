$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try {
 New-Item -ItemType Directory -Force .cache/framework,.cache/verification | Out-Null
 py -3.13 scripts/reverse/extract-item-parser.py
 if($LASTEXITCODE -ne 0){throw 'Queue predicate extraction failed'}
 $taskProfiles=@()
 foreach($taskArch in 'x86','x64'){
  $taskExe='.cache/framework/ItemParser-'+$taskArch+'.exe'
  $taskArgs=@('C:\Program Files\dotnet\sdk\9.0.301\Roslyn\bincore\csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
  foreach($taskLib in 'mscorlib','System','System.Core','System.Data','System.Web.Extensions'){
   $taskArgs+=('/r:C:\Program Files (x86)\Reference Assemblies\Microsoft\Framework\.NETFramework\v4.8\'+$taskLib+'.dll')
  }
  $taskArgs+=@('.cache/framework/ItemParserRecovered.cs','scripts/reverse/framework/ItemParserOracle.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.ItemServiceOperations/ItemAttributeElement.cs','legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore.ItemInstance/ItemLocation.cs')
  & 'C:\Program Files\dotnet\dotnet.exe' @taskArgs *> .cache/verification/item-parser-oracle-build.log
  if($LASTEXITCODE -ne 0){Get-Content .cache/verification/item-parser-oracle-build.log -Tail 6;throw 'Queue oracle build failed'}
  $taskRaw=& (Join-Path $PWD $taskExe) 2> .cache/verification/item-parser-oracle-error.log
  if($LASTEXITCODE -ne 0){throw 'Queue oracle failed'}
  $taskData=$taskRaw|ConvertFrom-Json -DateKind String
  if($taskData.architecture -ne $taskArch){throw 'Queue architecture mismatch'}
  $taskProfiles+=$taskData
 }
 if(($taskProfiles[0].profiles|ConvertTo-Json -Depth 14 -Compress) -ne ($taskProfiles[1].profiles|ConvertTo-Json -Depth 14 -Compress)){throw 'Parser architecture disagreement'}
 if(($taskProfiles[0].locations|ConvertTo-Json -Depth 8 -Compress) -ne ($taskProfiles[1].locations|ConvertTo-Json -Depth 8 -Compress)){throw 'Location architecture disagreement'}
 $taskSources=@('legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.ItemServiceOperations/ItemClassExBuilder.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.ItemServiceOperations/ItemAttributeElement.cs','legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore.ItemInstance/ItemLocation.cs','scripts/reverse/extract-item-parser.py','scripts/reverse/framework/ItemParserOracle.cs')
 $taskFixture=[ordered]@{profiles=$taskProfiles;sources=@($taskSources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});adaptations=@('ToArgInt replaced by explicit deterministic resolver; raw calls recorded; date/culture/local-clock interpretation not ported','int.TryParse Integer primitive verified separately in five cultures','attribute DTO and ItemLocation compiled from recovered source unchanged','Utility dictionary lookup/logger represented by bounded synthetic shims');source_services_executed=$false;database_writes=$false}
 $taskJson=($taskFixture|ConvertTo-Json -Depth 18).Replace(([string][char]13+[char]10),([string][char]10))+[char]10
 [IO.File]::WriteAllText((Join-Path $PWD 'tests/fixtures/framework-item-parser.json'),$taskJson,[Text.UTF8Encoding]::new($false))
 Write-Output ('Parser oracle PASS; profiles=2; cultures/profile=5; syntax/culture='+$taskProfiles[0].profiles[0].cases.Count+'; integer/culture='+$taskProfiles[0].profiles[0].numbers.Count+'; equality/culture='+$taskProfiles[0].profiles[0].pairs.Count+'; locations/profile='+$taskProfiles[0].locations.Count)
} finally {Pop-Location}
