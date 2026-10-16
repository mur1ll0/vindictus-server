$ErrorActionPreference='Stop'
$root=$PWD.Path
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source method missing'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}
$base='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/'
$manager='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore.OperationLogSystem/ItemLogManager.cs'
$template='scripts/reverse/framework/ItemLogsOracle.cs.in'
$code=[IO.File]::ReadAllText((Join-Path $root $template)).Replace('__LEDGER__',(Method ($base+'ItemLog.cs') 'public static void AddItemLedger(')).Replace('__SIMPLE__',(Method ($base+'ItemLog.cs') 'public static void AddSimpleLog('))
# Only time is injected; pure original methods run against synthetic owner/provider stubs.
$managerCode=[IO.File]::ReadAllText((Join-Path $root $manager));if($managerCode-notmatch'time = DateTime.Now;'){throw 'Source clock seam changed'}
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/ItemLogsManager.cs'),$managerCode.Replace('time = DateTime.Now;','time = OracleClock.Now;'))
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/ItemLogsOracle.cs'),$code)
$files=@('.cache/framework/ItemLogsOracle.cs','.cache/framework/ItemLogsManager.cs')
foreach($name in 'LogData','SimpleLogData','ItemLogType','ActionType','EventType','EventTypeDetail'){$files+=($base+$name+'.cs')}
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/ItemLogsOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')};$args+=$files
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/item-logs-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Item log oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/ItemLogsOracle-'+$arch+'.exe')) *> ('.cache/verification/item-logs-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Item log oracle failed;local log'}
}
$x=Get-Content .cache/verification/item-logs-x86.json -Raw;$y=Get-Content .cache/verification/item-logs-x64.json -Raw;if($x-cne$y){throw 'Item log architecture difference'}
$sources=@($manager;($base+'ItemLog.cs');($base+'LogData.cs');($base+'SimpleLogData.cs');($base+'ItemLogType.cs');($base+'ActionType.cs');($base+'EventType.cs');($base+'EventTypeDetail.cs');$template;'scripts/reverse/generate-item-logs-fixtures.ps1')
$hashes=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$result=[ordered]@{source_sha256=$hashes;profiles=@('Framework48-x86','Framework48-x64');scope='LogData,ItemLogManager,AddItemLedger/AddSimpleLog only;clock,owner,provider and feature switches instrumented;not GiveItem/Save/runtime';cases=($x|ConvertFrom-Json -DateKind String)}|ConvertTo-Json -Depth 12 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-item-logs.json'),$result,[Text.UTF8Encoding]::new($false))
Write-Output 'Item logs:64 synthetic cases per Framework architecture;identical;no legacy service executed.'
