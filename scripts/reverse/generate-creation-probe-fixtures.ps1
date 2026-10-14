$ErrorActionPreference='Stop'
$taskRoot=$PWD.Path
$taskMaker='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore.Functions/CharacterMaker.cs'
$taskExtension='legacy/decompiled/Utility-37359981d509/Utility/Extention.cs'
function Extract([string]$file,[string]$begin,[string]$end){
 $s=[IO.File]::ReadAllText((Join-Path $taskRoot $file));$a=$s.IndexOf($begin);if($a-lt0){throw 'Source start changed'};$b=$s.IndexOf($end,$a);if($b-le$a){throw 'Source end changed'};return $s.Substring($a,$b-$a)
}
$sn=(Extract $taskMaker 'private int MakeCharacterSN(out DateTime dt, out TimeSpan ts)' 'internal int QueryCharacterCount(').Replace('DateTime.UtcNow','Clock.Next()')
$create=Extract $taskMaker 'internal CreateCharacterResult CreateCharacter(' 'public DeleteCharacterResult DeleteCharacter('
$a=$create.IndexOf('DateTime dt;');$b=$create.IndexOf('using (CharacterDataContext characterDataContext2',$a);if($a-lt0-or$b-le$a){throw 'Creation setup changed'};$setup=$create.Substring($a,$b-$a)
$a=$create.IndexOf('int num = 0;',$b);$b=$create.IndexOf('if (usePreset)',$a);if($a-lt0-or$b-le$a){throw 'Creation loop changed'};$loop=$create.Substring($a,$b-$a)
$original='characterDataContext2.CharacterInfo.Where((CharacterInfo row) => row.UID == nexonSN && row.CharacterSN == charSN).SingleOrDefault()'
if(-not$loop.Contains($original)){throw 'Original probe expression changed'}
$loop=$loop.Replace($original,'Probe.Single(charSN)')
$epoch=Extract $taskExtension 'public static int ToInt32(this DateTime value)' 'public static DateTime ToDateTime(this int value)'
$code=[IO.File]::ReadAllText((Join-Path $taskRoot 'scripts/reverse/framework/CharacterCreationProbeOracle.cs.in')).Replace('__EPOCH__',$epoch).Replace('__SERIAL__',$sn).Replace('__SETUP__',$setup).Replace('__LOOP__',$loop)
[IO.File]::WriteAllText((Join-Path $taskRoot '.cache/framework/CharacterCreationProbeOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/CharacterCreationProbeOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')}
 $args+='.cache/framework/CharacterCreationProbeOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/creation-probe-build-'+$arch+'.log')
 if($LASTEXITCODE-ne0){throw 'Probe build failed; full log local'}
 if(Select-String ('.cache/verification/creation-probe-build-'+$arch+'.log') -Pattern 'warning [A-Z]+[0-9]+:'){throw 'Probe build warning'}
 & (Join-Path $taskRoot ('.cache/framework/CharacterCreationProbeOracle-'+$arch+'.exe')) *> ('.cache/verification/creation-probe-'+$arch+'.json')
 if($LASTEXITCODE-ne0){throw 'Probe execution failed; full log local'}
}
$x=Get-Content .cache/verification/creation-probe-x86.json -Raw|ConvertFrom-Json;$y=Get-Content .cache/verification/creation-probe-x64.json -Raw|ConvertFrom-Json
if(($x.cases|ConvertTo-Json -Depth 8 -Compress)-cne($y.cases|ConvertTo-Json -Depth 8 -Compress)){throw 'Probe architecture disagreement'}
[ordered]@{profiles=@($x,$y);original_services_executed=$false;scope='Original zero-SN retry and initial-SN SingleOrDefault loop; fixed synthetic UTC and row multiplicity';sources=@(($taskMaker,$taskExtension,'scripts/reverse/framework/CharacterCreationProbeOracle.cs.in','scripts/reverse/generate-creation-probe-fixtures.ps1')|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}})}|ConvertTo-Json -Depth 12 -Compress|Set-Content tests/fixtures/framework-character-creation-probe.json -Encoding utf8NoBOM
Write-Output 'Creation probe oracle:3 cases per architecture;0 differences;original services not executed.'
