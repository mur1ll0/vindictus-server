param()
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try {
 New-Item -ItemType Directory -Force -Path .cache/framework,.cache/verification|Out-Null
 $taskFeature='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore/FeatureMatrix.cs'
 $taskLogin='legacy/decompiled/LoginServiceCore-1600fb4b593d/LoginServiceCore.Processor/UserLoginProcessor.cs'
 $taskReward='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.HeroesContents/DormantReward.cs'
 $taskHarness='scripts/reverse/framework/RewardPolicyOracle.cs'
 $taskText=[IO.File]::ReadAllText((Join-Path $PWD $taskFeature))
 $taskIsStart=$taskText.IndexOf('public static bool IsEnable(string featureName)');$taskIsEnd=$taskText.IndexOf('public static string GetString(string featureName)',$taskIsStart)
 $taskIntStart=$taskText.IndexOf('public static int GetInteger(string featureName)');$taskIntEnd=$taskText.IndexOf('public static int? GetNullableInt',$taskIntStart)
 if($taskIsStart -lt 0 -or $taskIntStart -lt 0){throw 'Recovered feature blocks unavailable'}
 $taskIsBlock=$taskText.Substring($taskIsStart,$taskIsEnd-$taskIsStart);$taskIntBlock=$taskText.Substring($taskIntStart,$taskIntEnd-$taskIntStart)
 $taskText=[IO.File]::ReadAllText((Join-Path $PWD $taskLogin));$taskStart=$taskText.IndexOf('foreach (GetCharacters결과 character in characters)');$taskEnd=$taskText.IndexOf('isDormant = true;',$taskStart)
 if($taskStart -lt 0 -or $taskEnd -lt $taskStart){throw 'Recovered reward loop unavailable'}
 $taskLoop=$taskText.Substring($taskStart,$taskEnd-$taskStart)
 $taskWrapper=@"
using System;
using System.Collections.Generic;
using System.Linq;
using ServiceCore.HeroesContents;
namespace ServiceCore { public class FeatureMatrix {
 public static string CurrentVer { get { return Case.CurrentVer; } }
 public static string GetString(string key) { string value;return Case.Features.TryGetValue(key,out value)?value:""; }
 $taskIsBlock
 $taskIntBlock
} }
public static class RecoveredRewards {
 public static Context Run(IEnumerable<GetCharacters결과> characters) {
  var context=new Context();
  $taskLoop
  return context;
 }
}
"@
 [IO.File]::WriteAllText((Join-Path $PWD '.cache/framework/RewardPolicyRecovered.cs'),$taskWrapper,[Text.UTF8Encoding]::new($false))
 $taskSettings=@{};foreach($taskLine in [IO.File]::ReadAllLines((Join-Path $PWD '.env'))){if($taskLine -match '^([A-Z_]+)=(.*)$'){$taskSettings[$Matches[1]]=$Matches[2]}}
 $taskPassword=if($taskSettings.ContainsKey('MSSQL_SA_PASSWORD')){$taskSettings.MSSQL_SA_PASSWORD}else{$taskSettings.SQLSERVER_SA_PASSWORD}
 if(-not $taskPassword){throw 'Reference SQL password setting unavailable'}
 $taskBuilder=[System.Data.SqlClient.SqlConnectionStringBuilder]::new();$taskBuilder['Data Source']='127.0.0.1,14333';$taskBuilder['Initial Catalog']='study_heroes';$taskBuilder['User ID']='sa';$taskBuilder['Password']=$taskPassword;$taskBuilder['TrustServerCertificate']=$true;$taskBuilder['Encrypt']=$false;$taskBuilder['Connect Timeout']=10
 $env:REWARD_ORACLE_SQLSERVER=$taskBuilder.ConnectionString
 $taskProfiles=@()
 foreach($taskArch in 'x86','x64'){
  $taskExe='.cache/framework/RewardPolicy-'+$taskArch+'.exe';$taskArgs=@('C:\Program Files\dotnet\sdk\9.0.301\Roslyn\bincore\csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
  foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions','System.Data','System.Data.Linq'){$taskArgs+=('/r:C:\Program Files (x86)\Reference Assemblies\Microsoft\Framework\.NETFramework\v4.8\'+$taskLib+'.dll')}
  $taskArgs+=@('.cache/framework/RewardPolicyRecovered.cs',$taskReward,$taskHarness)
  & 'C:\Program Files\dotnet\dotnet.exe' @taskArgs *> .cache/verification/reward-oracle-build.log
  if($LASTEXITCODE -ne 0){Get-Content .cache/verification/reward-oracle-build.log -Tail 7;throw 'Reward oracle compilation failed'}
  foreach($taskKind in 'actual','synthetic'){
   $taskInput=if($taskKind -eq 'actual'){'.cache/private-content/reward-policy.json'}else{'tests/fixtures/synthetic-reward-policy.json'}
   $taskRaw=& (Join-Path $PWD $taskExe) $taskInput 2> .cache/verification/reward-oracle-error.log
   if($LASTEXITCODE -ne 0){throw 'Reward oracle failed; private diagnostic log only'}
   $taskData=$taskRaw|ConvertFrom-Json -DateKind String
   if($taskData.architecture -ne $taskArch){throw 'Framework architecture mismatch'}
   if($taskKind -eq 'actual'){$taskData|ConvertTo-Json -Depth 10|Set-Content ('.cache/private-content/reward-oracle-'+$taskArch+'.json') -Encoding utf8NoBOM}else{$taskProfiles+=$taskData}
  }
 }
 $taskPairs=@();foreach($taskGroup in ($taskProfiles[0].version_pairs|Group-Object current,value)){
  if(@($taskGroup.Group.enabled|Sort-Object -Unique).Count -ne 1){throw 'Version comparison differs across culture; unresolved profile'}
  $taskPairs+=[ordered]@{current=$taskGroup.Group[0].current;value=$taskGroup.Group[0].value;enabled=$taskGroup.Group[0].enabled}
 }
 if(($taskProfiles[0].version_pairs|ConvertTo-Json -Compress) -ne ($taskProfiles[1].version_pairs|ConvertTo-Json -Compress)){throw 'Version comparison architecture mismatch'}
 [IO.File]::WriteAllText((Join-Path $PWD 'src/domain/feature_version_pairs.json'),(($taskPairs|ConvertTo-Json -Depth 4).Replace("`r`n","`n")+"`n"),[Text.UTF8Encoding]::new($false))
 $taskPublicFixture=[ordered]@{profiles=$taskProfiles;sources=@(@($taskFeature,$taskLogin,$taskReward,$taskHarness)|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});source_services_executed=$false;sqlserver_persistent_writes=$false;query='SELECT synthetic datetime binary constants; SqlClient.GetDateTime compared to SqlDateTime.Value';version_comparison='Finite verified pairs, culture agreement invariant/en-US/zh-TW/zh-CN/tr-TR; unsupported pairs must fail closed.'}
 [IO.File]::WriteAllText((Join-Path $PWD 'tests/fixtures/framework-reward-policy.json'),(($taskPublicFixture|ConvertTo-Json -Depth 12).Replace("`r`n","`n")+"`n"),[Text.UTF8Encoding]::new($false))
 Write-Host ('Reward oracle PASS: Framework x86/x64; culture profiles=5; version pairs='+$taskPairs.Count+'; SqlClient dates/profile='+$taskProfiles[0].sqlclient_rows+'; original services/persistent writes=false; actual content results private.')
}finally {Remove-Item Env:REWARD_ORACLE_SQLSERVER -ErrorAction SilentlyContinue;Pop-Location}
