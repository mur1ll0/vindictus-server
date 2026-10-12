$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try {
 New-Item -ItemType Directory -Force .cache/framework,.cache/verification | Out-Null
 py -3.13 scripts/reverse/extract-item-argument.py
 if($LASTEXITCODE -ne 0){throw 'Temporal predicate extraction failed'}
 $taskProfiles=@()
 foreach($taskArch in 'x86','x64'){
  $taskExe='.cache/framework/ItemArgument-'+$taskArch+'.exe'
  $taskArgs=@('C:\Program Files\dotnet\sdk\9.0.301\Roslyn\bincore\csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
  foreach($taskLib in 'mscorlib','System','System.Core','System.Data','System.Web.Extensions'){
   $taskArgs+=('/r:C:\Program Files (x86)\Reference Assemblies\Microsoft\Framework\.NETFramework\v4.8\'+$taskLib+'.dll')
  }
  $taskArgs+=@('.cache/framework/ItemArgumentRecovered.cs','scripts/reverse/framework/ItemArgumentOracle.cs')
  & 'C:\Program Files\dotnet\dotnet.exe' @taskArgs *> .cache/verification/item-argument-oracle-build.log
  if($LASTEXITCODE -ne 0){Get-Content .cache/verification/item-argument-oracle-build.log -Tail 6;throw 'Temporal oracle build failed'}
  $taskRaw=& (Join-Path $PWD $taskExe) 2> .cache/verification/item-argument-oracle-error.log
  if($LASTEXITCODE -ne 0){throw 'Temporal oracle failed'}
  $taskData=$taskRaw|ConvertFrom-Json -DateKind String
  if($taskData.architecture -ne $taskArch){throw 'Temporal architecture mismatch'}
  $taskProfiles+=$taskData
 }
 if(($taskProfiles[0].profiles|ConvertTo-Json -Depth 14 -Compress) -ne ($taskProfiles[1].profiles|ConvertTo-Json -Depth 14 -Compress)){throw 'Parser architecture disagreement'}
 $taskSources=@('legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.ItemServiceOperations/ItemClassExBuilder.cs','legacy/decompiled/Utility-37359981d509/Utility/Extention.cs','scripts/reverse/extract-item-argument.py','scripts/reverse/framework/ItemArgumentOracle.cs')
 $taskFixture=[ordered]@{profiles=$taskProfiles;sources=@($taskSources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});adaptations=@('DateTime.TryParse and String.EndsWith observed by delegates; Rust tests inject observations, not a migrated date grammar','DateTime.Now replaced by per-case local clock; ToUniversalTime replaced by explicit custom zone conversion','Injected dates exercise all DateTimeKind values, ticks boundaries, signed truncation and unchecked int32 wrapping','Invalid Argument logger is a no-op; no source argument logging in production');source_services_executed=$false;database_writes=$false}
 $taskJson=($taskFixture|ConvertTo-Json -Depth 18).Replace(([string][char]13+[char]10),([string][char]10))+[char]10
 [IO.File]::WriteAllText((Join-Path $PWD 'tests/fixtures/framework-item-argument.json'),$taskJson,[Text.UTF8Encoding]::new($false))
 Write-Output ('Temporal oracle PASS; architectures=2; culture-zone profiles/architecture='+$taskProfiles[0].profiles.Count+'; cases/profile='+$taskProfiles[0].profiles[0].cases.Count)

} finally {Pop-Location}