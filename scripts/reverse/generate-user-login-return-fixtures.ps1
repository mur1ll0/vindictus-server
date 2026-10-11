param()
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try{
    New-Item -ItemType Directory -Force .cache/framework,.cache/verification | Out-Null
    $taskSource='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.LoginServiceOperations/UserLogin.cs'
    $taskHarness='scripts/reverse/framework/UserLoginReturnOracle.cs'
    $taskEnum='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/LoginFailMessage.cs'
    $taskProfiles=@()
    foreach($taskArch in 'x86','x64'){
        $taskExe='.cache/framework/UserLoginReturn-'+$taskArch+'.exe'
        $taskArgs=@('C:\Program Files\dotnet\sdk\9.0.301\Roslyn\bincore\csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
        foreach($taskLibrary in 'mscorlib','System','System.Core','System.Web.Extensions'){
            $taskArgs+=('/r:C:\Program Files (x86)\Reference Assemblies\Microsoft\Framework\.NETFramework\v4.8\'+$taskLibrary+'.dll')
        }
        $taskArgs+=@($taskSource,$taskEnum,$taskHarness)
        & 'C:\Program Files\dotnet\dotnet.exe' @taskArgs *> .cache/verification/user-login-return-oracle-build.log
        if($LASTEXITCODE-ne0){Get-Content .cache/verification/user-login-return-oracle-build.log -Tail 10;throw 'UserLogin oracle compilation failed'}
        $taskRaw=& (Join-Path $PWD $taskExe)
        if($LASTEXITCODE-ne0){throw 'UserLogin oracle execution failed'}
        $taskData=$taskRaw|ConvertFrom-Json
        if($taskData.architecture-ne$taskArch){throw 'Architecture mismatch'}
        $taskProfiles+=$taskData
    }
    $taskSources=@($taskSource,$taskEnum,$taskHarness,'legacy/decompiled/LoginServiceCore-1600fb4b593d/LoginServiceCore/LimitedAccount.cs','legacy/decompiled/LoginServiceCore-1600fb4b593d/LoginServiceCore/LoginService.cs')
    $taskHashes=@($taskSources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
    [ordered]@{profiles=$taskProfiles;sources=$taskHashes;original_services_executed=$false;method='Unchanged recovered UserLogin class and FailReason enum with cooperation runtime stubs. SingleOrDefault BCL over selected rows; SQL equality measured separately.'}|ConvertTo-Json -Depth 12|Set-Content tests/fixtures/framework-user-login-return.json -Encoding utf8NoBOM
    Write-Host ('UserLogin oracle: profiles=2; cases/profile='+$taskProfiles[0].cases.Count+'; SingleOrDefault cases/profile='+$taskProfiles[0].single.Count)
}finally{Pop-Location}
