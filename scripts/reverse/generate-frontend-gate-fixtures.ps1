param()
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try {
    New-Item -ItemType Directory -Force .cache/framework,.cache/verification | Out-Null
    $taskSource='legacy/decompiled/FrontendServiceCore-6be475728091/FrontendServiceCore.States/PassportCheck.cs'
    $taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskSource))
    $taskStart=$taskCode.IndexOf('if ((base.Parent.Permissions & Permissions.Login) == 0)')
    $taskEnd=$taskCode.IndexOf('base.Parent.UserInfo.Age = (byte)checkPassport.Age;', $taskStart)
    if($taskStart-lt0 -or $taskEnd-lt0){throw 'Recovered gates not found'}
    $taskGate=$taskCode.Substring($taskStart,$taskEnd-$taskStart)
    $taskGameClient='legacy/decompiled/FrontendServiceCore-6be475728091/FrontendServiceCore/GameClient.cs'
    $taskOtp=[regex]::Match([IO.File]::ReadAllText((Join-Path $PWD $taskGameClient)),'public bool IsOTP => ([^;]+);')
    if(-not $taskOtp.Success){throw 'Recovered OTP expression not found'}
    $taskHarnessPath='scripts/reverse/framework/FrontendGateOracle.cs'
    $taskHarness=[IO.File]::ReadAllText((Join-Path $PWD $taskHarnessPath)).Replace('/* RECOVERED_GATE */',$taskGate).Replace('/* RECOVERED_OTP */',$taskOtp.Groups[1].Value)
    [IO.File]::WriteAllText((Join-Path $PWD '.cache/framework/FrontendGate.cs'),$taskHarness,[Text.UTF8Encoding]::new($false))
    $taskProfiles=@()
    foreach($taskArchitecture in 'x86','x64') {
        $taskExe='.cache/framework/FrontendGate-'+$taskArchitecture+'.exe'
        $taskArgs=@('C:\Program Files\dotnet\sdk\9.0.301\Roslyn\bincore\csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$taskArchitecture),('/out:'+$taskExe))
        foreach($taskLibrary in 'mscorlib','System','System.Core','System.Web.Extensions'){
            $taskArgs+=('/r:C:\Program Files (x86)\Reference Assemblies\Microsoft\Framework\.NETFramework\v4.8\'+$taskLibrary+'.dll')
        }
        $taskArgs+=@('.cache/framework/FrontendGate.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.LoginServiceOperations/Permissions.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/LoginFailMessage.cs')
        & 'C:\Program Files\dotnet\dotnet.exe' @taskArgs *> .cache/verification/frontend-gate-oracle-build.log
        if($LASTEXITCODE-ne0){Get-Content .cache/verification/frontend-gate-oracle-build.log -Tail 10;throw 'Frontend oracle compilation failed'}
        $taskRaw=& (Join-Path $PWD $taskExe)
        if($LASTEXITCODE-ne0){throw 'Frontend oracle execution failed'}
        $taskProfile=$taskRaw | ConvertFrom-Json
        if($taskProfile.architecture-ne$taskArchitecture){throw 'Architecture mismatch'}
        $taskProfiles+=$taskProfile
    }
    $taskSources=@($taskSource,$taskGameClient,'legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.LoginServiceOperations/Permissions.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/LoginFailMessage.cs',$taskHarnessPath)
    $taskHashes=@($taskSources | ForEach-Object { [ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()} })
    [ordered]@{profiles=$taskProfiles;sources=$taskHashes;original_services_executed=$false;method='Unchanged recovered frontend gates/OTP expression and original enums; logging and parent/operation shims only'} | ConvertTo-Json -Depth 8 | Set-Content tests/fixtures/framework-frontend-gate.json -Encoding utf8NoBOM
    Write-Host ('Frontend oracle: profiles=2; gates/profile='+$taskProfiles[0].cases.Count+'; secure codes/profile='+$taskProfiles[0].secured.Count)
} finally {Pop-Location}
