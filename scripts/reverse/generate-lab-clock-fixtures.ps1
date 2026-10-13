$ErrorActionPreference='Stop'
$base=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$dir=Join-Path $base '.cache/lab-clock-oracle'
New-Item -ItemType Directory -Force $dir|Out-Null
$source=Join-Path $PSScriptRoot 'framework/LabClockOracle.cs'
$csc=Join-Path $env:WINDIR 'Microsoft.NET/Framework/v4.0.30319/csc.exe'
$profiles=@()
foreach($architecture in @('x86','x64')) {
 $exe=Join-Path $dir ('clock-'+$architecture+'.exe')
 & $csc /nologo /target:exe ('/platform:'+$architecture) /r:System.Web.Extensions.dll ('/out:'+$exe) $source *> (Join-Path $dir ('build-'+$architecture+'.log'))
 if($LASTEXITCODE-ne0){throw 'Own Framework clock oracle build failed; local diagnostics only'}
 $json=& $exe
 if($LASTEXITCODE-ne0){throw 'Own Framework clock oracle failed'}
 $profile=$json|ConvertFrom-Json -DateKind String
 if($profile.architecture-cne$architecture-or$profile.cases.Count-ne140){throw 'Clock oracle count/profile mismatch'}
 $profiles+=$profile
}
$left=$profiles[0].cases|ConvertTo-Json -Depth 5 -Compress
$right=$profiles[1].cases|ConvertTo-Json -Depth 5 -Compress
if($left-cne$right){throw 'Framework clock profiles disagree'}
[ordered]@{oracle='own Framework DateTime ticks/AddTicks; no original services executed';oracle_sha256=(Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash.ToLowerInvariant();architectures=@('x86','x64');cases_per_architecture=140;disagreements=0;cases=$profiles[0].cases}|ConvertTo-Json -Depth 6|Set-Content (Join-Path $base 'tests/fixtures/framework-lab-clock.json') -Encoding utf8NoBOM
Write-Host 'Clock oracle PASS:140cases per architecture x2; no disagreements; own executable only.'
