param([string]$Dotnet='C:\Program Files\dotnet\dotnet.exe',[string]$Compiler='C:\Program Files\dotnet\sdk\9.0.301\Roslyn\bincore\csc.dll')
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try {
    $taskSource='legacy/decompiled/LoginServiceCore-1600fb4b593d/LoginServiceCore/LoginService.cs'
    $taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskSource))
    $taskStart=$taskCode.IndexOf('public KeyValuePair<string, int> CheckLastLoginIP(')
    $taskEnd=$taskCode.IndexOf('public void UpdateLastLoginIP(',$taskStart)
    if($taskStart-lt0 -or $taskEnd-lt0){throw 'Recovered IP method not found'}
    $taskIp=$taskCode.Substring($taskStart,$taskEnd-$taskStart).Trim()
    if([regex]::Matches($taskIp,'DateTime.Now').Count-ne2){throw 'Unexpected recovered clock samples'}
    $taskIp=$taskIp.Replace('DateTime.Now.AddMinutes','Clock.SetNow.AddMinutes').Replace('Subtract(DateTime.Now)','Subtract(Clock.CheckNow)')
    $taskStart=$taskCode.IndexOf('GetMemberState = delegate(')
    $taskEnd=$taskCode.IndexOf('if (FeatureMatrix.IsEnable("PassportImmigrator")',$taskStart)
    $taskMember=$taskCode.Substring($taskStart,$taskEnd-$taskStart).Trim()
    $taskHarness=[IO.File]::ReadAllText((Join-Path $PWD 'scripts/reverse/framework/LoginPolicyOracle.cs')).Replace('/* MEMBER_STATE */',$taskMember).Replace('/* CHECK_IP */',$taskIp)
    [IO.File]::WriteAllText((Join-Path $PWD '.cache/framework/LoginPolicy.cs'),$taskHarness,[Text.UTF8Encoding]::new($false))
    $taskProfiles=@()
    foreach($taskArch in @('x86','x64')){
        $taskArgs=@($Compiler,'/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$taskArch),('/out:.cache/framework/LoginPolicy-'+$taskArch+'.exe'))
        foreach($taskLibrary in 'mscorlib','System','System.Core','System.Data.Linq','System.Web.Extensions'){
            $taskArgs+=('/r:C:\Program Files (x86)\Reference Assemblies\Microsoft\Framework\.NETFramework\v4.8\'+$taskLibrary+'.dll')
        }
        $taskArgs+=@('.cache/framework/LoginPolicy.cs','legacy/decompiled/LoginServiceCore-1600fb4b593d/LoginServiceCore/User.cs','legacy/decompiled/LoginServiceCore-1600fb4b593d/LoginServiceCore/GetUserBanInfo결과.cs')
        & $Dotnet @taskArgs *> .cache/verification/login-policy-oracle-build.log
        if($LASTEXITCODE-ne0){Get-Content .cache/verification/login-policy-oracle-build.log -Tail 10;throw 'Policy oracle build failed'}
        $taskRaw=& (Join-Path $PWD ('.cache/framework/LoginPolicy-'+$taskArch+'.exe'))
        if($LASTEXITCODE-ne0){throw 'Policy oracle failed'}
        $taskProfiles+=($taskRaw|ConvertFrom-Json -DateKind String)
    }
    $taskResult=[ordered]@{profiles=$taskProfiles;source_sha256=(Get-FileHash $taskSource).Hash.ToLowerInvariant();harness_sha256=(Get-FileHash scripts/reverse/framework/LoginPolicyOracle.cs).Hash.ToLowerInvariant();generated_at_utc=[DateTimeOffset]::UtcNow.ToString('o');original_services_executed=$false}
    $taskResult|ConvertTo-Json -Depth 8|Set-Content tests/fixtures/framework-login-policy.json -Encoding utf8NoBOM
    Write-Host ('Recovered policy oracle: profiles='+$taskProfiles.Count+'; IP cases/profile='+$taskProfiles[0].ip_cases.Count+'; ban cases/profile='+$taskProfiles[0].ban_cases.Count)
} finally {Pop-Location}
