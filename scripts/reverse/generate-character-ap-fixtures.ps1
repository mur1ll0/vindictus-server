$ErrorActionPreference='Stop'
$root=$PWD.Path
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source method missing'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}
$manager='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/APManager.cs'
$effect='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/SkillEffect.cs'
$template='scripts/reverse/framework/CharacterAPOracle.cs.in'
$code=[IO.File]::ReadAllText((Join-Path $root $template)).Replace('__PREFIX__',(Method $effect 'public int CalculateAPTime(int'))
foreach($p in @(@('__UPDATE__','public void UpdateAP()'),@('__RECALCULATE__','public void CalculateBonusAPTime()'),@('__USE__','public bool UseAP(int'),@('__GIVE__','public int GiveAP(int'))){$body=(Method $manager $p[1]).Replace('DateTime.UtcNow','Clock.Now');$code=$code.Replace($p[0],$body)}
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/CharacterAPOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/CharacterAPOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')};$args+='.cache/framework/CharacterAPOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/character-ap-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Character AP oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/CharacterAPOracle-'+$arch+'.exe')) *> ('.cache/verification/character-ap-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Character AP oracle failed;local log'}
}
$x=Get-Content .cache/verification/character-ap-x86.json -Raw|ConvertFrom-Json -DateKind String
$y=Get-Content .cache/verification/character-ap-x64.json -Raw|ConvertFrom-Json -DateKind String
$sources=@($manager;$effect;$template;'scripts/reverse/generate-character-ap-fixtures.ps1')
$hashes=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$result=[ordered]@{source_sha256=$hashes;scope='original AP Update/Recalculate/Use/Give;UTC,maximum policy,interval calculator instrumented;CalculateAPTime with float.Parse StatRatio (intervals);not stats,persistence,network or guild';x86=$x.cases;x64=$y.cases;prefix_x86=$x.prefixes;prefix_x64=$y.prefixes;intervals_x86=$x.intervals;intervals_x64=$y.intervals}|ConvertTo-Json -Depth 10 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-character-ap.json'),$result,[Text.UTF8Encoding]::new($false))
Write-Output 'Character AP:448 cases per architecture;separate results retained.'
