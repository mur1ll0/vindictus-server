param()
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try {
 New-Item -ItemType Directory -Force .cache/framework,.cache/verification|Out-Null
 $taskSource='legacy/decompiled/LoginServiceCore-1600fb4b593d/LoginServiceCore.Processor/UserLoginProcessor.cs'
 $taskReward='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.HeroesContents/DormantReward.cs'
 $taskHarness='scripts/reverse/framework/LoginEffectsOracle.cs'
 $taskText=[IO.File]::ReadAllText((Join-Path $PWD $taskSource))
 $taskNewStart=$taskText.IndexOf('if (ServiceCore.FeatureMatrix.IsEnable("friend_loyalty_system") && !friendRecommendflag.HasValue)')
 $taskNewEnd=$taskText.IndexOf('if (ServiceCore.FeatureMatrix.IsEnable("PassportImmigrator"))',$taskNewStart)
 $taskStart=$taskText.IndexOf('if (ServiceCore.FeatureMatrix.IsEnable("friend_loyalty_system") && !withfriendevent && !friendRecommendflag.HasValue)')
 $taskEnd=$taskText.IndexOf('AsyncResultSync banSync',$taskStart)
 if($taskNewStart-lt0-or$taskNewEnd-le$taskNewStart-or$taskStart-lt0-or$taskEnd-le$taskStart){throw 'Recovered markers unavailable'}
 $taskNewBlock=$taskText.Substring($taskNewStart,$taskNewEnd-$taskNewStart)
 $taskBlock=$taskText.Substring($taskStart,$taskEnd-$taskStart)
 $taskRawHash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($taskBlock))).ToLowerInvariant()
 if([regex]::Matches($taskBlock,'DateTime.UtcNow').Count-ne5){throw 'Unexpected clock call count'}
 foreach($taskClock in @('Clock.Loyalty','Clock.DormantElapsed','Clock.LatestElapsed','Clock.SetDormant','Clock.SetEvent')) {
  $taskPos=$taskBlock.IndexOf('DateTime.UtcNow')
  $taskBlock=$taskBlock.Remove($taskPos,15).Insert($taskPos,$taskClock)
 }
 $taskPredicate=[regex]::Match($taskText,'Func<DormantReward, bool> predicate = ([^\r\n]+);').Groups[1].Value
 if(-not$taskPredicate){throw 'Reward predicate unavailable'}
 $taskInsert=[regex]::Match($taskText,'context.User.InsertOnSubmit\(connected\);\s*context.SubmitChanges\(\);').Value
 if(-not$taskInsert){throw 'Insert barrier source calls unavailable'}
 $taskAchievement=[regex]::Match($taskText,'context.ReplaceUserAchievementNIDtoUID\(nexonID, nexonSN\);').Value
 if(-not$taskAchievement){throw 'Achievement source call unavailable'}
 $taskWrapper=@"
using System;
using System.Collections.Generic;
using System.Linq;
using System.Data.Linq;
using ServiceCore.HeroesContents;
public static class RecoveredEffects {
 public static object Run() {
  var context=new Context();var userEntity=new UserEntity();var connected=new User();
  int nexonSN=1;string nexonID="Synthetic";int? friendRecommendflag=Case.Current.flag;
  bool withfriendevent=false,isDormant=false,isReturn=false;
  if(Case.Current.new_user) {
   $taskNewBlock
   $taskInsert
   $taskAchievement
  }
  $taskBlock
  return new { actions=context.Actions,is_dormant=isDormant,is_return=isReturn };
 }
 public static bool Reward(int level,int min,int max) {
  var character=new GetCharacters결과{Level=level};
  var reward=new DormantReward{MinLevel=min,MaxLevel=max,Feature="synthetic_reward"};
  Func<DormantReward,bool> predicate=$taskPredicate;
  return predicate(reward);
 }
}
"@
 $taskRecovered='.cache/framework/LoginEffectsRecovered.cs'
 [IO.File]::WriteAllText((Join-Path $PWD $taskRecovered),$taskWrapper,[Text.UTF8Encoding]::new($false))
 $taskProfiles=@()
 foreach($taskArch in 'x86','x64') {
  $taskExe='.cache/framework/LoginEffects-'+$taskArch+'.exe'
  $taskArgs=@('C:\Program Files\dotnet\sdk\9.0.301\Roslyn\bincore\csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
  foreach($taskLibrary in 'mscorlib','System','System.Core','System.Web.Extensions') {
   $taskArgs+=('/r:C:\Program Files (x86)\Reference Assemblies\Microsoft\Framework\.NETFramework\v4.8\'+$taskLibrary+'.dll')
  }
  $taskArgs+=@($taskRecovered,$taskReward,$taskHarness)
  & 'C:\Program Files\dotnet\dotnet.exe' @taskArgs *> .cache/verification/login-effects-oracle-build.log
  if($LASTEXITCODE-ne0){Get-Content .cache/verification/login-effects-oracle-build.log -Tail 8;throw 'Login effects oracle compilation failed'}
  $taskRaw=& (Join-Path $PWD $taskExe)
  if($LASTEXITCODE-ne0){throw 'Login effects oracle execution failed'}
  $taskData=$taskRaw|ConvertFrom-Json -DateKind String
  if($taskData.architecture-ne$taskArch){throw 'Architecture mismatch'}
  $taskProfiles+=$taskData
 }
 $taskSources=@($taskSource,$taskReward,$taskHarness,'legacy/decompiled/LoginServiceCore-1600fb4b593d/LoginServiceCore/UserDBDataContext.cs')
 $taskHashes=@($taskSources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
 [ordered]@{
  profiles=$taskProfiles;sources=$taskHashes;original_services_executed=$false;database_effects_executed=$false;
  recovered_effect_block_sha256=$taskRawHash;generated_wrapper_sha256=(Get-FileHash $taskRecovered).Hash.ToLowerInvariant();
  method='Recovered new-friend/existing-loyalty/dormancy blocks and reward predicate; only five UtcNow reads substituted with explicit independent clock samples. Context records ordered intents, returns selected synthetic observations and empty character rows; SQL effect implementations not emulated.';
 }|ConvertTo-Json -Depth 12|Set-Content tests/fixtures/framework-login-effects.json -Encoding utf8NoBOM
 Write-Host ('Login effects oracle: profiles=2; cases/profile='+$taskProfiles[0].cases.Count+'; reward predicates/profile='+$taskProfiles[0].rewards.Count+'; source services/database writes=false')
} finally { Pop-Location }

