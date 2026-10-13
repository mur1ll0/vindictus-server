param()
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try {
 New-Item -ItemType Directory -Force .cache/framework,.cache/verification | Out-Null
 $taskSource='legacy/decompiled/LoginServiceCore-1600fb4b593d/LoginServiceCore/UserEntity.cs'
 $taskEncrypt='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.Configuration/StringEncrypter.cs'
 $taskHas='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/HasSecondPasswordMessage.cs'
 $taskResult='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/SecondPasswordResultMessage.cs'
 $taskMin='legacy/decompiled/Utility-37359981d509/Utility/SQLDateTime.cs'
 $taskHarness='scripts/reverse/framework/SecondPasswordOracle.cs'
 $taskText=[IO.File]::ReadAllText((Join-Path $PWD $taskSource))
 function Get-Method([string]$Signature){
  $taskStart=$taskText.IndexOf($Signature,[StringComparison]::Ordinal);if($taskStart-lt0){throw 'Recovered method missing'}
  $taskBrace=$taskText.IndexOf('{',$taskStart);$taskDepth=1;$taskEnd=$taskBrace+1
  while($taskDepth-gt0 -and $taskEnd-lt$taskText.Length){if($taskText[$taskEnd]-eq'{'){$taskDepth++};if($taskText[$taskEnd]-eq'}'){$taskDepth--};$taskEnd++}
  if($taskDepth-ne0){throw 'Method boundary invalid'}
  return $taskText.Substring($taskStart,$taskEnd-$taskStart)
 }
 $taskMethods=@('public void RecordCheckSecondPasswordFail','public void QuerySecondPassword','private int GetHashVersion','public void CheckSecondPassword','public void RegisterSecondPassword')|ForEach-Object{Get-Method $_}
 $taskBegin=$taskText.IndexOf('hashFunc = new Func<string, string, string>[3]');$taskEnd=$taskText.IndexOf('};',$taskText.IndexOf('hashCompareFunc = new',$taskBegin))+2
 if($taskBegin-lt0-or$taskEnd-le$taskBegin){throw 'Hash arrays missing'}
 $taskCombined=[IO.File]::ReadAllText((Join-Path $PWD $taskHarness)).Replace('/*HASH_ARRAYS*/',$taskText.Substring($taskBegin,$taskEnd-$taskBegin)).Replace('/*METHODS*/',($taskMethods-join"`n")).Replace('DateTime.UtcNow','Clock.Now')
 [IO.File]::WriteAllText((Join-Path $PWD '.cache/framework/SecondPassword.cs'),$taskCombined,[Text.UTF8Encoding]::new($false))
 foreach($taskMessage in @($taskHas,$taskResult)){$taskName=[IO.Path]::GetFileName($taskMessage);$taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskMessage)).Replace('DateTime.UtcNow','global::Clock.Now');[IO.File]::WriteAllText((Join-Path $PWD ('.cache/framework/'+$taskName)),$taskCode,[Text.UTF8Encoding]::new($false))}
 $taskProfiles=@()
 foreach($taskArch in 'x86','x64'){
  $taskExe='.cache/framework/SecondPassword-'+$taskArch+'.exe'
  $taskArgs=@('C:\Program Files\dotnet\sdk\9.0.301\Roslyn\bincore\csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
  foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions','System.Data'){$taskArgs+=('/r:C:\Program Files (x86)\Reference Assemblies\Microsoft\Framework\.NETFramework\v4.8\'+$taskLib+'.dll')}
  $taskArgs+=@('.cache/framework/SecondPassword.cs',$taskEncrypt,$taskMin,'.cache/framework/HasSecondPasswordMessage.cs','.cache/framework/SecondPasswordResultMessage.cs')
  [IO.File]::WriteAllText((Join-Path $PWD '.cache/framework/IMessage.cs'),'namespace ServiceCore.EndPointNetwork { public interface IMessage {} }',[Text.UTF8Encoding]::new($false));$taskArgs+='.cache/framework/IMessage.cs'
  & 'C:\Program Files\dotnet\dotnet.exe' @taskArgs *> ('.cache/verification/second-password-'+$taskArch+'-build.log');if($LASTEXITCODE-ne0){throw 'Second-password oracle build failed; local log'}
  & (Join-Path $PWD $taskExe) *> ('.cache/verification/second-password-'+$taskArch+'.json');if($LASTEXITCODE-ne0){throw 'Second-password oracle failed; local log'}
  $taskData=Get-Content ('.cache/verification/second-password-'+$taskArch+'.json') -Raw|ConvertFrom-Json -DateKind String
  if($taskData.architecture-ne$taskArch-or$taskData.hashes.Count-ne63-or$taskData.queries.Count-ne126-or$taskData.checks.Count-ne90-or$taskData.registers.Count-ne21-or$taskData.nullable.Count-ne15-or$taskData.nullableChecks.Count-ne18){throw 'Second-password oracle counts differ'}
  $taskProfiles+=$taskData
 }
 $taskRuntime=@('C:/Windows/Microsoft.NET/Framework/v4.0.30319/mscorlib.dll','C:/Windows/Microsoft.NET/Framework64/v4.0.30319/mscorlib.dll')|ForEach-Object{[ordered]@{architecture=if($_ -like '*Framework64*'){'x64'}else{'x86'};sha256=(Get-FileHash $_).Hash.ToLowerInvariant();file_version=(Get-Item $_).VersionInfo.FileVersion}}
 $taskSources=@(@($taskSource,$taskEncrypt,$taskHas,$taskResult,$taskMin,$taskHarness)|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}})
 [ordered]@{profiles=$taskProfiles;framework_runtime=@($taskRuntime);sources=$taskSources;method='Extracted UserEntity methods/hash arrays unmodified except deterministic UtcNow substitution in isolated copies. Fake one-row context records mutations. Original StringEncrypter and SQLDateTime. Hash modes0/1/2; Framework architecture is explicit.';original_services_executed=$false}|ConvertTo-Json -Depth 12 -Compress|Set-Content tests/fixtures/framework-second-password.json -Encoding utf8NoBOM
 Write-Host 'Second-password oracle: x86/x64; each63hashes/8versions/126queries/90checks/21registrations/15nullable-read-failures/18null-check-cross-hashes; synthetic only.'
} finally {Pop-Location}
