param()
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try {
    New-Item -ItemType Directory -Force .cache/framework,.cache/verification | Out-Null
    $taskProcessor='legacy/decompiled/LoginServiceCore-1600fb4b593d/LoginServiceCore.Processor/UserLoginProcessor.cs'
    $taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskProcessor))
    $taskStart=$taskCode.IndexOf('if (base.Connection.RemoteCategory')
    $taskEnd=$taskCode.IndexOf('int nexonSN =', $taskStart)
    if($taskStart-lt0 -or $taskEnd-lt0){throw 'Recovered duplicate block not found'}
    $taskDuplicate=$taskCode.Substring($taskStart,$taskEnd-$taskStart)
    $taskService='legacy/decompiled/LoginServiceCore-1600fb4b593d/LoginServiceCore/LoginService.cs'
    $taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskService))
    $taskStart=$taskCode.IndexOf('protected override IEntity MakeEntity(')
    $taskEnd=$taskCode.IndexOf('internal long CreateLogJournalID()', $taskStart)
    if($taskStart-lt0 -or $taskEnd-lt0){throw 'Recovered entity lifecycle not found'}
    # Interface -> synthetic class and protected -> public only expose the method to the harness.
    $taskEntity=$taskCode.Substring($taskStart,$taskEnd-$taskStart).Replace('IEntity','Entity').Replace('protected override','public override')
    $taskHarnessPath='scripts/reverse/framework/SessionLifecycleOracle.cs'
    $taskHarness=[IO.File]::ReadAllText((Join-Path $PWD $taskHarnessPath)).Replace('/* RECOVERED_DUPLICATE */',$taskDuplicate).Replace('/* RECOVERED_ENTITY */',$taskEntity)
    [IO.File]::WriteAllText((Join-Path $PWD '.cache/framework/SessionLifecycle.cs'),$taskHarness,[Text.UTF8Encoding]::new($false))
    $taskProfiles=@()
    foreach($taskArch in 'x86','x64'){
        $taskExe='.cache/framework/SessionLifecycle-'+$taskArch+'.exe'
        $taskArgs=@('C:\Program Files\dotnet\sdk\9.0.301\Roslyn\bincore\csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
        foreach($taskLibrary in 'mscorlib','System','System.Core','System.Web.Extensions'){
            $taskArgs+=('/r:C:\Program Files (x86)\Reference Assemblies\Microsoft\Framework\.NETFramework\v4.8\'+$taskLibrary+'.dll')
        }
        $taskArgs+=@('.cache/framework/SessionLifecycle.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/LoginFailMessage.cs')
        & 'C:\Program Files\dotnet\dotnet.exe' @taskArgs *> .cache/verification/session-lifecycle-oracle-build.log
        if($LASTEXITCODE-ne0){Get-Content .cache/verification/session-lifecycle-oracle-build.log -Tail 10;throw 'Lifecycle oracle compilation failed'}
        $taskRaw=& (Join-Path $PWD $taskExe)
        if($LASTEXITCODE-ne0){throw 'Lifecycle oracle execution failed'}
        $taskData=$taskRaw | ConvertFrom-Json
        if($taskData.architecture-ne$taskArch){throw 'Architecture mismatch'}
        $taskProfiles+=$taskData
    }
    $taskPaths=@($taskProcessor,$taskService,$taskHarnessPath,'legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/LoginFailMessage.cs')
    $taskHashes=@($taskPaths | ForEach-Object { [ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()} })
    [ordered]@{profiles=$taskProfiles;sources=$taskHashes;original_services_executed=$false;method='Recovered duplicate block and entity lifecycle; interface/accessibility adapted for synthetic event/scheduler/DB stubs. No AcquireService SQL emulation or distributed ownership proof'}|ConvertTo-Json -Depth 8|Set-Content tests/fixtures/framework-session-lifecycle.json -Encoding utf8NoBOM
    Write-Host ('Lifecycle oracle: profiles=2; duplicate cases/profile='+$taskProfiles[0].duplicates.Count+'; lifecycle cases/profile='+$taskProfiles[0].lifecycle.Count)
} finally {Pop-Location}
