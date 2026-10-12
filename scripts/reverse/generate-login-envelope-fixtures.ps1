param()
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
Push-Location $taskProject
try {
    New-Item -ItemType Directory -Force .cache/framework,.cache/verification | Out-Null
    $taskHash='be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683'
    $taskOriginal=(Resolve-Path -LiteralPath '../ServidorVindictus/Server/zh-tw-x-gm/Bin/ServiceCore.dll').Path
    if((Get-FileHash -LiteralPath $taskOriginal).Hash.ToLowerInvariant()-ne$taskHash){throw 'Original ServiceCore hash mismatch'}
    $taskProfiles=@()
    $taskSource='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/UserLoginMessage.cs'
    $taskMetadata=@()
    foreach($taskArch in 'x86','x64') {
        $taskTable=Get-Content ('docs/evidence/reverse/framework-types-'+$taskArch+'.json') -Raw | ConvertFrom-Json
        $taskRow=@($taskTable.messages | Where-Object name -eq 'ServiceCore.EndPointNetwork.UserLoginMessage')
        if($taskTable.source_sha256-ne$taskHash -or $taskRow.Count-ne1 -or $taskRow[0].category-ne478 -or $taskRow[0].guid-ne'47f6f00c-0bd3-3059-afad-84a872ed7399'){throw 'Original metadata mismatch'}
        $taskMetadata+=('docs/evidence/reverse/framework-types-'+$taskArch+'.json')
    }
    # Copy only to the ignored oracle build directory. Pin original GUID because assembly identity differs.
    $taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskSource))
    if(([regex]::Matches($taskCode,'\[Serializable\]')).Count-ne1){throw 'Unexpected message attribute shape'}
    $taskCode=$taskCode.Replace('[Serializable]','[Serializable, System.Runtime.InteropServices.Guid("47f6f00c-0bd3-3059-afad-84a872ed7399")]')
    [IO.File]::WriteAllText((Join-Path $PWD '.cache/framework/EnvelopeUserLogin.cs'),$taskCode,[Text.UTF8Encoding]::new($false))
    $taskSources=@('scripts/reverse/framework/LoginEnvelopeOracle.cs',$taskSource)
    foreach($taskFile in 'Packet.cs','ClassInfo.cs','SerializeWriter.cs','SerializeWriterHelper.cs','SerializedSizeCalculator.cs','SerializedSizeCalculatorHelper.cs','SerializeReader.cs','SerializeReaderHelper.cs','MessageHandlerFactory.cs') {
        $taskSources+=('legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net.Message/'+$taskFile)
    }
    $taskSources+=@('legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacket.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/ICryptoTransform.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/IMessage.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore/Isaac.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore/CryptoTransformHeroes.cs')
    foreach($taskArch in 'x86','x64') {
        $taskExe='.cache/framework/LoginEnvelope-'+$taskArch+'.exe'
        $taskArgs=@('C:\Program Files\dotnet\sdk\9.0.301\Roslyn\bincore\csc.dll','/nologo','/noconfig','/nostdlib+','/unsafe+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
        foreach($taskLibrary in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+=('/r:C:\Program Files (x86)\Reference Assemblies\Microsoft\Framework\.NETFramework\v4.8\'+$taskLibrary+'.dll')}
        $taskArgs+=@($taskSources | Where-Object {$_-ne$taskSource -and $_-notlike '*MessageHandlerFactory.cs'})
        $taskArgs+='.cache/framework/EnvelopeUserLogin.cs'
        $taskLog='.cache/verification/login-envelope-build-'+$taskArch+'.log'
        & 'C:\Program Files\dotnet\dotnet.exe' @taskArgs *> $taskLog
        if($LASTEXITCODE-ne0){Get-Content $taskLog -Tail 8;throw 'Envelope oracle compilation failed'}
        if((Get-Content $taskLog -Raw)-match '(?i)warning'){throw 'Oracle compilation warning'}
        & (Join-Path $PWD $taskExe) *> ('.cache/verification/login-envelope-'+$taskArch+'.json')
        if($LASTEXITCODE-ne0){throw 'Envelope oracle execution failed; see private log'}
        $taskProfile=Get-Content ('.cache/verification/login-envelope-'+$taskArch+'.json') -Raw | ConvertFrom-Json
        if($taskProfile.architecture-ne$taskArch){throw 'Architecture mismatch'}
        $taskProfiles+=$taskProfile
    }
    if(($taskProfiles[0].cases | ConvertTo-Json -Depth 6 -Compress)-cne($taskProfiles[1].cases | ConvertTo-Json -Depth 6 -Compress)){throw 'Wire architecture disagreement'}
    $taskHashes=@(($taskSources+$taskMetadata+'scripts/reverse/generate-login-envelope-fixtures.ps1') | ForEach-Object { [ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()} })
    $taskResult=[ordered]@{profiles=$taskProfiles;sources=$taskHashes;source_sha256=$taskHash;original_services_executed=$false;method='Recovered Framework writer/reader/cipher unchanged; UserLogin source copied with original metadata GUID attribute only; synthetic fields and allocator shim. Both generic fallback and object overload compared to typed roundtrip.'}
    $taskResult | ConvertTo-Json -Depth 9 -Compress | Set-Content tests/fixtures/framework-login-envelope.json -Encoding utf8NoBOM
    [ordered]@{verified_at_utc=[DateTimeOffset]::UtcNow.ToString('o');architectures=2;cases_per_architecture=$taskProfiles[0].cases.Count;source_sha256=$taskHash;sources=$taskHashes;method=$taskResult.method;original_services_executed=$false;client_executed=$false} | ConvertTo-Json -Depth 6 | Set-Content docs/evidence/auth/login-envelope.json -Encoding utf8NoBOM
    Write-Host ('Login envelope oracle: architectures=2; cases/profile='+$taskProfiles[0].cases.Count+'; wire disagreements=0; recovered reader roundtrips passed.')
} finally {Pop-Location}
