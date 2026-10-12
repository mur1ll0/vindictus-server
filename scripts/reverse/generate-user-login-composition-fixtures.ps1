param()
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try {
    New-Item -ItemType Directory -Force .cache/framework,.cache/verification | Out-Null
    $taskSource='legacy/decompiled/LoginServiceCore-1600fb4b593d/LoginServiceCore.Processor/UserLoginProcessor.cs'
    $taskEnum='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/LoginFailMessage.cs'
    $taskHarness='scripts/reverse/framework/UserLoginCompositionOracle.cs'
    $taskText=[IO.File]::ReadAllText((Join-Path $PWD $taskSource))
    function Get-Fragment([string]$Begin,[string]$End) {
        $taskStart=$taskText.IndexOf($Begin,[StringComparison]::Ordinal)
        $taskStop=$taskText.IndexOf($End,$taskStart,[StringComparison]::Ordinal)
        if($taskStart-lt0 -or $taskStop-lt$taskStart){throw 'Recovered composition fragment missing'}
        return $taskText.Substring($taskStart,$taskStop-$taskStart)
    }
    $taskMember=Get-Fragment 'if (!banResult.HasValue)' 'KeyValuePair<string, int> lockResult'
    $taskIP=Get-Fragment 'if (!(lockResult.Key == ""))' "`t`t`ttry"
    # Source may use CRLF; select the seven yields through the last semicolon instead.
    $taskStart=$taskText.IndexOf('yield return 1;',[StringComparison]::Ordinal)
    $taskEnd=$taskText.IndexOf('yield return isReturn;',$taskStart,[StringComparison]::Ordinal)+'yield return isReturn;'.Length
    $taskYields=$taskText.Substring($taskStart,$taskEnd-$taskStart)
    $taskCombined=[IO.File]::ReadAllText((Join-Path $PWD $taskHarness)).Replace('/*MEMBER*/',$taskMember).Replace('/*IP*/',$taskIP).Replace('/*YIELDS*/',$taskYields)
    [IO.File]::WriteAllText((Join-Path $PWD '.cache/framework/UserLoginComposition.cs'),$taskCombined,[Text.UTF8Encoding]::new($false))
    $taskProfiles=@()
    foreach($taskArch in 'x86','x64') {
        $taskExe='.cache/framework/UserLoginComposition-'+$taskArch+'.exe'
        $taskArgs=@('C:\Program Files\dotnet\sdk\9.0.301\Roslyn\bincore\csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
        foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions') {
            $taskArgs+=('/r:C:\Program Files (x86)\Reference Assemblies\Microsoft\Framework\.NETFramework\v4.8\'+$taskLib+'.dll')
        }
        $taskArgs+=@('.cache/framework/UserLoginComposition.cs',$taskEnum)
        & 'C:\Program Files\dotnet\dotnet.exe' @taskArgs *> ('.cache/verification/user-login-composition-'+$taskArch+'-build.log')
        if($LASTEXITCODE-ne0){throw 'Composition oracle build failed; see local log'}
        & (Join-Path $PWD $taskExe) *> ('.cache/verification/user-login-composition-'+$taskArch+'.json')
        if($LASTEXITCODE-ne0){throw 'Composition oracle failed'}
        $taskData=Get-Content ('.cache/verification/user-login-composition-'+$taskArch+'.json') -Raw|ConvertFrom-Json -DateKind String
        if($taskData.architecture-ne$taskArch -or $taskData.cases.Count-ne480 -or $taskData.decoding.Count-ne256){throw 'Composition oracle counts differ'}
        $taskProfiles+=$taskData
    }
    $taskHashes=@(@($taskSource,$taskEnum,$taskHarness)|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}})
    [ordered]@{profiles=$taskProfiles;sources=$taskHashes;original_services_executed=$false;method='Unmodified extracted member failure, IP projection and seven final yields; independent stubs for logging/journal. Synthetic inputs, Framework CP1252; effects/provisioning verified separately.'}|ConvertTo-Json -Depth 12 -Compress|Set-Content tests/fixtures/framework-user-login-composition.json -Encoding utf8NoBOM
    Write-Host 'Composition oracle: 480 cases and 256 byte decodings per architecture; x86/x64=2'
} finally {Pop-Location}
