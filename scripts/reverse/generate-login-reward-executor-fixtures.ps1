$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try {
 New-Item -ItemType Directory -Force -Path .cache/framework,.cache/verification|Out-Null
 $taskFeature='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore/FeatureMatrix.cs'
 $taskLogin='legacy/decompiled/LoginServiceCore-1600fb4b593d/LoginServiceCore.Processor/UserLoginProcessor.cs'
 $taskReward='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.HeroesContents/DormantReward.cs'
 $taskHarness='scripts/reverse/framework/LoginRewardExecutorOracle.cs'
 $taskText=[IO.File]::ReadAllText((Join-Path $PWD $taskFeature))
 $taskStart=$taskText.IndexOf('public static bool IsEnable(string featureName)');$taskEnd=$taskText.IndexOf('public static string GetString(string featureName)',$taskStart)
 $taskIs=$taskText.Substring($taskStart,$taskEnd-$taskStart)
 $taskStart=$taskText.IndexOf('public static int GetInteger(string featureName)');$taskEnd=$taskText.IndexOf('public static int? GetNullableInt',$taskStart)
 $taskInt=$taskText.Substring($taskStart,$taskEnd-$taskStart)
 $taskText=[IO.File]::ReadAllText((Join-Path $PWD $taskLogin))
 $taskStart=$taskText.IndexOf('if (ServiceCore.FeatureMatrix.IsEnable("friend_loyalty_system") && !withfriendevent')
 $taskEnd=$taskText.IndexOf('AsyncResultSync banSync',$taskStart)
 if($taskStart -lt 0 -or $taskEnd -lt $taskStart){throw 'Recovered existing-account block unavailable'}
 $taskBlock=$taskText.Substring($taskStart,$taskEnd-$taskStart)
 if([regex]::Matches($taskBlock,'DateTime\.UtcNow').Count -ne 5){throw 'Recovered clock count changed'}
 # Fixed, equal decision samples for this integration oracle; separate-clock decisions
 # are already covered by the 330-case source planner oracle.
 $taskBlock=$taskBlock.Replace('DateTime.UtcNow','Case.Now').Replace('ISingleResult<','IEnumerable<')
 $taskWrapper=@"
using System;
using System.Collections.Generic;
using System.Linq;
using ServiceCore.HeroesContents;
namespace ServiceCore { public class FeatureMatrix {
 public static string CurrentVer {get{return Case.CurrentVer;}}
 public static string GetString(string key){string value;return Case.Features.TryGetValue(key,out value)?value:"";}
 $taskIs
 $taskInt
} }
public static class RecoveredLogin { public static object Run(Context context) {
 bool withfriendevent=false,isDormant=false,isReturn=false;
 int? friendRecommendflag=context.Flag;int nexonSN=7;string nexonID="synthetic";
 var connected=new User();var userEntity=new Journal();
 $taskBlock
 return new {is_dormant=isDormant,is_return=isReturn};
} }
"@
 [IO.File]::WriteAllText((Join-Path $PWD '.cache/framework/LoginRewardExecutorRecovered.cs'),$taskWrapper,[Text.UTF8Encoding]::new($false))
 $taskProfiles=@()
 foreach($taskArch in 'x86','x64') {
  $taskExe='.cache/framework/LoginRewardExecutor-'+$taskArch+'.exe'
  $taskArgs=@('C:\Program Files\dotnet\sdk\9.0.301\Roslyn\bincore\csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
  foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions','System.Data.Linq'){$taskArgs+=('/r:C:\Program Files (x86)\Reference Assemblies\Microsoft\Framework\.NETFramework\v4.8\'+$taskLib+'.dll')}
  $taskArgs+='/r:C:\Program Files (x86)\Reference Assemblies\Microsoft\Framework\.NETFramework\v4.8\System.Data.dll'
  $taskArgs+=@('.cache/framework/LoginRewardExecutorRecovered.cs',$taskReward,$taskHarness)
  & 'C:\Program Files\dotnet\dotnet.exe' @taskArgs *> .cache/verification/login-reward-executor-oracle-build.log
  if($LASTEXITCODE -ne 0){Get-Content .cache/verification/login-reward-executor-oracle-build.log -Tail 8;throw 'Executor oracle build failed'}
  $taskRaw=& (Join-Path $PWD $taskExe) tests/fixtures/synthetic-reward-policy.json 2> .cache/verification/login-reward-executor-oracle-error.log
  if($LASTEXITCODE -ne 0){throw 'Executor oracle failed'}
  $taskData=$taskRaw|ConvertFrom-Json -DateKind String
  if($taskData.architecture -ne $taskArch){throw 'Architecture mismatch'}
  $taskProfiles+=$taskData
 }
 if(($taskProfiles[0].cases|ConvertTo-Json -Depth 12 -Compress) -ne ($taskProfiles[1].cases|ConvertTo-Json -Depth 12 -Compress)){throw 'Source architecture disagreement'}
 $taskFixture=[ordered]@{profiles=$taskProfiles;sources=@(@($taskFeature,$taskLogin,$taskReward,$taskHarness,'tests/fixtures/synthetic-reward-policy.json')|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});adaptations=@('five UtcNow expressions replaced by the same explicit synthetic clock','ISingleResult adapters represented as IEnumerable','context captures synthetic procedure calls; services not executed');source_services_executed=$false;sqlserver_persistent_writes=$false}
 [IO.File]::WriteAllText((Join-Path $PWD 'tests/fixtures/framework-login-reward-executor.json'),(($taskFixture|ConvertTo-Json -Depth 14).Replace("`r`n","`n")+"`n"),[Text.UTF8Encoding]::new($false))
 Write-Output ('Executor source oracle PASS; profiles=2; cases/profile='+$taskProfiles[0].cases.Count)
} finally {Pop-Location}
