$ErrorActionPreference='Stop'
$root=$PWD.Path
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source method missing'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}

$base='legacy/decompiled/PlayerService-f23d8b9df77c/'
$status=$base+'PlayerService.CharacterServiceCore/StatusEffectManager.cs'
$fire=$base+'PlayerService.CharacterServiceCore/TownCampfireManager.cs'
$player=$base+'PlayerService/PlayerEntity.cs'
$loader=$base+'PlayerService.CharacterServiceCore.Functions/CharacterLoader.cs'
$service=$base+'PlayerService/PlayerService.cs'
$character=$base+'PlayerService.CharacterServiceCore/CharacterEntity.cs'
$stats='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.CharacterServiceOperations/CharacterStats.cs'
$enum='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.CharacterServiceOperations/Stats.cs'
$aspect='legacy/decompiled/Utility-37359981d509/Utility/ProfileThisMethod.cs'
$scope='legacy/decompiled/Utility-37359981d509/Utility/ProfileScope.cs'
$data='legacy/decompiled/Utility-37359981d509/Utility/ProfileScopeData.cs'
$boundary='legacy/decompiled/PostSharp-9e55c1c3978c/PostSharp.Aspects/OnMethodBoundaryAspect.cs'
$template='scripts/reverse/framework/InitialStatusOracle.cs.in'
# Source order proof, separate from the synthetic oracle context.
$join=Method $loader 'public bool JoinWithCharacter('
if($join.IndexOf('LoadCharacter(')-lt0-or$join.IndexOf('LoadCharacter(')-ge$join.IndexOf('.LoadPlayer(')){throw 'Join ordering changed'}
$make=Method $service 'protected override IEntity MakeEntity('
if(-not$make.Contains('new PlayerEntity(this, entity)')){throw 'New player allocation changed'}
$code=[IO.File]::ReadAllText((Join-Path $root $template))
$parts=@{
 '__STATUS__'=(Method $status 'public StatusEffectManager(CharacterEntity entity)').Replace('DateTime.UtcNow','Events.UtcNow')
 '__SAVE__'=(Method $status 'public bool Save()')
 '__ELEMENTS__'=(Method $status 'public List<StatusEffectElement> GetStatusEffectElementList()').Replace('DateTime.UtcNow','Events.UtcNow')
 '__CAMPFIRE__'=(Method $fire 'public TownCampfireManager(CharacterEntity entity)')
 '__PLAYER__'=(Method $player 'public PlayerEntity(Service service, IEntity entity)')
 '__ENTRY__'=(Method $aspect 'public sealed override void OnEntry(')
 '__EXIT__'=(Method $aspect 'public sealed override void OnExit(')
 '__SUCCESS__'=(Method $boundary 'public virtual void OnSuccess(')
 '__EXCEPTION__'=(Method $boundary 'public virtual void OnException(')
 '__STATICSTATS__'=(Method $stats 'static CharacterStats()')
 '__STATS__'=(Method $stats 'public CharacterStats()')
 '__ENUM__'=(Method $enum 'public enum Stats')
}
foreach($key in $parts.Keys){$code=$code.Replace($key,$parts[$key])}
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/InitialStatusOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/InitialStatusOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')}
 $args+=@('.cache/framework/InitialStatusOracle.cs',$scope,$data)
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/initial-status-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Status oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/InitialStatusOracle-'+$arch+'.exe')) *> ('.cache/verification/initial-status-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Status oracle failed;local log'}
}
$x=Get-Content .cache/verification/initial-status-x86.json -Raw|ConvertFrom-Json -DateKind String
$y=Get-Content .cache/verification/initial-status-x64.json -Raw|ConvertFrom-Json -DateKind String
if(($x|ConvertTo-Json -Depth 15 -Compress)-cne($y|ConvertTo-Json -Depth 15 -Compress)){throw 'Architecture mismatch'}
$sources=@($status,$fire,$player,$loader,$service,$character,$stats,$enum,$aspect,$scope,$data,$boundary,($base+'PostSharp.ImplementationDetails_95bd3fe7/--z__a_6.cs'),$template,'scripts/reverse/generate-initial-status-fixtures.ps1')
$hashes=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$result=[ordered]@{source_sha256=$hashes;scope='Full original empty StatusEffect constructor/Save/GetStatusEffectElementList,TownCampfire,Player constructor,CharacterStats initialization,profiling hooks and ProfileScope/Data;UTC instrumented;nonempty effects trapped;three connected Save probes are outside native profile;synthetic profiling tag,not recovered original tag;Join ordering checked separately';x86=$x;x64=$y}|ConvertTo-Json -Depth 18 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-initial-status.json'),$result,[Text.UTF8Encoding]::new($false))
Write-Output 'Status oracle:32 cases and3 connected probes per architecture;x86=x64.'
