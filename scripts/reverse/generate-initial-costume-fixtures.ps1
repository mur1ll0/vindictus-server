$ErrorActionPreference='Stop'
$root=$PWD.Path
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source method missing'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}
$player='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/'
$info='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/CostumeInfo.cs'
$template='scripts/reverse/framework/InitialCostumeOracle.cs.in'
$code=[IO.File]::ReadAllText((Join-Path $root $template)).Replace('__CTOR__',(Method $info 'public CostumeInfo()')).Replace('__APPLY__',(Method ($player+'CostumeDataExtension.cs') 'public static void Apply('))
$code=$code.Replace('__LOAD__',(Method ($player+'CostumeManager.cs') 'public void LoadFromDB()'))
foreach($kind in @('ExtraKey','CostumeType')){$raw=[IO.File]::ReadAllText((Join-Path $root ($player+$kind+'.cs')));$raw=$raw.Substring($raw.IndexOf('public enum'));$code=$code.Replace($(if($kind-eq'ExtraKey'){'__EXTRA_ENUM__'}else{'__TYPE_ENUM__'}),$raw)}
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/InitialCostumeOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/InitialCostumeOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')};$args+='.cache/framework/InitialCostumeOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/initial-costume-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Lobby costume oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/InitialCostumeOracle-'+$arch+'.exe')) *> ('.cache/verification/initial-costume-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Lobby costume oracle failed;local log'}
}
$sources=@($info;($player+'CostumeDataExtension.cs');($player+'CostumeManager.cs');($player+'ExtraKey.cs');($player+'CostumeType.cs');$template;'scripts/reverse/generate-initial-costume-fixtures.ps1')
$hashes=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$profiles=@();foreach($arch in 'x86','x64'){$profiles+=@{profile=('Framework48-'+$arch);cases=(Get-Content ('.cache/verification/initial-costume-'+$arch+'.json') -Raw|ConvertFrom-Json -DateKind String)}}
$result=[ordered]@{source_sha256=$hashes;profiles=$profiles}|ConvertTo-Json -Depth 16 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-initial-costume.json'),$result.Replace("`r`n","`n"),[Text.UTF8Encoding]::new($false))
Write-Output 'Initial costume oracle:64 LoadFromDB cases per Framework architecture;explicit context and nested maps.'
