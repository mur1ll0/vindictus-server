$ErrorActionPreference='Stop'
$root=$PWD.Path
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source method missing'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}


$base='legacy/decompiled/PlayerService-f23d8b9df77c/'
$attendance=$base+'PlayerService.CharacterServiceCore.Attendance/'
$manager=$attendance+'AttendanceEventManager.cs'
$core=$attendance+'Attendance.cs'
$all=$attendance+'AttendanceAllUser.cs'
$first=$base+'PlayerService.CharacterServiceCore/CharacterDataContext.cs'
$feature='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore/FeatureMatrix.cs'
$type=$attendance+'AttendanceEventType.cs';$error=$attendance+'AttendanceErrorResult.cs'
$template='scripts/reverse/framework/InitialAttendanceOracle.cs.in'
$code=[IO.File]::ReadAllText((Join-Path $root $template))
$parts=@{
 '__FIRST__'=(Method $first 'public bool IsFirstCharacterCreated(int nexonSN)')
 '__NULLABLE__'=(Method $feature 'public static int? GetNullableInt(')
 '__TYPE_ENUM__'=(Method $type 'public enum AttendanceEventType')
 '__ERROR_ENUM__'=(Method $error 'public enum AttendanceErrorResult')
 '__BASE__'=(Method $core 'public Attendance(CharacterEntity entity, AttendanceEventManager manager)')
 '__ALL__'=(Method $all 'public AttendanceAllUser(CharacterEntity entity, AttendanceEventManager manager)')
 '__MANAGER__'=(Method $manager 'public AttendanceEventManager(CharacterEntity parent, Service service)')
 '__INITIALIZE__'=(Method $manager 'public void Initialize()')
 '__NEWBIE__'=(Method $manager 'public void CreateNewbieInstance(').Replace('DateTime.UtcNow','Events.UtcNow')
}
foreach($key in $parts.Keys){$code=$code.Replace($key,$parts[$key])}
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/InitialAttendanceOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/InitialAttendanceOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')}
 $args+='.cache/framework/InitialAttendanceOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/initial-attendance-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Attendance oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/InitialAttendanceOracle-'+$arch+'.exe')) *> ('.cache/verification/initial-attendance-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Attendance oracle failed;local log'}
}
$x=Get-Content .cache/verification/initial-attendance-x86.json -Raw|ConvertFrom-Json -DateKind String
$y=Get-Content .cache/verification/initial-attendance-x64.json -Raw|ConvertFrom-Json -DateKind String
if(($x|ConvertTo-Json -Depth 15 -Compress)-cne($y|ConvertTo-Json -Depth 15 -Compress)){throw 'Architecture mismatch'}
$sources=@($manager,$core,$all,$first,$feature,$type,$error,($base+'PlayerService.ItemServiceCore.ItemInstance/ItemInstanceDataContext.cs'),($base+'PlayerService/PlayerService.cs'),($base+'RouletteManager.cs'),$template,'scripts/reverse/generate-initial-attendance-fixtures.ps1')
$hashes=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$result=[ordered]@{source_sha256=$hashes;scope='original Attendance manager/Initialize/CreateNewbieInstance,Attendance and AllUser constructors,FirstCharacterCreated and GetNullableInt complete;LINQ provider counted,contexts/content/UTC instrumented;existing/active writers trapped and explicitly excluded;initial fields projected,not scheduler/full DataContext';x86=$x;x64=$y}|ConvertTo-Json -Depth 18 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-initial-attendance.json'),$result,[Text.UTF8Encoding]::new($false))
Write-Output 'Attendance oracle:96 cases per architecture;x86=x64;external branches trapped.'
