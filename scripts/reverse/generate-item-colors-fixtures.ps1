$ErrorActionPreference='Stop'
$root=$PWD.Path
$base='legacy/decompiled/PlayerService-f23d8b9df77c/'
$color=$base+'PlayerService.ItemServiceCore.Static/Color.cs'
$material=$base+'PlayerService.ItemServiceCore.Static/Material.cs'
$table=$base+'PlayerService.ItemServiceCore.Static/ColorTable.cs'
$item=$base+'PlayerService.ItemServiceCore.ItemInstance/Item.cs'
function Body([string]$path,[string]$start,[string]$end=''){
 $s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($start);if($i-lt0){throw 'Source start changed'}
 if($end-eq''){return $s.Substring($i)};$j=$s.IndexOf($end,$i);if($j-le$i){throw 'Source end changed'};return $s.Substring($i,$j-$i)
}
$methods=(Body $table 'public static Material GetMaterial(').TrimEnd();if(-not$methods.EndsWith('}')){throw 'Table structure changed'};$methods=$methods.Substring(0,$methods.Length-1)
$code=[IO.File]::ReadAllText((Join-Path $root 'scripts/reverse/framework/ItemColorsOracle.cs.in')).Replace('__COLOR__',(Body $color 'internal struct Color')).Replace('__MATERIAL__',(Body $material 'internal class Material')).Replace('__TABLE__',$methods).Replace('__SET__',(Body $item 'public void SetColor(' 'public bool AntiBindItem('))
New-Item -ItemType Directory -Force .cache/framework,.cache/verification|Out-Null
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/ItemColorsOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/ItemColorsOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')};$taskArgs+='.cache/framework/ItemColorsOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs *> ('.cache/verification/item-colors-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Colors oracle build failed;logs local'}
 & (Join-Path $root ('.cache/framework/ItemColorsOracle-'+$arch+'.exe')) *> ('.cache/verification/item-colors-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Colors oracle failed;logs local'}
}
$x=Get-Content .cache/verification/item-colors-x86.json -Raw;$y=Get-Content .cache/verification/item-colors-x64.json -Raw
if($x-cne$y){throw 'Architecture difference requires explicit profile'}
$cases=$x|ConvertFrom-Json -DateKind String
$runtime=@('C:/Windows/Microsoft.NET/Framework/v4.0.30319/mscorlib.dll','C:/Windows/Microsoft.NET/Framework64/v4.0.30319/mscorlib.dll')|ForEach-Object{[ordered]@{sha256=(Get-FileHash $_).Hash.ToLowerInvariant();file_version=(Get-Item $_).VersionInfo.FileVersion}}
[ordered]@{profiles=@('Framework48-x86','Framework48-x64');cases=$cases;framework_runtime=@($runtime);original_services_executed=$false;sources=@(($color,$material,$table,$item,'scripts/reverse/framework/ItemColorsOracle.cs.in','scripts/reverse/generate-item-colors-fixtures.ps1')|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}})}|ConvertTo-Json -Depth 20 -Compress|Set-Content tests/fixtures/framework-item-colors.json -Encoding utf8NoBOM
Write-Output ('Colors oracle: '+$cases.cases.Count+' SetColor cases;'+$cases.coords.Count+' coordinates;'+$cases.rng.Count+' seeded RNG sequences;'+$cases.raw.Count+' raw lengths;zero architecture differences.')
