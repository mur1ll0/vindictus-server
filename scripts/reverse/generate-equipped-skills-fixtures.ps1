$ErrorActionPreference='Stop'
$root=$PWD.Path
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source method missing'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}

$effect='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/SkillEffect.cs'
$linear='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/LinearFormulaExtension.cs'
$stats='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.CharacterServiceOperations/CharacterStats.cs'
$enum='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.CharacterServiceOperations/Stats.cs'
$equipment='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.CharacterServiceOperations/EquippedItemInfo.cs'
$template='scripts/reverse/framework/EquippedSkillsOracle.cs.in'
$code=[IO.File]::ReadAllText((Join-Path $root $template))
$code=$code.Replace('__ITEM__',(Method $effect 'public void ApplyItemSkills(')).Replace('__LINEAR__',(Method $linear 'public static int Linear('))
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/EquippedSkillsOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/EquippedSkillsOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')}
 $args+=@('.cache/framework/EquippedSkillsOracle.cs',$stats,$enum,$equipment)
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/equipped-skills-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Equipment oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/EquippedSkillsOracle-'+$arch+'.exe')) *> ('.cache/verification/equipped-skills-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Equipment oracle failed;local log'}
}
$x=Get-Content .cache/verification/equipped-skills-x86.json -Raw|ConvertFrom-Json -DateKind String
$y=Get-Content .cache/verification/equipped-skills-x64.json -Raw|ConvertFrom-Json -DateKind String
$sources=@($effect;$linear;$stats;$enum;$equipment;$template;'scripts/reverse/generate-equipped-skills-fixtures.ps1')
$hashes=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$result=[ordered]@{source_sha256=$hashes;scope='original ApplyItemSkills and Int32 Linear;CharacterStats/Stats/EquippedItemInfo compiled unchanged;Arg and lazy category provider instrumented;ASCII finite input profile;Framework48-x64 target,not loader/SQL/native actor';x86=$x;x64=$y}|ConvertTo-Json -Depth 15 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-equipped-skills.json'),$result,[Text.UTF8Encoding]::new($false))
Write-Output 'Equipment oracle:640 synthetic cases per architecture;22 mastery classes,avatar guards,overflow,duplicate/category failures.'
