$ErrorActionPreference='Stop'
$root=$PWD.Path
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source method missing'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}
$manager='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/SkillManager.cs'
$effect='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/SkillEffect.cs'
$template='scripts/reverse/framework/SkillCacheOracle.cs.in'
$code=[IO.File]::ReadAllText((Join-Path $root $template)).Replace('__UPDATE__',(Method $effect 'public void UpdateSkillArgument(string')).Replace('__ARG__',(Method $effect 'private int Arg(string skillID, string argKey, int')).Replace('__LOAD__',(Method $manager 'private void LoadBaseSkillRank(string')).Replace('__RANK__',(Method $manager 'public void OnSkillRankChanged(string'))
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/SkillCacheOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/SkillCacheOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')};$args+='.cache/framework/SkillCacheOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/skill-cache-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Skill cache oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/SkillCacheOracle-'+$arch+'.exe')) *> ('.cache/verification/skill-cache-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Skill cache oracle failed;local log'}
}
$x=Get-Content .cache/verification/skill-cache-x86.json -Raw;$y=Get-Content .cache/verification/skill-cache-x64.json -Raw;if($x-cne$y){throw 'Skill cache architecture difference'}
$sources=@($manager;$effect;$template;'scripts/reverse/generate-skill-cache-fixtures.ps1')
$hashes=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$result=[ordered]@{source_sha256=$hashes;profiles=@('Framework48-x86','Framework48-x64');scope='original LoadBaseSkillRank/OnSkillRankChanged/UpdateSkillArgument/Arg;final-rank/provider and AP/guild/list/actor dependencies instrumented;no stat/multicast/network runtime';cases=($x|ConvertFrom-Json -DateKind String)}|ConvertTo-Json -Depth 12 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-skill-cache.json'),$result,[Text.UTF8Encoding]::new($false))
Write-Output 'Skill cache:120 synthetic cases per architecture;identical;provider/world effects instrumented.'
