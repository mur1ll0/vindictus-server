$ErrorActionPreference='Stop'
$root=$PWD.Path
$stat='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore.ItemInstance/ItemStat.cs'
$enhance='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore.ItemInstance.Attribute/Enhance.cs'
$utility='legacy/decompiled/Utility-37359981d509/Utility/DictionaryExtension.cs'
$template='scripts/reverse/framework/EnhanceStatOracle.cs.in'
function Extract([string]$path,[string]$startMarker,[string]$endMarker){
 $s=[IO.File]::ReadAllText((Join-Path $root $path));$start=$s.IndexOf($startMarker);if($start-lt0){throw 'Source start changed'};if($endMarker-eq''){return $s.Substring($start)};$end=$s.IndexOf($endMarker,$start);if($end-le$start){throw 'Source end changed'};return $s.Substring($start,$end-$start)
}
$code=[IO.File]::ReadAllText((Join-Path $root $template)).Replace('__STAT__',(Extract $stat 'public class ItemStat' '')).Replace('__ENHANCE__',(Extract $enhance 'public class Enhance' '')).Replace('__UTILITY__',(Extract $utility 'public static bool AddOrIncrease<T>' 'public static HashSet<V> TryGetValue<T, V>'))
New-Item -ItemType Directory -Force .cache/framework,.cache/verification|Out-Null
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/EnhanceStatOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/optimize+','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/EnhanceStatOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')};$taskArgs+='.cache/framework/EnhanceStatOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs *> ('.cache/verification/enhance-stat-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Enhance stat oracle build failed; logs local'}
 & (Join-Path $root ('.cache/framework/EnhanceStatOracle-'+$arch+'.exe')) *> ('.cache/verification/enhance-stat-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Enhance stat oracle failed; logs local'}
}
$x=Get-Content .cache/verification/enhance-stat-x86.json -Raw|ConvertFrom-Json -DateKind String
$y=Get-Content .cache/verification/enhance-stat-x64.json -Raw|ConvertFrom-Json -DateKind String
$differences=0;for($i=0;$i-lt$x.grid.Count;$i++){for($j=0;$j-lt$x.grid[$i].results.Count;$j++){if($x.grid[$i].results[$j]-ne$y.grid[$i].results[$j]){$differences++}}}
$sources=@($stat,$enhance,$utility,$template,'scripts/reverse/generate-enhance-stat-fixtures.ps1')
[ordered]@{profiles=@('Framework48-x86','Framework48-x64');scope='Enhance.TryLoad/UpdateBonusStat over ItemStat with synthetic EnhanceInfo/EnhanceStatInfo rows and gear EnhanceOther;not combined parts';x86=$x;x64=$y;grid_architecture_differences=$differences;original_services_executed=$false;sources=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}})}|ConvertTo-Json -Depth 30 -Compress|Set-Content tests/fixtures/framework-enhance-stat.json -Encoding utf8NoBOM
Write-Output ('Enhance stat oracle: '+$x.cases.Count+' cases and '+($x.grid.Count*$x.grid[0].results.Count)+' grid products per architecture; '+$differences+' x86/x64 grid differences.')
