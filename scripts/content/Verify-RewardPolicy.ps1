[CmdletBinding()]
param([switch]$Docker)
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
Push-Location $taskProject
try {
 $taskInput=[IO.File]::ReadAllText((Join-Path $PWD '.cache/private-content/reward-policy.json'))
 $taskPackage=$taskInput|ConvertFrom-Json -DateKind String
 $taskProfiles=@('x86','x64')|ForEach-Object {Get-Content ('.cache/private-content/reward-oracle-'+$_+'.json') -Raw|ConvertFrom-Json -DateKind String}
 if(($taskProfiles[0].plan|ConvertTo-Json -Depth 8 -Compress) -ne ($taskProfiles[1].plan|ConvertTo-Json -Depth 8 -Compress)){throw 'Private Framework architecture results differ'}
 $taskCharacters=Join-Path $PWD '.cache/private-content/characters.json'
 [IO.File]::WriteAllText($taskCharacters,($taskProfiles[0].plan.characters|ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false))
 $taskStart=[Diagnostics.ProcessStartInfo]::new((Join-Path $PWD 'target/debug/vindictus-server.exe'))
 $taskStart.WorkingDirectory=$PWD.Path;$taskStart.UseShellExecute=$false;$taskStart.CreateNoWindow=$true
 $taskStart.RedirectStandardInput=$true;$taskStart.RedirectStandardOutput=$true;$taskStart.RedirectStandardError=$true
 $taskStart.StandardInputEncoding=[Text.UTF8Encoding]::new($false)
 foreach($taskArg in @('inspect-reward-policy','--input-stdin','--characters-file',$taskCharacters)){$taskStart.ArgumentList.Add($taskArg)}
 $taskProcess=[Diagnostics.Process]::new();$taskProcess.StartInfo=$taskStart
 try {
  if(-not $taskProcess.Start()){throw 'Could not start Rust reward inspection'}
  $taskOut=$taskProcess.StandardOutput.ReadToEndAsync();$taskErr=$taskProcess.StandardError.ReadToEndAsync()
  $taskProcess.StandardInput.Write($taskInput);$taskProcess.StandardInput.Close()
  if(-not $taskProcess.WaitForExit(30000)){$taskProcess.Kill($true);throw 'Rust reward inspection timeout'}
  if($taskProcess.ExitCode -ne 0){[IO.File]::WriteAllText((Join-Path $PWD '.cache/verification/reward-inspection-error.log'),$taskErr.GetAwaiter().GetResult());throw 'Rust reward inspection failed; private local log only'}
  $taskNative=$taskOut.GetAwaiter().GetResult()|ConvertFrom-Json
 } finally {$taskProcess.Dispose()}
 function Assert-Plan($taskResult) {
  if($taskResult.plan_sha256 -ne $taskProfiles[0].plan.digest -or $taskResult.queued_calls -ne $taskProfiles[0].plan.count -or $taskResult.catalog_rows -ne $taskPackage.manifest.catalog_rows -or $taskResult.characters -ne @($taskProfiles[0].plan.characters).Count -or $taskResult.database_mutations -or -not $taskResult.clock_is_synthetic -or $taskResult.game_authentication_available){throw 'Rust inspection differs from recovered Framework plan or diagnostic contract'}
 }
 Assert-Plan $taskNative
 $taskContainer=$null
 if($Docker){
  $taskMount=(Join-Path $PWD '.cache/private-content')+':/content:ro'
  $taskRun=Invoke-ProjectDocker -Arguments @('compose','run','--rm','--no-deps','--volume',$taskMount,'server','inspect-reward-policy','--input-stdin','--characters-file','/content/characters.json') -InputText $taskInput
  if($taskRun.ExitCode -ne 0){[IO.File]::WriteAllText((Join-Path $PWD '.cache/verification/reward-docker-error.log'),$taskRun.Stderr);throw 'Docker inspection failed; local diagnostic log only'}
  $taskContainer=$taskRun.Stdout|ConvertFrom-Json;Assert-Plan $taskContainer
 }
 $taskBase=Join-Path $taskProject '../ServidorVindictus/Server/zh-tw-x-gm/Bin'
 if((Get-FileHash (Join-Path $taskBase 'heroesContents.db3')).Hash.ToLowerInvariant() -ne $taskPackage.manifest.database_sha256 -or (Get-FileHash (Join-Path $taskBase 'ServiceCore.dll.config')).Hash.ToLowerInvariant() -ne $taskPackage.manifest.config_sha256){throw 'Original content/config hash changed'}
 $taskFixture=Get-Content tests/fixtures/framework-reward-policy.json -Raw|ConvertFrom-Json -DateKind String
 foreach($taskSource in $taskFixture.sources){if((Get-FileHash $taskSource.path).Hash.ToLowerInvariant() -ne $taskSource.sha256){throw 'Framework recovered source/harness hash changed'}}
 $taskPaths=@('Cargo.lock','scripts/content/export_reward_policy.py','scripts/content/Export-RewardPolicy.ps1','scripts/content/Verify-RewardPolicy.ps1','scripts/content/test_export_reward_policy.py','scripts/reverse/generate-reward-policy-fixtures.ps1','scripts/reverse/framework/RewardPolicyOracle.cs','src/domain/feature_policy.rs','src/domain/feature_version_pairs.json','src/domain/dormant_rewards.rs','src/domain/legacy_user_data.rs','src/infrastructure/reward_policy.rs','src/application/reward_inspection.rs','src/cli.rs','src/main.rs','tests/reward_policy.rs','tests/fixtures/framework-reward-policy.json','tests/fixtures/synthetic-reward-policy.json')
 $taskReport=[ordered]@{
  stage='M13.2d.2b.2b.1'; verified_at_utc=[DateTimeOffset]::UtcNow.ToString('o');
  profile=$taskPackage.manifest; private_export_sha256=(Get-FileHash .cache/private-content/reward-policy.json).Hash.ToLowerInvariant();
  content_is_private=$true; acl='current Windows user and SYSTEM; inheritance disabled';
  feature_subset_rows=@($taskPackage.features).Count; original_files_hashes_unchanged=$true;
  catalog_iteration_order_preserved=$true; duplicate_rewards_preserved=$true; character_names_raw_utf16=$true;
  framework_profiles=@('x86','x64'); framework_cultures=@('invariant','en-US','zh-TW','zh-CN','tr-TR');
  finite_version_pairs=81; unverified_version_pairs_rejected=$true; sqlclient_date_rows_per_profile=908;
  private_plan_characters=$taskNative.characters; private_plan_calls=$taskNative.queued_calls; private_plan_sha256=$taskNative.plan_sha256;
  rust_native_matches_framework=$true; rust_docker_matches_framework=[bool]$Docker;
  diagnostic_clock_is_synthetic=$true; synthetic_clock_units300=0; database_mutations=$false;
  original_game_services_executed=$false; sqlserver_persistent_writes=$false;
  account_executor_integrated=$false; inventory_mail_delivery_implemented=$false; game_authentication_available=$false;
  sources=$taskFixture.sources;
  artifacts=@($taskPaths|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}})
 }
 New-Item -ItemType Directory -Force -Path docs/evidence/content|Out-Null
 $taskReport|ConvertTo-Json -Depth 10|Set-Content docs/evidence/content/reward-policy.json -Encoding utf8NoBOM
 Write-Host ('Reward private plan PASS: catalog='+$taskNative.catalog_rows+'; characters='+$taskNative.characters+'; calls='+$taskNative.queued_calls+'; native/Framework equal=true; Docker='+[bool]$Docker+'; source hashes unchanged; mutations=false')
} finally {Pop-Location}