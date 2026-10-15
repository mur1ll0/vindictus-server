$ErrorActionPreference='Stop'
$root=$PWD.Path
$stat='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore.ItemInstance/ItemStat.cs'
$extension='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/ItemContentsExtension.cs'
$utility='legacy/decompiled/Utility-37359981d509/Utility/DictionaryExtension.cs'
function Extract([string]$path,[string]$startMarker,[string]$endMarker){
 $s=[IO.File]::ReadAllText((Join-Path $root $path));$start=$s.IndexOf($startMarker);if($start-lt0){throw 'Method start changed'};if($endMarker-eq''){return $s.Substring($start)};$end=$s.IndexOf($endMarker,$start);if($end-le$start){throw 'Method end changed'};return $s.Substring($start,$end-$start)
}
$body=Extract $stat 'public class ItemStat' ''
$equip=(Extract $extension 'public static ItemStat GetItemStat(' '').TrimEnd();if(-not$equip.EndsWith('}')){throw 'Extension structure changed'};$equip=$equip.Substring(0,$equip.Length-1)
$dict=Extract $utility 'public static bool AddOrIncrease<T>' 'public static HashSet<V> TryGetValue<T, V>'
$code=[IO.File]::ReadAllText((Join-Path $root 'scripts/reverse/framework/ItemStatsOracle.cs.in')).Replace('__STAT__',$body).Replace('__EQUIP__',$equip).Replace('__UTILITY__',$dict)
New-Item -ItemType Directory -Force .cache/framework,.cache/verification|Out-Null
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/ItemStatsOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/ItemStatsOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')};$taskArgs+='.cache/framework/ItemStatsOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs *> ('.cache/verification/item-stats-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Item stats oracle build failed; logs local'}
 & (Join-Path $root ('.cache/framework/ItemStatsOracle-'+$arch+'.exe')) *> ('.cache/verification/item-stats-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Item stats oracle failed; logs local'}
}
$x=Get-Content .cache/verification/item-stats-x86.json -Raw;$y=Get-Content .cache/verification/item-stats-x64.json -Raw
if($x-cne$y){throw 'Measured architecture difference requires explicit profiles'}
$cases=$x|ConvertFrom-Json -DateKind String
[ordered]@{profiles=@('Framework48-x86','Framework48-x64');cases=$cases;original_services_executed=$false;sources=@(($stat,$extension,$utility,'legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.HeroesContents/EquipItemInfo.cs','scripts/reverse/framework/ItemStatsOracle.cs.in','scripts/reverse/generate-item-stats-fixtures.ps1')|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}})}|ConvertTo-Json -Depth 30 -Compress|Set-Content tests/fixtures/framework-item-stats.json -Encoding utf8NoBOM
Write-Output ('Item stats oracle: '+$cases.weights.Count+' weights and '+$cases.batches.Count+' batches/'+($cases.batches.Count*18)+' operations per architecture; no differences.')
