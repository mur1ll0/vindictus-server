$ErrorActionPreference='Stop'
$root=$PWD.Path
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source method missing'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}
$titles='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/TitleManager.cs'
$feature='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore/FeatureMatrix.cs'
$stats='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.CharacterServiceOperations/CharacterStats.cs'
$enum='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.CharacterServiceOperations/Stats.cs'
$template='scripts/reverse/framework/InitialTitlesOracle.cs.in'
$code=[IO.File]::ReadAllText((Join-Path $root $template)).Replace('__INTEGER__',(Method $feature 'public static int GetInteger(')).Replace('__TITLES__',(Method $titles 'public CharacterStats GetTitleStats()'))
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/InitialTitlesOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/InitialTitlesOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')}
 $args+=@('.cache/framework/InitialTitlesOracle.cs',$stats,$enum)
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/initial-titles-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Initial titles oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/InitialTitlesOracle-'+$arch+'.exe')) *> ('.cache/verification/initial-titles-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Initial titles oracle failed;local log'}
}
$x=Get-Content .cache/verification/initial-titles-x86.json -Raw|ConvertFrom-Json -DateKind String
$y=Get-Content .cache/verification/initial-titles-x64.json -Raw|ConvertFrom-Json -DateKind String
$sources=@($titles;$feature;$stats;$enum;$template;'scripts/reverse/generate-initial-titles-fixtures.ps1')
$hashes=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$result=[ordered]@{source_sha256=$hashes;scope='original GetTitleStats and FeatureMatrix.GetInteger;empty account collection and unacquired own titles only;provider counterfactuals instrumented';x86=$x;x64=$y}|ConvertTo-Json -Depth 15 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-initial-titles.json'),$result,[Text.UTF8Encoding]::new($false))
Write-Output 'Initial titles oracle:72 cases and2 provider counterfactuals per architecture.'
