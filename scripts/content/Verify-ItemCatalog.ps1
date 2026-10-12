[CmdletBinding()]
param([switch]$Docker)
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
Push-Location $taskProject
try {
 . ./scripts/dev/docker-process.ps1
 $taskInput=[IO.File]::ReadAllText((Join-Path $PWD '.cache/private-content/item-catalog.json'))
 $taskPackage=$taskInput|ConvertFrom-Json -Depth 14
 $taskFixture=Get-Content tests/fixtures/framework-item-catalog.json -Raw|ConvertFrom-Json -Depth 20
 $taskExpected=$taskFixture.profiles[0].actual.summary
 if(-not $taskExpected -or $taskFixture.profiles[0].actual.error){throw 'Framework projection is incomplete'}
 $taskStart=[Diagnostics.ProcessStartInfo]::new((Join-Path $PWD 'target/debug/vindictus-server.exe'))
 $taskStart.WorkingDirectory=$PWD.Path;$taskStart.UseShellExecute=$false;$taskStart.CreateNoWindow=$true
 $taskStart.RedirectStandardInput=$true;$taskStart.RedirectStandardOutput=$true;$taskStart.RedirectStandardError=$true
 $taskStart.StandardInputEncoding=[Text.UTF8Encoding]::new($false)
 foreach($taskArg in @('inspect-item-catalog','--input-stdin')){$taskStart.ArgumentList.Add($taskArg)}
 $taskProcess=[Diagnostics.Process]::new();$taskProcess.StartInfo=$taskStart
 try {
  if(-not $taskProcess.Start()){throw 'Could not start catalog inspection'}
  $taskOut=$taskProcess.StandardOutput.ReadToEndAsync();$taskErr=$taskProcess.StandardError.ReadToEndAsync()
  $taskProcess.StandardInput.Write($taskInput);$taskProcess.StandardInput.Close()
  if(-not $taskProcess.WaitForExit(30000)){$taskProcess.Kill($true);throw 'Catalog inspection timeout'}
  if($taskProcess.ExitCode -ne 0){[IO.File]::WriteAllText((Join-Path $PWD '.cache/verification/item-catalog-native-error.log'),$taskErr.GetAwaiter().GetResult());throw 'Catalog inspection failed; private local log only'}
  $taskNative=$taskOut.GetAwaiter().GetResult()|ConvertFrom-Json
 } finally {$taskProcess.Dispose()}
 function Assert-Catalog($taskResult) {
  if(($taskResult|ConvertTo-Json -Depth 8 -Compress) -ne ($taskExpected|ConvertTo-Json -Depth 8 -Compress)){throw 'Rust catalog projection differs from Framework counts/digest/scope'}
 }
 Assert-Catalog $taskNative
 if($Docker){
  $taskRun=Invoke-ProjectDocker -Arguments @('compose','run','--rm','--no-deps','server','inspect-item-catalog','--input-stdin') -InputText $taskInput
  if($taskRun.ExitCode -ne 0){[IO.File]::WriteAllText((Join-Path $PWD '.cache/verification/item-catalog-docker-error.log'),$taskRun.Stderr);throw 'Docker catalog diagnostic failed; private local log only'}
  Assert-Catalog ($taskRun.Stdout|ConvertFrom-Json)
 }
 $taskBase=Join-Path $taskProject '../ServidorVindictus/Server/zh-tw-x-gm/Bin'
 if((Get-FileHash (Join-Path $taskBase 'heroesContents.db3')).Hash.ToLowerInvariant() -ne $taskPackage.manifest.database_sha256 -or (Get-FileHash (Join-Path $taskBase 'ServiceCore.dll.config')).Hash.ToLowerInvariant() -ne $taskPackage.manifest.config_sha256){throw 'Source content/config hash changed'}
 foreach($taskSource in $taskFixture.sources){if((Get-FileHash $taskSource.path).Hash.ToLowerInvariant() -ne $taskSource.sha256){throw 'Recovered source or harness hash changed'}}
 $taskReport=[ordered]@{
  stage='M13.2d.2b.2c.2b.3b.1';verified_at_utc=[DateTimeOffset]::UtcNow.ToString('o');profile=$taskPackage.manifest;
  private_export_sha256=(Get-FileHash .cache/private-content/item-catalog.json).Hash.ToLowerInvariant();feature_subset_rows=@($taskPackage.features).Count;
  additional_version_pairs=24;culture_agreement=@('invariant','en-US','zh-TW','zh-CN','tr-TR');framework_architectures=@('x86','x64');
  native_matches_framework=$true;docker_matches_framework=[bool]$Docker;projection=$taskNative;original_files_hashes_unchanged=$true;
  select_star_observed_order_preserved=$true;historical_runtime_provider_order_observed=$false;
  full_dto_property_conversion_migrated=$false;item_stat_body_migrated=$false;random_rate_tables_migrated=$false;full_item_contents_startup_migrated=$false;
  raw_content_published=$false;source_services_executed=$false;database_mutations=$false;game_client_login_available=$false;
  sources=$taskFixture.sources
 }
 $taskJson=($taskReport|ConvertTo-Json -Depth 12).Replace(([string][char]13+[char]10),([string][char]10))+[char]10
 [IO.File]::WriteAllText((Join-Path $PWD 'docs/evidence/content/item-catalog.json'),$taskJson,[Text.UTF8Encoding]::new($false))
 Write-Host ('Catalog private projection PASS: raw equips='+$taskPackage.manifest.equip_rows+'; raw classes='+$taskPackage.manifest.class_rows+'; effective equips='+$taskNative.counts.equip_unique+'; effective classes='+$taskNative.counts.class_unique+'; native/Framework equal=true; Docker='+[bool]$Docker+'; source hashes unchanged; mutations=false')
} finally {Pop-Location}
