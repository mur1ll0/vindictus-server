$ErrorActionPreference='Stop'
$root=$PWD.Path
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source method missing'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}
$base='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/'
$whisper=$base+'WhisperFilter.cs';$guild=$base+'GuildInfoManager.cs'
$template='scripts/reverse/framework/InitialSocialOracle.cs.in'
$code=[IO.File]::ReadAllText((Join-Path $root $template)).Replace('__WHISPER__',(Method $whisper 'public WhisperFilter(CharacterEntity character)')).Replace('__GUILD__',(Method $guild 'public GuildInfoManager(PlayerEntity parent)')).Replace('__GUARD__',(Method $guild 'public void UpdateBriefGuildInfo()'))
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/InitialSocialOracle.cs'),$code)
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/InitialSocialOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')}
 $args+='.cache/framework/InitialSocialOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/initial-social-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Vocation oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/InitialSocialOracle-'+$arch+'.exe')) *> ('.cache/verification/initial-social-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Vocation oracle failed;local log'}
}
$x=Get-Content .cache/verification/initial-social-x86.json -Raw|ConvertFrom-Json -DateKind String
$y=Get-Content .cache/verification/initial-social-x64.json -Raw|ConvertFrom-Json -DateKind String
$sources=@($whisper;$guild;($base+'CharacterEntity.cs');'legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService/PlayerEntity.cs';$template;'scripts/reverse/generate-initial-social-fixtures.ps1')
$hashes=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$result=[ordered]@{source_sha256=$hashes;scope='original WhisperFilter/GuildInfoManager constructors and parameterless UpdateBriefGuildInfo;feature result/context instrumented;false external branch trapped and not implemented';x86=$x;x64=$y}|ConvertTo-Json -Depth 15 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-initial-social.json'),$result,[Text.UTF8Encoding]::new($false))
Write-Output 'Social oracle:32 cases per architecture;shared contexts/initial guild state;unsupported false branch trapped.'
