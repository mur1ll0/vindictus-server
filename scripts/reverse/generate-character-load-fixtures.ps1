$ErrorActionPreference='Stop'
$root=$PWD.Path
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source method missing'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}
$manager='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/StatManager.cs'
$effect='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/SkillEffect.cs'
$contents='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/CharacterContents.cs'
$code=[IO.File]::ReadAllText((Join-Path $root 'scripts/reverse/framework/CharacterLoadOracle.cs.in'))
foreach($p in @(@('__MEDIUM__',$effect,'public double ApplyMediumLoadSkills('),@('__BASE__',$contents,'public static double GetMediumLoadWeight('),@('__HEAVY__',$contents,'public static double GetHeavyLoadMultiplier('))){$code=$code.Replace($p[0],(Method $p[1] $p[2]))}
$body=(Method $manager 'public void CalculateItemStat(').Replace('CharacterContents.GetMediumLoadWeight(','CharacterContents.MeasuredBase(')
$code=$code.Replace('__CALCULATE__',$body)
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/CharacterLoadOracle.cs'),$code)
foreach($arch in 'x86','x64'){
$args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/CharacterLoadOracle-'+$arch+'.exe'))
foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')};$args+='.cache/framework/CharacterLoadOracle.cs'
& 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/character-load-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Load oracle build failed'}
& (Join-Path $root ('.cache/framework/CharacterLoadOracle-'+$arch+'.exe')) *> ('.cache/verification/character-load-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Load oracle failed'}
}
$x=Get-Content .cache/verification/character-load-x86.json -Raw|ConvertFrom-Json -DateKind String
$y=Get-Content .cache/verification/character-load-x64.json -Raw|ConvertFrom-Json -DateKind String
$paths=@($manager;$effect;$contents;'legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.HeroesContents/CharacterBaseLoadInfo.cs';'scripts/reverse/framework/CharacterLoadOracle.cs.in';'scripts/reverse/generate-character-load-fixtures.ps1')
$hashes=@($paths|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$r=[ordered]@{source_sha256=$hashes;scope='original CalculateItemStat,ApplyMediumLoadSkills,GetMediumLoadWeight and GetHeavyLoadMultiplier;empty item effect provider,owner/base query failures instrumented;not equipped-item effects,SQL,actor or network';x86=$x;x64=$y}|ConvertTo-Json -Depth 12 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-character-load.json'),$r,[Text.UTF8Encoding]::new($false))
Write-Output 'Load oracle:640 synthetic cases per architecture;empty-Gear transitions/base-provider failures;separate results retained.'
