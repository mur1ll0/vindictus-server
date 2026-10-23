param([string]$ProfileFile='.cache/private-content/skill-profile-v5.json',[string]$CatalogFile='.cache/private-content/item-catalog.json')
$ErrorActionPreference='Stop';$root=$PWD.Path
$profile=Get-Content $ProfileFile -Raw|ConvertFrom-Json -DateKind String
$catalog=Get-Content $CatalogFile -Raw|ConvertFrom-Json -DateKind String
# The skill profile carries no CurrentVer; its features are compared against the catalog's.
$current=@($catalog.features|Where-Object key -CEQ 'CurrentVer')
if($current.Count-ne1-or-not$current[0].value){throw 'CurrentVer profile absent'}
$known=@();foreach($name in 'feature_version_pairs','feature_catalog_version_pairs','feature_lobby_version_pairs','feature_frontend_version_pairs','feature_inventory_version_pairs','feature_appearance_version_pairs','feature_npc_version_pairs'){$known+=@(Get-Content ('src/domain/'+$name+'.json') -Raw|ConvertFrom-Json -DateKind String)}
$values=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$pairs=@(foreach($r in $profile.features){if($r.value-and$values.Add($r.value)-and-not@($known|Where-Object {$_.current-ceq$current[0].value-and$_.value-ceq$r.value}).Count){[ordered]@{current=$current[0].value;value=$r.value}}})
$inputText=[ordered]@{pairs=@($pairs)}|ConvertTo-Json -Depth 5 -Compress
$results=@()
foreach($arch in 'x86','x64'){
 $psi=[Diagnostics.ProcessStartInfo]::new((Join-Path $root ('.cache/framework/InitialInventoryOracle-'+$arch+'.exe')));$psi.ArgumentList.Add('--feature-pairs');$psi.UseShellExecute=$false;$psi.CreateNoWindow=$true;$psi.RedirectStandardInput=$true;$psi.RedirectStandardOutput=$true;$psi.RedirectStandardError=$true;$psi.StandardInputEncoding=[Text.UTF8Encoding]::new($false)
 $p=[Diagnostics.Process]::new();$p.StartInfo=$psi
 try{if(-not$p.Start()){throw 'Feature replay start failed'};$out=$p.StandardOutput.ReadToEndAsync();$err=$p.StandardError.ReadToEndAsync();$p.StandardInput.Write($inputText);$p.StandardInput.Close();$p.WaitForExit();$text=$out.GetAwaiter().GetResult();$errorText=$err.GetAwaiter().GetResult();if($p.ExitCode-ne0){[IO.File]::WriteAllText((Join-Path $root '.cache/verification/skill-feature-error.log'),$errorText);throw 'Feature replay failed;details local'};$results+=,($text|ConvertFrom-Json -DateKind String)}finally{$p.Dispose()}
}
if(($results[0]|ConvertTo-Json -Compress)-cne($results[1]|ConvertTo-Json -Compress)){throw 'Feature architecture mismatch'}
ConvertTo-Json -InputObject @($results[0]) -Depth 5 -Compress|Set-Content src/domain/feature_skill_version_pairs.json -Encoding utf8NoBOM
[ordered]@{profiles=@('Framework48-x86-en-US','Framework48-x64-en-US');pairs=@($results[0]);method='Recovered FeatureMatrix.IsEnable against candidate key, original CurrentVer.CompareTo, no service execution';source_sha256=(Get-FileHash legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore/FeatureMatrix.cs).Hash.ToLowerInvariant();harness_sha256=(Get-FileHash scripts/reverse/framework/InitialInventoryOracle.cs.in).Hash.ToLowerInvariant();generator_sha256=(Get-FileHash scripts/reverse/generate-skill-feature-pairs.ps1).Hash.ToLowerInvariant();original_services_executed=$false}|ConvertTo-Json -Depth 6 -Compress|Set-Content tests/fixtures/framework-skill-features.json -Encoding utf8NoBOM
Write-Output ('Skill rank feature comparisons:'+@($results[0]).Count+';zero architecture differences;original services not executed.')
