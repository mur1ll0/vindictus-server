$ErrorActionPreference='Stop'
$root=$PWD.Path
$property='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore.ItemInstance/ItemRequestProperty.cs'
$item='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore.ItemInstance/Item.cs'
$extension='legacy/decompiled/Utility-37359981d509/Utility/Extention.cs'
function Extract([string]$path,[string]$startMarker,[string]$endMarker){
 $s=[IO.File]::ReadAllText((Join-Path $root $path));$start=$s.IndexOf($startMarker);if($start-lt0){throw 'Method start changed'};$end=$s.IndexOf($endMarker,$start);if($end-le$start){throw 'Method end changed'};return $s.Substring($start,$end-$start)
}
$methods=Extract $property 'private void Build_Attribute_Expire()' 'private void Build_Attribute_SynthesisGrade()'
$initialize=Extract $item 'ExpireDateTime = property.ExpireDateTime;' 'IsCharacterBinded = property.IsCharacterBinded;'
$format=Extract $extension 'public static string ToFormatableString(' 'public static DateTime? ToFormatableDateTime('
# Instrument only the clock expression; recovered algorithms and exception boundaries stay intact.
$code=[IO.File]::ReadAllText((Join-Path $root 'scripts/reverse/framework/ItemTimeOracle.cs.in')).Replace('__PROPERTY__',$methods.Replace('DateTime.UtcNow','Clock.UtcNow')).Replace('__INITIALIZE__',$initialize.Replace('DateTime.UtcNow','Clock.UtcNow')).Replace('__FORMAT__',$format)
New-Item -ItemType Directory -Force .cache/framework,.cache/verification | Out-Null
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/ItemTimeOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/ItemTimeOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')};$taskArgs+='.cache/framework/ItemTimeOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs *> ('.cache/verification/item-time-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Item time oracle build failed; log local'}
 & (Join-Path $root ('.cache/framework/ItemTimeOracle-'+$arch+'.exe')) *> ('.cache/verification/item-time-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Item time oracle failed; log local'}
}
$x=Get-Content .cache/verification/item-time-x86.json -Raw
$y=Get-Content .cache/verification/item-time-x64.json -Raw
if($x-cne$y){throw 'Measured architecture difference requires an explicit profile'}
$cases=$x|ConvertFrom-Json -DateKind String
[ordered]@{profiles=@('Framework48-x86-en-US','Framework48-x64-en-US');cases=$cases;original_services_executed=$false;sources=@(($property,$item,$extension,'scripts/reverse/framework/ItemTimeOracle.cs.in','scripts/reverse/generate-item-time-fixtures.ps1')|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}})}|ConvertTo-Json -Depth 20 -Compress|Set-Content tests/fixtures/framework-item-time.json -Encoding utf8NoBOM
Write-Output ('Item time oracle: '+$cases.Count+' cases per architecture; no architecture differences; original services not executed.')
