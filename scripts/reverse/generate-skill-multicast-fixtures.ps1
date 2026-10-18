$ErrorActionPreference='Stop'
$root=$PWD.Path
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source method missing'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}
$manager='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/SkillManager.cs'
$effect='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/SkillEffect.cs'
$stat='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/StatManager.cs'
$template='scripts/reverse/framework/SkillMulticastOracle.cs.in'
$code=[IO.File]::ReadAllText((Join-Path $root $template))
foreach($p in @(@('__UPDATE__',$effect,'public void UpdateSkillArgument(string'),@('__ARG__',$effect,'private int Arg(string skillID, string argKey, int'),@('__LOAD__',$manager,'private void LoadBaseSkillRank(string'),@('__RANK__',$manager,'public void OnSkillRankChanged(string'),@('__MEDITATION__',$effect,'public static bool IsMeditation('),@('__STAT_RANK__',$stat,'public void OnSkillRankChanged('))){$code=$code.Replace($p[0],(Method $p[1] $p[2]))}
$source=[IO.File]::ReadAllText((Join-Path $root $manager));$start=$source.IndexOf('SkillRankChanged += OnSkillRankChanged;');$end=$source.IndexOf('_003C_003Ez__a_4.a4.OnSuccess',$start)
if($start-lt0-or$end-le$start){throw 'Original multicast subscription boundary changed'}
$code=$code.Replace('__SUBSCRIBE__',$source.Substring($start,$end-$start))
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/SkillMulticastOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/SkillMulticastOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')};$args+='.cache/framework/SkillMulticastOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/skill-multicast-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Multicast oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/SkillMulticastOracle-'+$arch+'.exe')) *> ('.cache/verification/skill-multicast-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Multicast oracle failed;local log'}
}
$x=Get-Content .cache/verification/skill-multicast-x86.json -Raw|ConvertFrom-Json -DateKind String
$y=Get-Content .cache/verification/skill-multicast-x64.json -Raw|ConvertFrom-Json -DateKind String
if($x.Count-ne256-or$y.Count-ne256){throw 'Unexpected oracle cases'}
$paths=@($manager;$effect;$stat;$template;'scripts/reverse/generate-skill-multicast-fixtures.ps1')
$hashes=@($paths|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$r=[ordered]@{source_sha256=$hashes;scope='original multicast subscriptions,manager/stat subscribers,rank/argument cache and IsMeditation;classification,actor,AP,stat calculation,pushes and observables instrumented;not an actor or client test';x86=$x;x64=$y}|ConvertTo-Json -Depth 14 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-skill-multicast.json'),$r,[Text.UTF8Encoding]::new($false))
Write-Output 'Multicast oracle:256 cases per architecture;source subscriber order/cache/failure cutoffs;actor effects instrumented.'
