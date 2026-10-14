$ErrorActionPreference='Stop'
$taskRoot=$PWD.Path
$maker='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore.Functions/CharacterMaker.cs'
$create='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/CreateCharacterResult.cs'
$name='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/CharacterNameCheckResult.cs'
$s=[IO.File]::ReadAllText((Join-Path $taskRoot $maker));$a=$s.IndexOf('internal CreateCharacterResult ResultChanger(');$b=$s.IndexOf('internal CreateCharacterResult CreateCharacter(', $a);if($a-lt0-or$b-le$a){throw 'Source mapping boundaries changed'}
function EnumSource([string]$path){$v=[IO.File]::ReadAllText((Join-Path $taskRoot $path));return $v.Replace('namespace ServiceCore.EndPointNetwork;','namespace ServiceCore.EndPointNetwork {')+"`n}"}
$code=[IO.File]::ReadAllText((Join-Path $taskRoot 'scripts/reverse/framework/CharacterMakerResultOracle.cs.in')).Replace('__CREATE_ENUM__',(EnumSource $create)).Replace('__NAME_ENUM__',(EnumSource $name)).Replace('__RESULT_CHANGER__',$s.Substring($a,$b-$a))
[IO.File]::WriteAllText((Join-Path $taskRoot '.cache/framework/CharacterMakerResultOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/CharacterMakerResultOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')};$args+='.cache/framework/CharacterMakerResultOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/creation-maker-enums-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Enum build failed; log local'}
 if(Select-String ('.cache/verification/creation-maker-enums-build-'+$arch+'.log') -Pattern 'warning [A-Z]+[0-9]+:'){throw 'Enum build warning'}
 & (Join-Path $taskRoot ('.cache/framework/CharacterMakerResultOracle-'+$arch+'.exe')) *> ('.cache/verification/creation-maker-enums-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Enum oracle failed; log local'}
}
$x=Get-Content .cache/verification/creation-maker-enums-x86.json -Raw|ConvertFrom-Json;$y=Get-Content .cache/verification/creation-maker-enums-x64.json -Raw|ConvertFrom-Json
foreach($key in 'values','cases'){if(($x.$key|ConvertTo-Json -Depth 8 -Compress)-cne($y.$key|ConvertTo-Json -Depth 8 -Compress)){throw 'Enum architecture disagreement'}}
[ordered]@{profiles=@($x,$y);original_services_executed=$false;sources=@(($maker,$create,$name,'scripts/reverse/framework/CharacterMakerResultOracle.cs.in','scripts/reverse/generate-character-maker-result-fixtures.ps1')|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}})}|ConvertTo-Json -Depth 12 -Compress|Set-Content tests/fixtures/framework-character-maker-results.json -Encoding utf8NoBOM
Write-Output 'Creation result oracle:16 values and11 mapping cases per architecture;0differences;original services not executed.'
