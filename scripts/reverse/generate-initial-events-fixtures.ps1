$ErrorActionPreference='Stop'
$root=$PWD.Path
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source method missing'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}



function Field([string]$path,[string]$name){
 $s=[IO.File]::ReadAllText((Join-Path $root $path));$m=[regex]::Match($s,'(?m)^\s*private [^\r\n;]*\b'+[regex]::Escape($name)+'(?:\s*=[^\r\n;]*)?;')
 if(-not$m.Success){throw 'Source field missing'};return $m.Value
}
& ./scripts/reverse/recover-initial-events-metadata.ps1 *> .cache/verification/initial-events-static-recovery.log
$base='legacy/decompiled/PlayerService-f23d8b9df77c/'
$roulette=$base+'RouletteManager.cs';$marble=$base+'PlayerService.ItemServiceCore.MarbleSystem/MarbleManager.cs'
$log=$base+'PlayerService.ItemServiceCore/ItemLog.cs'
$template='scripts/reverse/framework/InitialEventsOracle.cs.in'
$code=[IO.File]::ReadAllText((Join-Path $root $template))
$parts=@{
 '__ADDJOURNAL__'=(Method $log 'public static long AddItemJournal(string operationName)')
 '__ROULETTE_FIELDS__'=(@('rouletteExpire','rouletteVersion','rouletteID','rouletteSlotInfo','appearanceCount')|ForEach-Object{Field $roulette $_})-join([string][char]10)
 '__ROULETTE__'=(Method $roulette 'public RouletteManager(CharacterEntity entity)')
 '__EXPIRED__'=(Method $roulette 'public bool IsExpired')
 '__MARBLE_FIELDS__'=(@('MarbleBoardInfo','MarbleNodeInfoList','JournalID','State','ChanceResultNodeIndexList','ChanceDiceFaceNumber')|ForEach-Object{Field $marble $_})-join([string][char]10)
 '__STATE_ENUM__'=(Method $marble 'public enum MarbleState')
 '__MARBLE__'=(Method $marble 'public MarbleManager(CharacterEntity entity)').Replace('ItemLog.AddItemJournal("MarbleLog")','Events.Journal(this)')
 '__INITIALIZED__'=(Method $marble 'public bool Initilized')
}
foreach($key in $parts.Keys){$code=$code.Replace($key,$parts[$key])}
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/InitialEventsOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/InitialEventsOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')}
 $args+='.cache/framework/InitialEventsOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/initial-events-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Event oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/InitialEventsOracle-'+$arch+'.exe')) *> ('.cache/verification/initial-events-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Event oracle failed;local log'}
}
$x=Get-Content .cache/verification/initial-events-x86.json -Raw|ConvertFrom-Json -DateKind String
$y=Get-Content .cache/verification/initial-events-x64.json -Raw|ConvertFrom-Json -DateKind String
if(($x|ConvertTo-Json -Depth 15 -Compress)-cne($y|ConvertTo-Json -Depth 15 -Compress)){throw 'Architecture mismatch'}
$sources=@($roulette,$marble,$log,($base+'PlayerService.CharacterServiceCore/CharacterEntity.cs'),($base+'PlayerService/PlayerService.cs'),($base+'PlayerService.ItemServiceCore.ItemInstance/ItemInstanceDataContext.cs'),$template,'scripts/reverse/recover-initial-events-metadata.ps1','scripts/reverse/generate-initial-events-fixtures.ps1')
$hashes=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$metadata=Get-Content .cache/verification/initial-events-metadata.json -Raw|ConvertFrom-Json -DateKind String
$binary='../ServidorVindictus/Server/zh-tw-x-gm/Bin/PlayerService.dll'
if($metadata.player_service_sha256-ne(Get-FileHash $binary).Hash.ToLowerInvariant()){throw 'Static metadata artifact mismatch'}
$il=[IO.File]::ReadAllText((Join-Path $root '.cache/framework/RouletteManager.il'))
foreach($name in @('RouletteManager','PlayerService.ItemServiceCore.MarbleSystem.MarbleManager')){
 if(-not[regex]::IsMatch($il,'(?m)^\.class [^\r\n]*beforefieldinit '+[regex]::Escape($name)+'\r?$')){throw 'Original beforefieldinit flag absent'}
}
$result=[ordered]@{source_sha256=$hashes;scope='complete original instance constructors/nonstatic field initializers/initial getters and ItemLog.AddItemJournal;contexts/features/journal provider instrumented;Marble call wrapped only to observe pre-journal state;static initialization explicitly excluded and identified separately,not forced by empty startup call;context-construction-failure controls outside native admission';static_metadata=$metadata;x86=$x;x64=$y}|ConvertTo-Json -Depth 18 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-initial-events.json'),$result,[Text.UTF8Encoding]::new($false))
Write-Output 'Events oracle:64 cases per architecture;x86=x64;constructor/journal failure boundaries.'
