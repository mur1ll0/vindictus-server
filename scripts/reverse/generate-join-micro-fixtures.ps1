$ErrorActionPreference='Stop'
$root=$PWD.Path
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source method missing'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}
$source='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/CharacterEntity.cs'
$template='scripts/reverse/framework/JoinMicroOracle.cs.in'
$micro='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/MicroPlayStatus.cs'
$init=Method $source 'public void InitializeMicroPlayStatus()'
$replacements=@{
 '(utcNow.ToLocalTime().Date + TimeOfDay).ToUniversalTime()'='Universal(Local(utcNow).Date + TimeOfDay)'
 '(LastDailyResetTime.ToLocalTime() + new TimeSpan(1, 0, 0, 0)).ToUniversalTime()'='Universal(Local(LastDailyResetTime) + new TimeSpan(1, 0, 0, 0))'
 '(LastDailyResetTime.ToLocalTime() - new TimeSpan(1, 0, 0, 0)).ToUniversalTime()'='Universal(Local(LastDailyResetTime) - new TimeSpan(1, 0, 0, 0))'
}
foreach($key in $replacements.Keys){if(-not$init.Contains($key)){throw 'Original timezone expression changed'};$init=$init.Replace($key,$replacements[$key])}
$init=$init.Replace('DateTime.UtcNow','Events.UtcNow')
$reset=(Method $source 'public void ResetDailyMicroPlayCount()').Replace('DateTime.UtcNow','Events.UtcNow')
$code=[IO.File]::ReadAllText((Join-Path $root $template)).Replace('__INIT__',$init).Replace('__RESET__',$reset).Replace('__CHAT__',(Method $micro 'public void InitializeChatReport()').Replace('DateTime.UtcNow','Events.UtcNow'))
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/JoinMicroOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/JoinMicroOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions','System.Data','System.Data.Linq'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')}
 $args+='.cache/framework/JoinMicroOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/join-micro-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Event oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/JoinMicroOracle-'+$arch+'.exe')) *> ('.cache/verification/join-micro-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Event oracle failed;local log'}
}
$x=Get-Content .cache/verification/join-micro-x86.json -Raw|ConvertFrom-Json -DateKind String
$y=Get-Content .cache/verification/join-micro-x64.json -Raw|ConvertFrom-Json -DateKind String
if(($x|ConvertTo-Json -Depth 15 -Compress)-cne($y|ConvertTo-Json -Depth 15 -Compress)){throw 'Architecture mismatch'}
$sources=@($source,$micro,$template,'scripts/reverse/generate-join-micro-fixtures.ps1','docs/evidence/database/offline-vm-clock.json')
$hashes=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$result=[ordered]@{source_sha256=$hashes;scope='complete original InitializeMicroPlayStatus and ResetDailyMicroPlayCount;fresh missing row;UTC/provider instrumented;explicit recovered fixed UTC+8 substitutes host conversions;not runtime timezone observation';x86=$x;x64=$y}|ConvertTo-Json -Depth 15 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-join-micro.json'),$result,[Text.UTF8Encoding]::new($false))
Write-Output 'JoinMicro oracle:128 cases per architecture;x86=x64;UTC+8 strict daily boundaries and separate clock/submit failures.'
