$ErrorActionPreference='Stop'
$taskRoot=$PWD.Path
function SourceMethod([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $taskRoot $path));$start=$s.IndexOf($signature);if($start-lt0){throw 'Method missing'};$b=$s.IndexOf('{',$start);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Method braces changed'};return $s.Substring($start,$e-$start)}
$taskManager='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/CostumeManager.cs'
$taskExtension='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/CostumeDataExtension.cs'
$taskTemplate='scripts/reverse/framework/EquippedCostumeOracle.cs.in'
$taskCode=[IO.File]::ReadAllText((Join-Path $taskRoot $taskTemplate))
$taskMethods=''
foreach($taskMethod in 'public Costume GetCostume(','public void SetCostume(','public void RemoveCostume(','public void LoadItemCostumes('){$taskMethods+=(SourceMethod $taskManager $taskMethod)+"`n"}
$taskCode=$taskCode.Replace('__METHODS__',$taskMethods).Replace('__TO_COSTUME__',(SourceMethod $taskExtension 'public static Costume ToCostume('))
[IO.File]::WriteAllText((Join-Path $taskRoot '.cache/framework/EquippedCostumeOracle.cs'),$taskCode,[Text.UTF8Encoding]::new($false))
$taskProfiles=@()
foreach($taskArch in 'x86','x64'){
 $taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$taskArch),('/out:.cache/framework/EquippedCostumeOracle-'+$taskArch+'.exe'))
 foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions','System.Data','System.Data.Linq'){$taskArgs+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLib+'.dll')}
 $taskArgs+='.cache/framework/EquippedCostumeOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs *> ('.cache/verification/equipped-costume-build-'+$taskArch+'.log');if($LASTEXITCODE-ne0){throw 'Costume oracle build failed'}
 & (Join-Path $taskRoot ('.cache/framework/EquippedCostumeOracle-'+$taskArch+'.exe')) *> ('.cache/verification/equipped-costume-'+$taskArch+'.json');if($LASTEXITCODE-ne0){throw 'Costume oracle execution failed'}
 $taskProfiles+=,(Get-Content ('.cache/verification/equipped-costume-'+$taskArch+'.json') -Raw|ConvertFrom-Json -DateKind String)
}
$taskPaths=@($taskManager,$taskExtension,$taskTemplate,'scripts/reverse/generate-equipped-costume-fixtures.ps1')
$taskResult=[ordered]@{scope='Original ToCostume/GetCostume/SetCostume/RemoveCostume/LoadItemCostumes with actual Framework EntitySet and synthetic metadata; CostumeInfo view callbacks instrumented; x64 conversion admitted by new loader';sources=@($taskPaths|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});x86=$taskProfiles[0];x64=$taskProfiles[1]}|ConvertTo-Json -Depth 20 -Compress
[IO.File]::WriteAllText((Join-Path $taskRoot 'tests/fixtures/framework-equipped-costume.json'),($taskResult+"`n"),[Text.UTF8Encoding]::new($false))
Write-Output 'Equipped costume oracle:256 cases per architecture; conversion, alias effects, base fallback and EntitySet remove/reinsert order.'
