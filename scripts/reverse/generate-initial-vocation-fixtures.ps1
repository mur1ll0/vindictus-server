$ErrorActionPreference='Stop'
$root=$PWD.Path
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source method missing'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}
$manager='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore._Vocation_/VocationManager.cs'
$template='scripts/reverse/framework/InitialVocationOracle.cs.in'
$code=[IO.File]::ReadAllText((Join-Path $root $template)).Replace('__CTOR__',(Method $manager 'public VocationManager(CharacterEntity parent)')).Replace('__INIT__',(Method $manager 'private void InitSkillDict()')).Replace('__SUM__',(Method $manager 'private void CalculateUsedSkillPoint()'))
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/InitialVocationOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/InitialVocationOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')}
 $args+='.cache/framework/InitialVocationOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/initial-vocation-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Vocation oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/InitialVocationOracle-'+$arch+'.exe')) *> ('.cache/verification/initial-vocation-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Vocation oracle failed;local log'}
}
$x=Get-Content .cache/verification/initial-vocation-x86.json -Raw|ConvertFrom-Json -DateKind String
$y=Get-Content .cache/verification/initial-vocation-x64.json -Raw|ConvertFrom-Json -DateKind String
$base='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/'
$sources=@($manager;($base+'Character.cs');($base+'CharacterEntity.cs');($base+'Vocation.cs');($base+'VocationSkill.cs');$template;'scripts/reverse/generate-initial-vocation-fixtures.ps1')
$hashes=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$result=[ordered]@{source_sha256=$hashes;scope='original VocationManager constructor,InitSkillDict,CalculateUsedSkillPoint;association,owner and privacy-safe logger instrumented;not skills training or transformation';x86=$x;x64=$y}|ConvertTo-Json -Depth 15 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-initial-vocation.json'),$result,[Text.UTF8Encoding]::new($false))
Write-Output 'Vocation oracle:128 cases per architecture;None,ordinal UTF16,duplicate-first-win and unchecked rank sum.'
