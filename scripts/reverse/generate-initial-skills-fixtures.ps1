$ErrorActionPreference='Stop'
$root=$PWD.Path
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source method missing'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}
$manager='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/SkillManager.cs'
$extension='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/CharacterContentsExtension.cs'
$template='scripts/reverse/framework/InitialSkillsOracle.cs.in'
$code=[IO.File]::ReadAllText((Join-Path $root $template)).Replace('__AVAILABLE__',(Method $extension 'public static bool IsAvailableClass(this SkillInfo')).Replace('__CONSTRAINT__',(Method $manager 'public string ConstraintPassed(SkillRankInfo')).Replace('__RECALCULATE__',(Method $manager 'private void RecalculateRequiredAP(')).Replace('__GIVE__',(Method $manager 'public string GiveSkill(string skillID, int rank, Action'))
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/InitialSkillsOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/InitialSkillsOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')};$args+='.cache/framework/InitialSkillsOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/initial-skills-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Initial skill oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/InitialSkillsOracle-'+$arch+'.exe')) *> ('.cache/verification/initial-skills-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Initial skill oracle failed;local log'}
}
$x=Get-Content .cache/verification/initial-skills-x86.json -Raw;$y=Get-Content .cache/verification/initial-skills-x64.json -Raw;if($x-cne$y){throw 'Initial skill architecture difference'}
$sources=@($manager;$extension;$template;'scripts/reverse/generate-initial-skills-fixtures.ps1')
$hashes=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$result=[ordered]@{source_sha256=$hashes;profiles=@('Framework48-x86','Framework48-x64');scope='GiveSkill/ConstraintPassed/RecalculateRequiredAP only;allocator,logging,SP,story,callbacks instrumented;not an entity runtime';cases=($x|ConvertFrom-Json -DateKind String)}|ConvertTo-Json -Depth 10 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-initial-skills.json'),$result,[Text.UTF8Encoding]::new($false))
Write-Output 'Initial skills:200 synthetic cases per architecture;identical;instrumented effects explicitly scoped.'
