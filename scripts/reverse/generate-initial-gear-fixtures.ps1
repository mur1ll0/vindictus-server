$ErrorActionPreference='Stop'
$root=$PWD.Path
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source method missing'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}


$gear='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/Gear.cs'
$effect='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/SkillEffect.cs'
$slot='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/EquippableSlot.cs'
$stats='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.CharacterServiceOperations/CharacterStats.cs'
$enum='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.CharacterServiceOperations/Stats.cs'
$equipment='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.CharacterServiceOperations/EquippedItemInfo.cs'
$template='scripts/reverse/framework/InitialGearOracle.cs.in'
$code=[IO.File]::ReadAllText((Join-Path $root $template))
$code=$code.Replace('__ITEM__',(Method $effect 'public void ApplyItemSkills('))
foreach($p in @(@('__SLOT__',$slot,'public IEnumerable<EquippedItemInfo> ToEquippedItemInfo()'),@('__AVATAR__',$gear,'public int GetAvatarPart('),@('__GEAR__',$gear,'public List<EquippedItemInfo> ToEquippedItemInfos()'),@('__SET__',$gear,'public SetBonusInfo ToSetBonusInfo()'))){$code=$code.Replace($p[0],(Method $p[1] $p[2]))}
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/InitialGearOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/InitialGearOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')}
 $args+=@('.cache/framework/InitialGearOracle.cs',$stats,$enum,$equipment)
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/initial-gear-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Initial Gear oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/InitialGearOracle-'+$arch+'.exe')) *> ('.cache/verification/initial-gear-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Initial Gear oracle failed;local log'}
}
$x=Get-Content .cache/verification/initial-gear-x86.json -Raw|ConvertFrom-Json -DateKind String
$y=Get-Content .cache/verification/initial-gear-x64.json -Raw|ConvertFrom-Json -DateKind String
$sources=@($effect;$gear;$slot;$stats;$enum;$equipment;$template;'scripts/reverse/generate-initial-gear-fixtures.ps1')
$hashes=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$result=[ordered]@{source_sha256=$hashes;scope='original empty-slot ToEquippedItemInfo,GetAvatarPart,ToEquippedItemInfos and ToSetBonusInfo and ApplyItemSkills;empty owner/metadata grouping/feature providers instrumented;not nonempty equipment,Gear loader,SQL or native actor';x86=$x;x64=$y}|ConvertTo-Json -Depth 18 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-initial-gear.json'),$result,[Text.UTF8Encoding]::new($false))
Write-Output 'Initial Gear oracle:160 synthetic cases per architecture;slot order,per-slot dedup,consumable selector,placeholder defaults and empty set bonus.'
