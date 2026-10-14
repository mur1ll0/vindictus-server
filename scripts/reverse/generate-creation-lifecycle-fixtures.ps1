$ErrorActionPreference='Stop'
$root=$PWD.Path
$operation='legacy/decompiled/UnifiedNetwork-70191dde69d5/UnifiedNetwork.Cooperation/Operation.cs'
$processor='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore.Processor/CreateCharacterProcessor.cs'
function Extract([string]$file,[string]$start,[string]$end){$s=[IO.File]::ReadAllText((Join-Path $root $file));$a=$s.IndexOf($start);if($a-lt0){throw 'Source start changed'};$b=$s.IndexOf($end,$a);if($b-le$a){throw 'Source end changed'};return $s.Substring($a,$b-$a)}
$events=Extract $operation 'internal void IssueCompleteEvent()' 'public override string ToString()'
$callbacks=Extract $processor 'Action<Operation> onCompleteCallback = delegate' 'using (CharacterDataContext context'
$code=[IO.File]::ReadAllText((Join-Path $root 'scripts/reverse/framework/CreationLifecycleOracle.cs.in')).Replace('__EVENTS__',$events).Replace('__CALLBACKS__',$callbacks)
[IO.File]::WriteAllText((Join-Path $root '.cache/framework/CreationLifecycleOracle.cs'),$code)
foreach($arch in 'x86','x64'){
$args=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$arch),('/out:.cache/framework/CreationLifecycleOracle-'+$arch+'.exe'))
foreach($lib in 'mscorlib','System','System.Core','System.Web.Extensions'){$args+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$lib+'.dll')};$args+='.cache/framework/CreationLifecycleOracle.cs'
& 'C:/Program Files/dotnet/dotnet.exe' @args *> ('.cache/verification/creation-lifecycle-build-'+$arch+'.log');if($LASTEXITCODE-ne0){throw 'Lifecycle oracle build failed; log local'}
if(Select-String ('.cache/verification/creation-lifecycle-build-'+$arch+'.log') -Pattern 'warning [A-Z]+[0-9]+:'){throw 'Lifecycle oracle warning'}
& (Join-Path $root ('.cache/framework/CreationLifecycleOracle-'+$arch+'.exe')) *> ('.cache/verification/creation-lifecycle-'+$arch+'.json');if($LASTEXITCODE-ne0){throw 'Lifecycle oracle failed; log local'}
}
$x=Get-Content .cache/verification/creation-lifecycle-x86.json -Raw|ConvertFrom-Json;$y=Get-Content .cache/verification/creation-lifecycle-x64.json -Raw|ConvertFrom-Json
if($x.cases.Count-ne124-or($x.cases|ConvertTo-Json -Depth 8 -Compress)-cne($y.cases|ConvertTo-Json -Depth 8 -Compress)){throw 'Lifecycle case/architecture mismatch'}
[ordered]@{profiles=@($x,$y);original_services_executed=$false;sources=@(($operation,$processor,'legacy/decompiled/UnifiedNetwork-70191dde69d5/UnifiedNetwork.Cooperation/OperationProcessor.cs','scripts/reverse/framework/CreationLifecycleOracle.cs.in','scripts/reverse/generate-creation-lifecycle-fixtures.ps1')|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}})}|ConvertTo-Json -Depth 12 -Compress|Set-Content tests/fixtures/framework-creation-lifecycle.json -Encoding utf8NoBOM
Write-Output 'Creation lifecycle oracle:124 cases per architecture;0differences;original services not executed.'
