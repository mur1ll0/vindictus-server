$ErrorActionPreference='Stop'
$root=$PWD.Path
function Method([string]$path,[string]$signature){$s=[IO.File]::ReadAllText((Join-Path $root $path));$i=$s.IndexOf($signature);if($i-lt0){throw 'Source method missing'};$b=$s.IndexOf('{',$i);$d=1;$e=$b+1;while($d-gt0-and$e-lt$s.Length){if($s[$e]-eq'{'){$d++};if($s[$e]-eq'}'){$d--};$e++};if($d-ne0){throw 'Source braces changed'};return $s.Substring($i,$e-$i)}
$base='legacy/decompiled/PlayerService-f23d8b9df77c/'
$owner=$base+'PlayerService.ItemServiceCore/ItemOwner.cs';$loader=$base+'PlayerService.ItemServiceCore/DBLoader.cs';$logs=$base+'PlayerService.ItemServiceCore.OperationLogSystem/ItemLogManager.cs';$event=$base+'PlayerService.ItemServiceCore/EventType.cs';$character=$base+'PlayerService.CharacterServiceCore/CharacterImpl.cs';$item=$base+'PlayerService.ItemServiceCore/ItemImpl.cs'
$template='scripts/reverse/framework/EmptyItemSaveOracle.cs.in'
$code=[IO.File]::ReadAllText((Join-Path $root $template))
$parts=@{
 '__ISCHARACTER__'=([regex]::Match([IO.File]::ReadAllText((Join-Path $root $owner)),'public bool IsCharacter =>[^;]+;').Value)
 '__SAVE__'=(Method $owner 'public bool Save()')
 '__DBSAVE__'=(Method $loader 'public override bool Save()')
 '__LOGCTOR__'=(Method $logs 'public ItemLogManager(long cid)')
 '__SETJOURNAL__'=(Method $logs 'public void SetJournal(')
 '__CLEAR__'=(Method $logs 'public void Clear()')
 '__AFTERCOUNT__'=(Method $logs 'private void SetAfterCount()')
 '__WRITE__'=(Method $logs 'public void Write()')
 '__EVENT__'=(Method $event 'public enum EventType')
 '__CHARCTOR__'=(Method $character 'public CharacterImpl(IEntity entity, Service service)')
 '__ITEMCTOR__'=(Method $item 'public ItemImpl(IEntity entity, Service service)')
}
foreach($key in $parts.Keys){$code=$code.Replace($key,$parts[$key])}
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/EmptyItemSaveOracle.cs'),$code,[Text.UTF8Encoding]::new($false))
foreach($arch in 'x86','x64'){
 $args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/EmptyItemSaveOracle-'+$arch+'.exe'))
 foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions','System.Data','System.Data.Linq'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')}
 $args+='.cache/framework/EmptyItemSaveOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/empty-item-save-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Event oracle build failed;local log'}
 & (Join-Path $root ('.cache/framework/EmptyItemSaveOracle-'+$arch+'.exe')) *> ('.cache/verification/empty-item-save-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Event oracle failed;local log'}
}
$x=Get-Content .cache/verification/empty-item-save-x86.json -Raw|ConvertFrom-Json -DateKind String
$y=Get-Content .cache/verification/empty-item-save-x64.json -Raw|ConvertFrom-Json -DateKind String
if(($x|ConvertTo-Json -Depth 15 -Compress)-cne($y|ConvertTo-Json -Depth 15 -Compress)){throw 'Architecture mismatch'}
$sources=@($owner,$loader,$logs,$event,$character,$item,$template,'scripts/reverse/generate-empty-item-save-fixtures.ps1')
$hashes=@($sources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
$result=[ordered]@{source_sha256=$hashes;scope='complete original ItemOwner.Save,DBLoader.Save,empty ItemLogManager methods and CharacterImpl/ItemImpl ctors;provider/logger/scheduler instrumented;Undo only recorded as required,not executed;not complete ItemOwner';x86=$x;x64=$y}|ConvertTo-Json -Depth 18 -Compress
[IO.File]::WriteAllText((Join-Path $root 'tests/fixtures/framework-empty-item-save.json'),$result,[Text.UTF8Encoding]::new($false))
Write-Output 'Empty item Save oracle:32cases/architecture;x86=x64;full original Save and binding constructors;Undo recorded only.'
