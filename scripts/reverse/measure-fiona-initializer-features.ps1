param([string]$ProfileFile='.cache/private-content/fiona-initializer.json',[string]$OutputFile='.cache/private-content/fiona-initializer-reference-pairs.json')
$ErrorActionPreference='Stop';$root=$PWD.Path;$private=[IO.Path]::GetFullPath((Join-Path $root '.cache/private-content'))
foreach($file in @($ProfileFile,$OutputFile)){$full=[IO.Path]::GetFullPath((Join-Path $root $file));if(-not$full.StartsWith($private+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Real feature input/output must remain in private cache'}}
if((Get-Item $ProfileFile).Length-gt16MB){throw 'Private profile bound'}
$profile=Get-Content $ProfileFile -Raw|ConvertFrom-Json -DateKind String
$current=@($profile.features|Where-Object key -CEQ 'CurrentVer');if($current.Count-ne1-or-not$current[0].value){throw 'Current version absent'}
$values=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$pairs=@(foreach($r in $profile.features){if(($profile.enabled_keys-ccontains$r.key)-and$r.value-and$values.Add($r.value)){[ordered]@{current=$current[0].value;value=$r.value}}})
if($pairs.Count-gt128){throw 'Private comparison bound'}
$inputText=[ordered]@{pairs=$pairs}|ConvertTo-Json -Depth 4 -Compress;$results=@()
foreach($arch in @('x86','x64')){
 $psi=[Diagnostics.ProcessStartInfo]::new((Join-Path $root ('.cache/framework/InitialInventoryOracle-'+$arch+'.exe')));$psi.ArgumentList.Add('--feature-pairs');$psi.UseShellExecute=$false;$psi.CreateNoWindow=$true;$psi.RedirectStandardInput=$true;$psi.RedirectStandardOutput=$true;$psi.RedirectStandardError=$true;$psi.StandardInputEncoding=[Text.UTF8Encoding]::new($false)
 $process=[Diagnostics.Process]::new();$process.StartInfo=$psi
 try{if(-not$process.Start()){throw 'Private feature oracle start failed'};$out=$process.StandardOutput.ReadToEndAsync();$err=$process.StandardError.ReadToEndAsync();$process.StandardInput.Write($inputText);$process.StandardInput.Close();$process.WaitForExit();$text=$out.GetAwaiter().GetResult();$errorText=$err.GetAwaiter().GetResult();if($process.ExitCode-ne0){[IO.File]::WriteAllText((Join-Path $private ('initializer-feature-error-'+$arch+'.log')),$errorText);throw 'Private feature oracle failed;details kept private'};$results+=,@($text|ConvertFrom-Json -DateKind String)}finally{$process.Dispose()}
}
if(($results[0]|ConvertTo-Json -Depth 5 -Compress)-cne($results[1]|ConvertTo-Json -Depth 5 -Compress)){throw 'Private feature architecture difference'}
$source='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore/FeatureMatrix.cs'
$report=[ordered]@{format_version=1;source='Framework.FeatureMatrix.IsEnable';profile_sha256=(Get-FileHash $ProfileFile).Hash.ToLowerInvariant();database_sha256=$profile.database_sha256;config_sha256=$profile.config_sha256;feature_matrix_sha256=(Get-FileHash $source).Hash.ToLowerInvariant();profiles=@('Framework48-x86-en-US','Framework48-x64-en-US');pairs=@($results[0])}
$json=$report|ConvertTo-Json -Depth 6
[IO.File]::WriteAllText((Join-Path $root $OutputFile),$json.Replace("`r`n","`n")+"`n",[Text.UTF8Encoding]::new($false))
Write-Output ('Private feature evidence:comparisons='+$pairs.Count+';x86=x64;payload only in protected cache;no public feature values.')
