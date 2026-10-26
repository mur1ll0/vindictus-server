$ErrorActionPreference='Stop'
# The oracle prints UTF-8 cases (HeroesString samples); keep them intact when redirected.
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$taskSources=@('scripts/reverse/framework/LobbyControlWireOracle.cs')
foreach($taskFile in 'Packet.cs','ClassInfo.cs','SerializeWriter.cs','SerializeWriterHelper.cs','SerializedSizeCalculator.cs','SerializedSizeCalculatorHelper.cs','SerializeReader.cs','SerializeReaderHelper.cs'){$taskSources+=('legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net.Message/'+$taskFile)}
$taskSources+=@('legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacket.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/ICryptoTransform.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/IMessage.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/HeroesString.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/SystemMessageCategory.cs')
$taskX86=Get-Content docs/evidence/reverse/framework-types-x86.json -Raw|ConvertFrom-Json
$taskX64=Get-Content docs/evidence/reverse/framework-types-x64.json -Raw|ConvertFrom-Json
$taskOriginalHash='be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683'
if((Get-FileHash ../ServidorVindictus/Server/zh-tw-x-gm/Bin/ServiceCore.dll).Hash.ToLowerInvariant()-ne$taskOriginalHash){throw 'Original assembly hash changed'}
$taskCopies=@()
foreach($taskName in 'PingMessage','PongMessage','SystemMessage','NGSecurityMessage','CafeLoginMessage','NoticeShutDownMessage'){
 $taskRow=@($taskX86.messages|Where-Object name -eq ('ServiceCore.EndPointNetwork.'+$taskName));$taskOther=@($taskX64.messages|Where-Object name -eq ('ServiceCore.EndPointNetwork.'+$taskName))
 if($taskRow.Count-ne1-or$taskOther.Count-ne1-or$taskRow[0].category-ne$taskOther[0].category-or$taskRow[0].guid-ne$taskOther[0].guid){throw 'Original architecture metadata disagreement'}
 $taskPath='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/'+$taskName+'.cs';$taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskPath));$taskCode=$taskCode.Replace('[Serializable]','[Serializable, System.Runtime.InteropServices.Guid("'+$taskRow[0].guid+'")]');$taskCopy='.cache/framework/LobbyControl-'+$taskName+'.cs';[IO.File]::WriteAllText((Join-Path $PWD $taskCopy),$taskCode,[Text.UTF8Encoding]::new($false));$taskSources+=$taskPath;$taskCopies+=$taskCopy
}
$taskProfiles=@()
foreach($taskArch in 'x86','x64'){
 $taskExe='.cache/framework/LobbyControlWire-'+$taskArch+'.exe';$taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/unsafe+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
 foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLib+'.dll')}
 $taskArgs+=@($taskSources|Where-Object{$_-notlike '*ServiceCore.EndPointNetwork/*Message.cs'-or$_-like '*IMessage.cs'});$taskArgs+=$taskCopies
 & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs *> ('.cache/verification/lobby-control-wire-build-'+$taskArch+'.log');if($LASTEXITCODE-ne0){throw 'Control wire compilation failed; local log'};if(Select-String ('.cache/verification/lobby-control-wire-build-'+$taskArch+'.log') -Pattern 'warning [A-Z]+[0-9]+:'){throw 'Control wire compiler warnings'}
 & (Join-Path $PWD $taskExe) *> ('.cache/verification/lobby-control-wire-'+$taskArch+'.json');if($LASTEXITCODE-ne0){throw 'Control wire execution failed; local log'}
 $taskProfile=Get-Content ('.cache/verification/lobby-control-wire-'+$taskArch+'.json') -Raw|ConvertFrom-Json;if($taskProfile.cases.Count-ne459){throw 'Control wire case count mismatch'};$taskProfiles+=$taskProfile
}
if(($taskProfiles[0].cases|ConvertTo-Json -Depth 8 -Compress)-cne($taskProfiles[1].cases|ConvertTo-Json -Depth 8 -Compress)){throw 'Control wire architecture disagreement'}
[ordered]@{profiles=$taskProfiles;source_sha256=$taskOriginalHash;sources=@($taskSources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});original_services_executed=$false}|ConvertTo-Json -Depth 12 -Compress|Set-Content tests/fixtures/framework-lobby-control-wire.json -Encoding utf8NoBOM
Write-Host 'Control wire original DTO/serializer oracle:459synthetic cases per architecture,0differences; public synthetic fixture generated.'
