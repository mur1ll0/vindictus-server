$ErrorActionPreference='Stop'
$root=$PWD.Path
$source='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/CostumeManager.cs'
$s=[IO.File]::ReadAllText((Join-Path $root $source))
function Extract([string]$a,[string]$b){$start=$s.IndexOf($a);if($start-lt0){throw 'Costume method start changed'};$end=$s.IndexOf($b,$start);if($end-le$start){throw 'Costume method end changed'};return $s.Substring($start,$end-$start)}
$methods=(Extract 'public Costume GetCostume(' 'public void RemoveCostume(')+(Extract 'public void LoadExtraData(ExtraKey key, int value)' 'public void LoadExtraData(Costume costume)')+(Extract 'public void LoadBodyShape(' 'public void ApplyTownEffect(')
$code=[IO.File]::ReadAllText((Join-Path $root 'scripts/reverse/framework/CostumeOracle.cs.in')).Replace('__METHODS__',$methods)
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/CostumeOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/CostumeOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions','System.Data','System.Data.Linq'){$taskArgs+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')};$taskArgs+='.cache/framework/CostumeOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs *> ('.cache/verification/costume-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Costume oracle build failed; log local'}
 & (Join-Path $root ('.cache/framework/CostumeOracle-'+$arch+'.exe')) *> ('.cache/verification/costume-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Costume oracle failed; log local'}
}
$x=Get-Content .cache/verification/costume-x86.json -Raw|ConvertFrom-Json;$y=Get-Content .cache/verification/costume-x64.json -Raw|ConvertFrom-Json
$differences=0;for($i=0;$i-lt$x.floats.Count;$i++){if($x.floats[$i].sn-ne$y.floats[$i].sn){$differences++}};if(($x.ordered|ConvertTo-Json -Depth 8 -Compress)-cne($y.ordered|ConvertTo-Json -Depth 8 -Compress)-or$differences-ne57-or($x.updates|ConvertTo-Json -Depth 8 -Compress)-cne($y.updates|ConvertTo-Json -Depth 8 -Compress)-or($x.extras|ConvertTo-Json -Depth 8 -Compress)-cne($y.extras|ConvertTo-Json -Depth 8 -Compress)){throw 'Costume measured profile changed'}
[ordered]@{profiles=@($x,$y);conversion_architecture_disagreements=$differences;original_services_executed=$false;sources=@(($source,'scripts/reverse/framework/CostumeOracle.cs.in','scripts/reverse/generate-costume-fixtures.ps1')|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}})}|ConvertTo-Json -Depth 12 -Compress|Set-Content tests/fixtures/framework-costume.json -Encoding utf8NoBOM
Write-Output 'Costume oracle:2062 body conversions,16 alias updates,10 extras per architecture;57 measured float differences;original services not executed.'
