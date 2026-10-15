$ErrorActionPreference='Stop'
$root=$PWD.Path
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source method missing'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}
$a='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.CharacterServiceOperations/CharacterStats.cs'
$b='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/CharacterContents.cs'
$template='scripts/reverse/framework/CharacterBaseStatsOracle.cs.in'
$code=[IO.File]::ReadAllText((Join-Path $root $template)).Replace('__CTOR__',(Method $a 'public CharacterStats(int s,')).Replace('__ADD__',(Method $a 'public CharacterStats Add(CharacterStats rhs)')).Replace('__GET__',(Method $b 'public static CharacterStats GetCharacterStats('))
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/CharacterBaseStatsOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/CharacterBaseStatsOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')};$args+='.cache/framework/CharacterBaseStatsOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/character-base-stats-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Base stats oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/CharacterBaseStatsOracle-'+$arch+'.exe')) *> ('.cache/verification/character-base-stats-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Base stats oracle execution failed;local log'}
}
$x=Get-Content .cache/verification/character-base-stats-x86.json -Raw;$y=Get-Content .cache/verification/character-base-stats-x64.json -Raw;if($x-cne$y){throw 'Base stats oracle architecture difference'}
$sources=@($a;$b;$template;'scripts/reverse/generate-character-base-stats-fixtures.ps1')
$hashes=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
[ordered]@{source_sha256=$hashes;profiles=@('Framework-x86','Framework-x64');cases=($x|ConvertFrom-Json -DateKind String)}|ConvertTo-Json -Depth 10|Set-Content tests/fixtures/framework-character-base-stats.json -Encoding utf8NoBOM
Write-Output 'Base stats oracle:256 cases per architecture;zero differences;synthetic fields only.'
