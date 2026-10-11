[CmdletBinding()]
param()
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try {
    $taskCompiler='C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe'
    if(-not (Test-Path -LiteralPath $taskCompiler)){throw '.NET Framework compiler required.'}
    New-Item -ItemType Directory -Force .cache\framework | Out-Null
    $taskProfiles=@()
    foreach($taskArchitecture in 'x86','x64'){
        $taskCompiler=if($taskArchitecture -eq 'x86'){'C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe'}else{'C:\Windows\Microsoft.NET\Framework64\v4.0.30319\csc.exe'}
        $taskOutputPath='.cache/framework/IdentityOracle-'+$taskArchitecture+'.exe'
        & $taskCompiler /nologo /noconfig /r:System.Web.Extensions.dll /r:System.dll ('/platform:'+$taskArchitecture) ('/out:'+$taskOutputPath) (Resolve-Path -LiteralPath 'scripts/reverse/framework/IdentityOracle.cs').Path
        if($LASTEXITCODE -ne 0){throw 'Synthetic Framework identity oracle compilation failed.'}
        [IO.File]::WriteAllText((Join-Path $taskProject ($taskOutputPath+'.config')), '<configuration><runtime><UseRandomizedStringHashAlgorithm enabled="0" /></runtime></configuration>')
        $taskRuns=@()
        for($taskRun=0;$taskRun -lt 2;$taskRun++){
            $taskOutput=& $taskOutputPath
            if($LASTEXITCODE -ne 0){throw 'Synthetic Framework identity oracle execution failed.'}
            $taskRuns+=($taskOutput -join "`n")
        }
        if($taskRuns[0] -ne $taskRuns[1]){throw 'Hash profile unstable across fresh processes.'}
        $taskData=$taskRuns[0] | ConvertFrom-Json
        if($taskData.architecture -ne $taskArchitecture){throw 'Oracle process architecture differs from requested profile.'}
        $taskProfiles+=$taskData
    }
    $taskSource='legacy/decompiled/FrontendServiceCore-6be475728091/FrontendServiceCore.States/PassportCheck.cs'
    $taskFixture=[ordered]@{
        method='Synthetic adaptation of PassportCheck identity expressions on .NET Framework BCL; originals never executed; random hashing explicitly disabled; two fresh processes compared per architecture'
        oracle_sha256=(Get-FileHash -Algorithm SHA256 -LiteralPath 'scripts/reverse/framework/IdentityOracle.cs').Hash.ToLowerInvariant()
        recovered_source_sha256=(Get-FileHash -Algorithm SHA256 -LiteralPath $taskSource).Hash.ToLowerInvariant()
        recovered_source=$taskSource
        implementations=$taskProfiles
    }
    [IO.File]::WriteAllText((Join-Path $taskProject 'tests/fixtures/framework-identity.json'),($taskFixture | ConvertTo-Json -Depth 12 -Compress),[Text.UTF8Encoding]::new($false))
    Write-Host ('Identity fixtures: '+$taskProfiles.Count+' architectures, '+$taskProfiles[0].profiles.Count+' cultures, '+$taskProfiles[0].profiles[0].cases.Count+' cases each; processes stable.')
} finally {Pop-Location}

