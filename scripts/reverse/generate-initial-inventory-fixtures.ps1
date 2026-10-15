$ErrorActionPreference='Stop'
$root=$PWD.Path;$p='legacy/decompiled/PlayerService-f23d8b9df77c/'
$item=$p+'PlayerService.ItemServiceCore.ItemInstance/Item.cs'
$features='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore/FeatureMatrix.cs'
function Slice([string]$path,[string]$a,[string]$b=''){
 $s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($a);if($i-lt0){throw 'Source marker changed'};if($b-eq''){return $s.Substring($i)};$j=$s.IndexOf($b,$i);if($j-le$i){throw 'Source end changed'};return $s.Substring($i,$j-$i)
}
function Method([string]$path,[string]$signature){
 $s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Method signature changed'};$brace=$s.IndexOf('{',$i);$depth=1;$end=$brace+1
 while($depth-gt0-and$end-lt$s.Length){if($s[$end]-eq'{'){$depth++};if($s[$end]-eq'}'){$depth--};$end++}
 if($depth-ne0){throw 'Method braces changed'};return $s.Substring($i,$end-$i)
}
$replacements=@{}
$replacements['__CRC__']=Slice 'legacy/decompiled/Utility-37359981d509/Utility/Crc32.cs' 'public class Crc32'
$replacements['__CHECKER__']=Slice ($p+'PlayerService.ItemServiceCore.CheckSum/Checker.cs') 'internal class Checker'
$replacements['__COLOR__']=Slice ($p+'PlayerService.ItemServiceCore.Static/Color.cs') 'internal struct Color'
$replacements['__MATERIAL__']=Slice ($p+'PlayerService.ItemServiceCore.Static/Material.cs') 'internal class Material'
$t=(Slice ($p+'PlayerService.ItemServiceCore.Static/ColorTable.cs') 'public static Material GetMaterial(').TrimEnd();$replacements['__TABLE__']=$t.Substring(0,$t.Length-1)
$replacements['__HAS__']=Slice $item 'public bool HasDurability' 'public bool IsMaxArmorBroken'
$replacements['__DURFIELDS__']=Slice $item 'public int ReducedDurability' 'public int IsCafeOnly'
$replacements['__DURABILITY__']=Slice $item 'public int Durability' 'public bool IsCharacterBinded'
$replacements['__SETCOLOR__']=Method $item 'public void SetColor('
$replacements['__INITIALIZE__']=(Method $item 'public bool Initialize(').Replace('ItemRequestProperty','Property').Replace('DateTime.UtcNow','Clock.UtcNow').Replace('Log<Item>.Logger.WarnFormat','Log<Item>.Logger.WarnFormat')
$valid=Method $item 'public bool IsValid()';$start=$valid.IndexOf('if (ItemClassInfo == null)');$end=$valid.IndexOf('methodExecutionArgs.ReturnValue = result;',$start);$replacements['__VALIDITY__']=$valid.Substring($start,$end-$start)+'return result;'
$replacements['__UNIQUE__']=Slice ($p+'PlayerService.ItemServiceCore/ItemContentsExtension.cs') 'public static int UniqueConstraintLeft(' 'public static bool IsLookChangeItem('
$replacements['__AVAILABLE__']=Method ($p+'PlayerService.ItemServiceCore.Storage/StorageSetting.cs') 'public static IEnumerable<StorageType> GetAvailableStorages('
$helper=$p+'PlayerService.ItemServiceCore.Storage.Component/ItemClassHelper.cs'
$replacements['__BUNDLE__']=Method $helper 'private bool BundleExceed()'
$replacements['__CREATEVIRTUAL__']=(Method $helper 'private int CreateItem(').Replace('ItemRequestProperty','Property')
$cash=$p+'PlayerService.ItemServiceCore.Storage.Component/CashStorage.cs'
$replacements['__CASHEMPTY__']=Slice $cash 'private IEnumerable<ItemLocation> EmptySlots' 'public StorageType Type'
$replacements['__CASHBIND__']=Method $cash 'public void BindVirtualItem('
$bag=$p+'PlayerService.ItemServiceCore.Storage.Component/BagStorage.cs'
$replacements['__BAGEMPTY__']=Method $bag 'private IDictionary<int, ICollection<ItemLocation>> GetEmptySlotsWithDestroyItem('
$replacements['__BAGBIND__']=Method $bag 'public void BindVirtualItem('
$service='legacy/decompiled/UnifiedNetwork-70191dde69d5/UnifiedNetwork.Entity/Service.cs'
$replacements['__ALLOCATOR__']=(Slice $service 'public DateTime MakeEntityID()' 'public IEntityProxy Connect(').Replace('DateTime.UtcNow','Clock.UtcNow')
$replacements['__ISENABLE__']=Method $features 'public static bool IsEnable(string featureName)'
$replacements['__GETINTEGER__']=Method $features 'public static int GetInteger(string featureName)'
$code=[IO.File]::ReadAllText((Join-Path $root 'scripts/reverse/framework/InitialInventoryOracle.cs.in'))
foreach($key in $replacements.Keys){$code=$code.Replace($key,$replacements[$key])}
New-Item -ItemType Directory -Force .cache/framework,.cache/verification|Out-Null
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/InitialInventoryOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $a=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/InitialInventoryOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$a+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')};$a+='.cache/framework/InitialInventoryOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @a *> ('.cache/verification/initial-inventory-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Initial inventory oracle build failed;log local'}
 & (Join-Path $root ('.cache/framework/InitialInventoryOracle-'+$arch+'.exe')) *> ('.cache/verification/initial-inventory-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Initial inventory oracle execution failed;log local'}
}
$x=Get-Content .cache/verification/initial-inventory-x86.json -Raw;$y=Get-Content .cache/verification/initial-inventory-x64.json -Raw
if($x-cne$y){throw 'Architecture difference requires explicit profile'}
$cases=$x|ConvertFrom-Json -DateKind String
$sources=@($item;$features;$helper;$cash;$bag;$service;'legacy/decompiled/Utility-37359981d509/Utility/Crc32.cs';($p+'PlayerService.ItemServiceCore.CheckSum/Checker.cs');($p+'PlayerService.ItemServiceCore/ItemContentsExtension.cs');($p+'PlayerService.ItemServiceCore.Storage/StorageSetting.cs');($p+'PlayerService.ItemServiceCore/ItemClassConst.cs');($p+'PlayerService.ItemServiceCore.Static/Color.cs');($p+'PlayerService.ItemServiceCore.Static/Material.cs');($p+'PlayerService.ItemServiceCore.Static/ColorTable.cs');'legacy/decompiled/Utility-37359981d509/Utility/ProfileThisMethod.cs';'scripts/reverse/framework/InitialInventoryOracle.cs.in';'scripts/reverse/generate-initial-inventory-fixtures.ps1')
[ordered]@{profiles=@('Framework48-x86-en-US','Framework48-x64-en-US');cases=$cases;original_services_executed=$false;sources=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}})}|ConvertTo-Json -Depth 18 -Compress|Set-Content tests/fixtures/framework-initial-inventory.json -Encoding utf8NoBOM
Write-Output ('Initial inventory oracle:validity'+$cases.validity.Count+';checksums'+$cases.checks.Count+';placement plans'+$cases.plans.Count+';zero architecture differences.')
