$ErrorActionPreference='Stop'
$root=$PWD.Path
function Extract([string]$file,[string]$begin,[string]$end){$s=[IO.File]::ReadAllText((Join-Path $root $file));$a=$s.IndexOf($begin);$b=$s.IndexOf($end,$a);if($a-lt0-or$b-le$a){throw 'Original allocator boundaries changed'};return $s.Substring($a,$b-$a)}
$service='legacy/decompiled/UnifiedNetwork-70191dde69d5/UnifiedNetwork.Entity/Service.cs'
$maker='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore.Functions/CharacterMaker.cs'
$extension='legacy/decompiled/Utility-37359981d509/Utility/Extention.cs'
$cid=(Extract $service 'public DateTime MakeEntityID()' 'public IEntityProxy Connect(').Replace('DateTime.UtcNow','Clock.Next()').Replace('base.ID','serviceID')
$sn=(Extract $maker 'private int MakeCharacterSN(out DateTime dt, out TimeSpan ts)' 'internal int QueryCharacterCount(').Replace('private int MakeCharacterSN','public int MakeCharacterSN').Replace('DateTime.UtcNow','Clock.Next()')
$epoch=Extract $extension 'public static int ToInt32(this DateTime value)' 'public static DateTime ToDateTime(this int value)'
$code=[IO.File]::ReadAllText((Join-Path $root 'scripts/reverse/framework/CharacterAllocatorOracle.cs.in')).Replace('__EPOCH_METHOD__',$epoch).Replace('__ENTITY_METHODS__',$cid).Replace('__SERIAL_METHODS__',$sn)
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/CharacterAllocatorOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/CharacterAllocatorOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')}
 $args+='.cache/framework/CharacterAllocatorOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/character-allocator-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Allocator harness build failed; log local'}
 if(Select-String ('.cache/verification/character-allocator-build-'+$arch+'.log') -Pattern 'warning [A-Z]+[0-9]+:'){throw 'Allocator compiler warnings'}
 & (Join-Path $root ('.cache/framework/CharacterAllocatorOracle-'+$arch+'.exe')) *> ('.cache/verification/character-allocator-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Allocator oracle failed; log local'}
}
$x=Get-Content .cache/verification/character-allocator-x86.json -Raw|ConvertFrom-Json;$y=Get-Content .cache/verification/character-allocator-x64.json -Raw|ConvertFrom-Json
foreach($key in 'cid','sn','seconds'){if(($x.$key|ConvertTo-Json -Depth 8 -Compress)-cne($y.$key|ConvertTo-Json -Depth 8 -Compress)){throw 'Allocator architecture disagreement'}}
[ordered]@{cid_cases=$x.cid.Count;sn_cases=$x.sn.Count;seconds_cases=$x.seconds.Count;architectures=2;disagreements=0;original_services_executed=$false}|ConvertTo-Json -Compress
if($x.cid.Count-ne252-or$x.sn.Count-ne560-or$x.seconds.Count-ne39){throw 'Allocator case counts differ'}
[ordered]@{profiles=@($x,$y);sources=@(($service,$maker,$extension,'scripts/reverse/framework/CharacterAllocatorOracle.cs.in','scripts/reverse/generate-character-allocator-fixtures.ps1')|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});original_services_executed=$false;scope='Allocator arithmetic only; no database collision/ownership/create success'}|ConvertTo-Json -Depth 12 -Compress|Set-Content tests/fixtures/framework-character-allocator.json -Encoding utf8NoBOM
