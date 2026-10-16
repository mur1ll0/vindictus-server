$ErrorActionPreference='Stop'
$root=$PWD.Path
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source method missing'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}
$player='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/'
$info='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/CostumeInfo.cs'
$template='scripts/reverse/framework/LobbyCostumeOracle.cs.in'
$code=[IO.File]::ReadAllText((Join-Path $root $template)).Replace('__CTOR__',(Method $info 'public CostumeInfo()')).Replace('__APPLY__',(Method ($player+'CostumeDataExtension.cs') 'public static void Apply('))
foreach($kind in @('ExtraKey','CostumeType')){$raw=[IO.File]::ReadAllText((Join-Path $root ($player+$kind+'.cs')));$raw=$raw.Substring($raw.IndexOf('public enum'));$code=$code.Replace($(if($kind-eq'ExtraKey'){'__EXTRA_ENUM__'}else{'__TYPE_ENUM__'}),$raw)}
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/LobbyCostumeOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/LobbyCostumeOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')};$args+='.cache/framework/LobbyCostumeOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/lobby-costume-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Lobby costume oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/LobbyCostumeOracle-'+$arch+'.exe')) *> ('.cache/verification/lobby-costume-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Lobby costume oracle failed;local log'}
}
$sources=@($info;($player+'CostumeDataExtension.cs');($player+'CostumeManager.cs');($player+'ExtraKey.cs');($player+'CostumeType.cs');$template;'scripts/reverse/generate-lobby-costume-fixtures.ps1';'scripts/reverse/compact-lobby-costume-fixtures.py')
$hashes=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$profiles=@();foreach($arch in 'x86','x64'){$profiles+=@{profile=('Framework48-'+$arch);cases=(Get-Content ('.cache/verification/lobby-costume-'+$arch+'.json') -Raw|ConvertFrom-Json -DateKind String)}}
$result=[ordered]@{source_sha256=$hashes;profiles=$profiles}|ConvertTo-Json -Depth 16 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-lobby-costume.json'),$result.Replace("`r`n","`n"),[Text.UTF8Encoding]::new($false))
& python scripts/reverse/compact-lobby-costume-fixtures.py
if($LASTEXITCODE-ne0){throw 'Synthetic fixture compaction failed'}
Write-Output 'Lobby costume original projection:256 synthetic cases per architecture;values stay synthetic.'
