$ErrorActionPreference='Stop'
$root=$PWD.Path
$item='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore.ItemInstance/Item.cs'
function Extract([string]$startMarker,[string]$endMarker){
 $s=[IO.File]::ReadAllText((Join-Path $root $item));$start=$s.IndexOf($startMarker);if($start-lt0){throw 'Source start changed'};$end=$s.IndexOf($endMarker,$start);if($end-le$start){throw 'Source end changed'};return $s.Substring($start,$end-$start)
}
$has=Extract 'public bool HasDurability' 'public bool IsMaxArmorBroken'
$fields=Extract 'public int ReducedDurability' 'public int IsCafeOnly'
$durability=Extract 'public int Durability' 'public bool IsCharacterBinded'
$source=[IO.File]::ReadAllText((Join-Path $root $item))
$end=$source.IndexOf('return IsValid();',$source.IndexOf('public bool Initialize('))
$start=$source.LastIndexOf('if (property.ItemClassInfo.IsEquippable())',$end)
if($start-lt0-or$end-le$start){throw 'Initialize branch changed'}
$initialize=$source.Substring($start,$end-$start)
$code=[IO.File]::ReadAllText((Join-Path $root 'scripts/reverse/framework/ItemDurabilityOracle.cs.in')).Replace('__HAS__',$has).Replace('__FIELDS__',$fields).Replace('__DURABILITY__',$durability).Replace('__INITIALIZE__',$initialize)
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/ItemDurabilityOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/ItemDurabilityOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')};$taskArgs+='.cache/framework/ItemDurabilityOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs *> ('.cache/verification/item-durability-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Durability oracle build failed; logs local'}
 & (Join-Path $root ('.cache/framework/ItemDurabilityOracle-'+$arch+'.exe')) *> ('.cache/verification/item-durability-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Durability oracle failed; logs local'}
}
$x=Get-Content .cache/verification/item-durability-x86.json -Raw;$y=Get-Content .cache/verification/item-durability-x64.json -Raw
if($x-cne$y){throw 'Architecture difference needs explicit profile'}
$cases=$x|ConvertFrom-Json -DateKind String
[ordered]@{profiles=@('Framework48-x86','Framework48-x64');cases=$cases;original_services_executed=$false;sources=@(($item,'scripts/reverse/framework/ItemDurabilityOracle.cs.in','scripts/reverse/generate-item-durability-fixtures.ps1')|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}})}|ConvertTo-Json -Depth 20 -Compress|Set-Content tests/fixtures/framework-item-durability.json -Encoding utf8NoBOM
Write-Output ('Durability oracle: '+$cases.initial.Count+' initializations;'+$cases.batches.Count+' batches/'+($cases.batches.Count*9)+' setters;non-equipment branch;zero architecture differences.')
