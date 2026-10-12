$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try {
 New-Item -ItemType Directory -Force .cache/framework,.cache/verification | Out-Null
 py -3.13 scripts/reverse/extract-item-catalog.py
 if($LASTEXITCODE -ne 0){throw 'Catalog predicate extraction failed'}
 $taskProfiles=@()
 foreach($taskArch in 'x86','x64'){
  $taskExe='.cache/framework/ItemCatalog-'+$taskArch+'.exe'
  $taskArgs=@('C:\Program Files\dotnet\sdk\9.0.301\Roslyn\bincore\csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
  foreach($taskLib in 'mscorlib','System','System.Core','System.Data','System.Web.Extensions'){
   $taskArgs+=('/r:C:\Program Files (x86)\Reference Assemblies\Microsoft\Framework\.NETFramework\v4.8\'+$taskLib+'.dll')
  }
  $taskArgs+=@('.cache/framework/ItemCatalogRecovered.cs','scripts/reverse/framework/ItemCatalogOracle.cs')
  & 'C:\Program Files\dotnet\dotnet.exe' @taskArgs *> .cache/verification/item-catalog-oracle-build.log
  if($LASTEXITCODE -ne 0){Get-Content .cache/verification/item-catalog-oracle-build.log -Tail 6;throw 'Catalog oracle build failed'}
  $taskRaw=& (Join-Path $PWD $taskExe) '.cache/private-content/item-catalog.json' 'src/domain/feature_version_pairs.json' 2> .cache/verification/item-catalog-oracle-error.log
  if($LASTEXITCODE -ne 0){throw 'Catalog oracle failed'}
  $taskData=$taskRaw|ConvertFrom-Json -DateKind String
  if($taskData.architecture -ne $taskArch){throw 'Catalog architecture mismatch'}
  if($taskData.actual.error -or -not $taskData.actual.summary){throw 'Actual catalog projection failed'}
  $taskProfiles+=$taskData
 }
 foreach($taskPart in 'version_pairs','actual'){
  if(($taskProfiles[0].$taskPart|ConvertTo-Json -Depth 14 -Compress) -ne ($taskProfiles[1].$taskPart|ConvertTo-Json -Depth 14 -Compress)){throw 'Catalog architecture disagreement'}
 }
 if(($taskProfiles[0].cases|ConvertTo-Json -Depth 14 -Compress) -ne ($taskProfiles[1].cases|ConvertTo-Json -Depth 14 -Compress)){throw 'Catalog architecture disagreement'}
 $taskSources=@('legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/ItemContents.cs','legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/ItemContentsExtension.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.HeroesContents/IEnumerableExtension.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.HeroesContents/HeroesContentsLoader.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore/FeatureMatrix.cs','legacy/decompiled/Utility-37359981d509/Utility/DictionaryExtension.cs','scripts/reverse/extract-item-catalog.py','scripts/reverse/framework/ItemCatalogOracle.cs')
 $taskPairs=@()
 foreach($taskGroup in ($taskProfiles[0].version_pairs|Group-Object current,value)){
  if(@($taskGroup.Group.enabled|Sort-Object -Unique).Count -ne 1){throw 'Catalog version comparison culture disagreement'}
  $taskPairs+=[ordered]@{current=$taskGroup.Group[0].current;value=$taskGroup.Group[0].value;enabled=$taskGroup.Group[0].enabled}
 }
 $taskPairsJson=($taskPairs|ConvertTo-Json -Depth 4).Replace(([string][char]13+[char]10),([string][char]10))+[char]10
 [IO.File]::WriteAllText((Join-Path $PWD 'src/domain/feature_catalog_version_pairs.json'),$taskPairsJson,[Text.UTF8Encoding]::new($false))
 $taskFixture=[ordered]@{profiles=$taskProfiles;sources=@($taskSources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});adaptations=@('Initialization slice through Storage remap, IsValid, exact lookup and last-wins helpers recovered from source','Phase and root-feature trace instrumentation only; actual IsEnable body retained with preloaded profile subset','GetItemStat replaced by null-returning shim; full row property conversion, stats and rate/remaining-table initialization unported','Synthetic DTO projection and read-only SELECT-* observed input order; source runtime historical order not observed','Actual dataset output contains only counts/digest; no original services or VM executed and no persistent writes');source_services_executed=$false;database_writes=$false}
 $taskJson=($taskFixture|ConvertTo-Json -Depth 18).Replace(([string][char]13+[char]10),([string][char]10))+[char]10
 [IO.File]::WriteAllText((Join-Path $PWD 'tests/fixtures/framework-item-catalog.json'),$taskJson,[Text.UTF8Encoding]::new($false))
 Write-Output ('Catalog oracle PASS; architectures=2; cases/architecture='+$taskProfiles[0].cases.Count+'; additional_version_pairs='+$taskPairs.Count+'; actual_projection_equal=true')
} finally {Pop-Location}
