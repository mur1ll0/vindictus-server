$ErrorActionPreference='Stop'
$root=$PWD.Path
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source method missing'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}
$effect='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/SkillEffect.cs'
$manager='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/StatManager.cs'
$stats='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.CharacterServiceOperations/CharacterStats.cs'
$enum='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.CharacterServiceOperations/Stats.cs'
$template='scripts/reverse/framework/CharacterStatsOracle.cs.in'
$code=[IO.File]::ReadAllText((Join-Path $root $template))
foreach($p in @(@('__ITEM__',$effect,'public void ApplyItemSkills('),@('__FINAL__',$effect,'public void ApplyStatSkills('),@('__CALCULATE__',$manager,'public void CalculateFinalStat()'))){$code=$code.Replace($p[0],(Method $p[1] $p[2]))}
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/CharacterStatsOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/CharacterStatsOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')}
 $args+=@('.cache/framework/CharacterStatsOracle.cs',$stats,$enum)
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/character-stats-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Stats oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/CharacterStatsOracle-'+$arch+'.exe')) *> ('.cache/verification/character-stats-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Stats oracle failed;local log'}
}
$x=Get-Content .cache/verification/character-stats-x86.json -Raw|ConvertFrom-Json -DateKind String
$y=Get-Content .cache/verification/character-stats-x64.json -Raw|ConvertFrom-Json -DateKind String
$sources=@($manager;$effect;$stats;$enum;$template;'scripts/reverse/generate-character-stats-fixtures.ps1')
$hashes=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$result=[ordered]@{source_sha256=$hashes;scope='original ApplyStatSkills,empty-Gear ApplyItemSkills and CalculateFinalStat;Arg,owner,AP/observer callbacks instrumented;not equipped-item branches or native actor';x86=$x;x64=$y}|ConvertTo-Json -Depth 15 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-character-stats.json'),$result,[Text.UTF8Encoding]::new($false))
Write-Output 'Stats oracle:800 formula,8 empty-Gear,32 callback cases per architecture;separate results retained.'
