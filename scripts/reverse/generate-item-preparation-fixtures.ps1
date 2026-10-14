$ErrorActionPreference='Stop'
$root=$PWD.Path
$factory='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore.Storage/ItemFactory.cs'
$trait='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore.ItemInstance/ItemTraitExtension.cs'
function Extract([string]$path,[string]$startMarker,[string]$endMarker){
 $s=[IO.File]::ReadAllText((Join-Path $root $path));$start=$s.IndexOf($startMarker);if($start-lt0){throw 'Method start changed'};$end=$s.IndexOf($endMarker,$start);if($end-le$start){throw 'Method end changed'};return $s.Substring($start,$end-$start)
}
$methods=Extract $factory 'public List<VirtualCreateItem> MakeVirtualItems(' 'public List<Item> MakeSystemItems('
$stack=Extract $trait 'public static bool IsStackableWith(' 'public static bool Stack('
$code=[IO.File]::ReadAllText((Join-Path $root 'scripts/reverse/framework/ItemPreparationOracle.cs.in')).Replace('__STACK__',$stack).Replace('__FACTORY__',$methods)
New-Item -ItemType Directory -Force .cache/framework,.cache/verification | Out-Null
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/ItemPreparationOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/ItemPreparationOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')};$taskArgs+='.cache/framework/ItemPreparationOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs *> ('.cache/verification/item-preparation-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Item preparation oracle build failed; log local'}
 & (Join-Path $root ('.cache/framework/ItemPreparationOracle-'+$arch+'.exe')) *> ('.cache/verification/item-preparation-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Item preparation oracle failed; log local'}
}
$x=Get-Content .cache/verification/item-preparation-x86.json -Raw
$y=Get-Content .cache/verification/item-preparation-x64.json -Raw
if($x-cne$y){throw 'Measured architecture difference requires an explicit profile'}
$cases=$x|ConvertFrom-Json -DateKind String
[ordered]@{profiles=@('Framework48-x86','Framework48-x64');cases=$cases;original_services_executed=$false;sources=@(($factory,$trait,'scripts/reverse/framework/ItemPreparationOracle.cs.in','scripts/reverse/generate-item-preparation-fixtures.ps1')|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}})}|ConvertTo-Json -Depth 20 -Compress|Set-Content tests/fixtures/framework-item-preparation.json -Encoding utf8NoBOM
Write-Output ('Item preparation oracle: '+$cases.pairs.Count+' stack comparisons; '+$cases.batches.Count+' batches per architecture; no architecture differences; original services not executed.')
