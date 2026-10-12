$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try {
 New-Item -ItemType Directory -Force .cache/framework,.cache/verification | Out-Null
 py -3.13 scripts/reverse/extract-queue-decisions.py
 if($LASTEXITCODE -ne 0){throw 'Queue predicate extraction failed'}
 $taskProfiles=@()
 foreach($taskArch in 'x86','x64'){
  $taskExe='.cache/framework/QueueDecisions-'+$taskArch+'.exe'
  $taskArgs=@('C:\Program Files\dotnet\sdk\9.0.301\Roslyn\bincore\csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
  foreach($taskLib in 'mscorlib','System','System.Core','System.Data','System.Web.Extensions'){
   $taskArgs+=('/r:C:\Program Files (x86)\Reference Assemblies\Microsoft\Framework\.NETFramework\v4.8\'+$taskLib+'.dll')
  }
  $taskArgs+=@('.cache/framework/QueueDecisionsRecovered.cs','scripts/reverse/framework/QueueDecisionsOracle.cs')
  & 'C:\Program Files\dotnet\dotnet.exe' @taskArgs *> .cache/verification/queue-decisions-oracle-build.log
  if($LASTEXITCODE -ne 0){Get-Content .cache/verification/queue-decisions-oracle-build.log -Tail 6;throw 'Queue oracle build failed'}
  $taskRaw=& (Join-Path $PWD $taskExe) 2> .cache/verification/queue-decisions-oracle-error.log
  if($LASTEXITCODE -ne 0){throw 'Queue oracle failed'}
  $taskData=$taskRaw|ConvertFrom-Json -DateKind String
  if($taskData.architecture -ne $taskArch){throw 'Queue architecture mismatch'}
  $taskProfiles+=$taskData
 }
 foreach($taskGroup in 'attempts','routes','binds','selection'){
  if(($taskProfiles[0].$taskGroup|ConvertTo-Json -Depth 10 -Compress) -ne ($taskProfiles[1].$taskGroup|ConvertTo-Json -Depth 10 -Compress)){throw 'Queue architecture disagreement'}
 }
 $taskSources=@('legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore.Static/QueuedItemController.cs','legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/DBLoader.cs','scripts/reverse/extract-queue-decisions.py','scripts/reverse/framework/QueueDecisionsOracle.cs')
 $taskFixture=[ordered]@{profiles=$taskProfiles;sources=@($taskSources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});adaptations=@('row body replaced by synthetic attempt/exception counter; recovered outer loop and catch retained','LINQ-to-SQL selection evaluated against in-memory synthetic rows','mail switch and default binder retained; feature lookup replaced by controlled shim','expiry clock explicitly sampled; source conditions mapped to route names; delivery body not executed');source_services_executed=$false;database_writes=$false}
 $taskJson=($taskFixture|ConvertTo-Json -Depth 14).Replace(([string][char]13+[char]10),([string][char]10))+[char]10
 [IO.File]::WriteAllText((Join-Path $PWD 'tests/fixtures/framework-queue-decisions.json'),$taskJson,[Text.UTF8Encoding]::new($false))
 Write-Output ('Queue oracle PASS; profiles=2; attempts='+$taskProfiles[0].attempts.Count+'; routes='+$taskProfiles[0].routes.Count+'; binders='+$taskProfiles[0].binds.Count+'; selections='+$taskProfiles[0].selection.Count)
} finally {Pop-Location}
