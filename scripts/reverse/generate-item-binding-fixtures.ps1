$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try {
 New-Item -ItemType Directory -Force .cache/framework,.cache/verification | Out-Null
 py -3.13 scripts/reverse/extract-item-binding.py
 if($LASTEXITCODE -ne 0){throw 'Binding predicate extraction failed'}
 $taskProfiles=@()
 foreach($taskArch in 'x86','x64'){
  $taskExe='.cache/framework/ItemBinding-'+$taskArch+'.exe'
  $taskArgs=@('C:\Program Files\dotnet\sdk\9.0.301\Roslyn\bincore\csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
  foreach($taskLib in 'mscorlib','System','System.Core','System.Data','System.Web.Extensions'){
   $taskArgs+=('/r:C:\Program Files (x86)\Reference Assemblies\Microsoft\Framework\.NETFramework\v4.8\'+$taskLib+'.dll')
  }
  $taskArgs+=@('.cache/framework/ItemBindingRecovered.cs','scripts/reverse/framework/ItemBindingOracle.cs')
  & 'C:\Program Files\dotnet\dotnet.exe' @taskArgs *> .cache/verification/item-binding-oracle-build.log
  if($LASTEXITCODE -ne 0){Get-Content .cache/verification/item-binding-oracle-build.log -Tail 6;throw 'Binding oracle build failed'}
  $taskRaw=& (Join-Path $PWD $taskExe) 2> .cache/verification/item-binding-oracle-error.log
  if($LASTEXITCODE -ne 0){throw 'Binding oracle failed'}
  $taskData=$taskRaw|ConvertFrom-Json -DateKind String
  if($taskData.architecture -ne $taskArch){throw 'Binding architecture mismatch'}
  $taskProfiles+=$taskData
 }
 foreach($taskPart in 'unsupported','gemstone_values'){
  if(($taskProfiles[0].$taskPart|ConvertTo-Json -Depth 14 -Compress) -ne ($taskProfiles[1].$taskPart|ConvertTo-Json -Depth 14 -Compress)){throw 'Binding architecture disagreement'}
 }
 if(($taskProfiles[0].cases|ConvertTo-Json -Depth 14 -Compress) -ne ($taskProfiles[1].cases|ConvertTo-Json -Depth 14 -Compress)){throw 'Binding architecture disagreement'}
 $taskSources=@('legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/ItemClassConst.cs','legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore.TradeSystem/ItemGenerator.cs','legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore.ItemInstance/Item.cs','legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/ItemContentsExtension.cs','legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore.ItemInstance/GemstoneType.cs','legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore.ItemInstance/AttributeClass.cs','legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/ItemContents.cs','legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore.Static/QueuedItemController.cs','scripts/reverse/extract-item-binding.py','scripts/reverse/framework/ItemBindingOracle.cs')
 $taskFixture=[ordered]@{profiles=$taskProfiles;sources=@($taskSources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});adaptations=@('CheckCharacterBind, CheckHighLevelEquipment and IsEquippable predicates extracted without rewriting their bodies','GetEquipItemInfo replaced with explicit synthetic dictionary-membership projection; equip lookup calls are not traced','Feature enable/integer and enchant lookup supplied by deterministic synthetic dependencies; trace records only these calls','CurrentCulture en-US with user overrides disabled; printable ASCII class operations only in Rust, unknown culture inputs deliberately refused','Enum recovered from source; original source services and VM never executed; no inventory or queue writes');source_services_executed=$false;database_writes=$false}
 $taskJson=($taskFixture|ConvertTo-Json -Depth 18).Replace(([string][char]13+[char]10),([string][char]10))+[char]10
 [IO.File]::WriteAllText((Join-Path $PWD 'tests/fixtures/framework-item-binding.json'),$taskJson,[Text.UTF8Encoding]::new($false))
 Write-Output ('Binding oracle PASS; architectures=2; culture=en-US; cases/architecture='+$taskProfiles[0].cases.Count+'; unsupported='+$taskProfiles[0].unsupported.Count)
} finally {Pop-Location}
