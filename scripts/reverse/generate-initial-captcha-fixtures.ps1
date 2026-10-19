$ErrorActionPreference='Stop'
$root=$PWD.Path
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source method missing'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}



function Field([string]$path,[string]$name){
 $s=[IO.File]::ReadAllText((Join-Path $root $path));$m=[regex]::Match($s,'(?m)^\s*private [^\r\n;]*\b'+[regex]::Escape($name)+'(?:\s*=[^\r\n;]*)?;')
 if(-not$m.Success){throw 'Source field missing'};return $m.Value
}
$base='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/'
$manager=$base+'CaptchaManager.cs';$result=$base+'UserCaptchaGetResult.cs'
$template='scripts/reverse/framework/InitialCaptchaOracle.cs.in'
$code=[IO.File]::ReadAllText((Join-Path $root $template))
$parts=@{
 '__FIELDS__'=(@('Parent','UID','CID','Pool','TimeoutSchedulerID','LastCheckConditionTime','RefreshCount','LastRefreshTime','PunishedCount','FailureCount','LastFailureTime','LastSucceedTime','ForceCondition')|ForEach-Object{Field $manager $_})-join([string][char]10)
 '__CTOR__'=(Method $manager 'public CaptchaManager(CharacterEntity entity)').Replace('DateTime.UtcNow','Events.UtcNow')
 '__SAVE__'=(Method $manager 'private void Save()')
 '__LOGOUT__'=(Method $manager 'public void OnLogOut()')
 '__PROGRESS__'=(Method $manager 'public bool IsInProgress()')
 '__RESULT__'=(Method $result 'public class UserCaptchaGetResult')
}
foreach($key in $parts.Keys){$code=$code.Replace($key,$parts[$key])}
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/InitialCaptchaOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/InitialCaptchaOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions','System.Data','System.Data.Linq'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')}
 $args+='.cache/framework/InitialCaptchaOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/initial-captcha-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Event oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/InitialCaptchaOracle-'+$arch+'.exe')) *> ('.cache/verification/initial-captcha-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Event oracle failed;local log'}
}
$x=Get-Content .cache/verification/initial-captcha-x86.json -Raw|ConvertFrom-Json -DateKind String
$y=Get-Content .cache/verification/initial-captcha-x64.json -Raw|ConvertFrom-Json -DateKind String
if(($x|ConvertTo-Json -Depth 15 -Compress)-cne($y|ConvertTo-Json -Depth 15 -Compress)){throw 'Architecture mismatch'}
$sources=@($manager,$result,($base+'CharacterDataContext.cs'),($base+'CharacterEntity.cs'),$template,'scripts/reverse/generate-initial-captcha-fixtures.ps1')
$hashes=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$result=[ordered]@{source_sha256=$hashes;scope='complete original constructor/nonstatic fields/result mapper/IsInProgress/OnLogOut/Save;query/context/UTC/logger and SQL parameter boundaries instrumented;pool remains empty;static Rand/challenges/scheduler/UserCaptchaCheck excluded';x86=$x;x64=$y}|ConvertTo-Json -Depth 18 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-initial-captcha.json'),$result,[Text.UTF8Encoding]::new($false))
Write-Output 'Captcha oracle:128 cases per architecture;x86=x64;nullable/UTC and empty logout/save failures.'
