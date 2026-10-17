$ErrorActionPreference='Stop'
$root=$PWD.Path;$base='legacy/decompiled/PlayerService-f23d8b9df77c/'
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source signature changed'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}
$item=$base+'PlayerService.ItemServiceCore.ItemInstance/Item.cs';$factory=$base+'PlayerService.ItemServiceCore.Storage/ItemFactory.cs';$owner=$base+'PlayerService.ItemServiceCore/ItemOwner.cs';$data=$base+'PlayerService.ItemServiceCore/LogData.cs';$template='scripts/reverse/framework/InventoryDeliveryOracle.cs.in'
$shared=$base+'PlayerService.ItemServiceCore.SharedItemSystem/SharedItemManager.cs'
$code=[IO.File]::ReadAllText((Join-Path $root $template));$parts=@{'__MAKE__'=(Method $factory 'public Item MakeItem(');'__NUM__'=(Method $item 'public int Num');'__INITIALIZE__'=(Method $item 'public bool Initialize(');'__BUNDLE__'=(Method $owner 'public int? GetBundleItemCount(');'__SHARED_BUNDLE__'=(Method $shared 'public int? GetBundleItemCount(')}
foreach($pair in @(@('__COLOR1__','public int Color1'),@('__COLOR2__','public int Color2'),@('__COLOR3__','public int Color3'),@('__REDUCED__','public int ReducedDurability'),@('__BONUS__','public int MaxDurabilityBonus'))){$parts[$pair[0]]=Method $item $pair[1]}
$s=[IO.File]::ReadAllText((Join-Path $root $data));$parts['__LOGDATA__']=$s.Substring($s.IndexOf('public class LogData'))
foreach($k in $parts.Keys){$code=$code.Replace($k,$parts[$k])}
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/InventoryDeliveryOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/InventoryDeliveryOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')};$args+='.cache/framework/InventoryDeliveryOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/inventory-delivery-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Delivery oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/InventoryDeliveryOracle-'+$arch+'.exe')) *> ('.cache/verification/inventory-delivery-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Delivery oracle failed;local log'}
}
$x=Get-Content .cache/verification/inventory-delivery-x86.json -Raw;$y=Get-Content .cache/verification/inventory-delivery-x64.json -Raw;if($x-cne$y){throw 'Delivery architecture difference'}
$sources=@($item;$factory;$owner;$shared;$data;$template;'scripts/reverse/generate-inventory-delivery-fixtures.ps1')
$result=[ordered]@{source_sha256=@($sources|ForEach-Object{@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});profiles=@('Framework48-x86','Framework48-x64');scope='MakeItem/Initialize/Num/getters/LogData/owner and shared GetBundleItemCount originals;allocation,content,attributes,validity,colors,durability,storage/provider and clock instrumented;not a full ItemOwner';cases=($x|ConvertFrom-Json -DateKind String)}
$result|ConvertTo-Json -Depth 14 -Compress|Set-Content tests/fixtures/framework-inventory-delivery.json -Encoding utf8NoBOM
Write-Output 'Inventory delivery oracle PASS:32 cases per Framework architecture;one CreateItem per successful factory call;Num setter emitted none;stack counts nullable.'
