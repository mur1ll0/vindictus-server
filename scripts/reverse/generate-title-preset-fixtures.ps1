$ErrorActionPreference='Stop'
$root=$PWD.Path
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source method missing'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}
$base='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/'
$manager=$base+'TitleManager.cs';$title=$base+'Title.cs';$goal=$base+'TitleGoalProgress.cs';$event=$base+'CharacterTitleEvent.cs'
$template='scripts/reverse/framework/TitlePresetOracle.cs.in'
$code=[IO.File]::ReadAllText((Join-Path $root $template))
$parts=@{
 '__PRESET__'=(Method $manager 'public void GiveTitlePreset()')
 '__TITLE_CTOR__'=(Method $title 'public Title(long cid, int titleId, bool acquired, DateTime? acquiredTime, string acquiredQuest, DateTime? expireDateTime)')
 '__TITLE_SET__'=(Method $title 'private void SetTitle(')
 '__GOAL_CTOR__'=(Method $goal 'public TitleGoalProgress(long cid, int titleGoalId, int progress)')
 '__EVENT_ENUM__'=(Method $event 'public enum CharacterTitleEvent')
}
foreach($key in $parts.Keys){$code=$code.Replace($key,$parts[$key])}
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/TitlePresetOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/TitlePresetOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions','System.Data','System.Data.Linq'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')}
 $args+='.cache/framework/TitlePresetOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/title-preset-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Event oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/TitlePresetOracle-'+$arch+'.exe')) *> ('.cache/verification/title-preset-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Event oracle failed;local log'}
}
$x=Get-Content .cache/verification/title-preset-x86.json -Raw|ConvertFrom-Json -DateKind String
$y=Get-Content .cache/verification/title-preset-x64.json -Raw|ConvertFrom-Json -DateKind String
if(($x|ConvertTo-Json -Depth 15 -Compress)-cne($y|ConvertTo-Json -Depth 15 -Compress)){throw 'Architecture mismatch'}
$sources=@($manager,$title,$goal,$event,$template,'scripts/reverse/generate-title-preset-fixtures.ps1')
$hashes=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$result=[ordered]@{source_sha256=$hashes;scope='complete original GiveTitlePreset,Title ctor/SetTitle,TitleGoalProgress ctor and event enum;synchronized enabled profile;SQL/provider/enumeration/log/observable instrumented;not full TitleManager';x86=$x;x64=$y}|ConvertTo-Json -Depth 18 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-title-preset.json'),$result,[Text.UTF8Encoding]::new($false))
Write-Output 'TitlePreset oracle:40 cases per architecture;x86=x64;insert/result/enumeration failures preserve prefixes;shared goal references.'
