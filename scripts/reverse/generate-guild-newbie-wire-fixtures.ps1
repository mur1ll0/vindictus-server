$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
Push-Location $taskProject
try {
 [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
 New-Item -ItemType Directory -Force .cache/framework,.cache/verification|Out-Null
 $taskSources=@('scripts/reverse/framework/GuildNewbieWireOracle.cs')
 foreach($taskFile in 'Packet.cs','ClassInfo.cs','SerializeWriter.cs','SerializeWriterHelper.cs','SerializedSizeCalculator.cs','SerializedSizeCalculatorHelper.cs','SerializeReader.cs','SerializeReaderHelper.cs','MessageAnalyzer.cs','MEMORYSTATUSEX.cs'){$taskSources+='legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net.Message/'+$taskFile}
 $taskSources+=@('legacy/decompiled/Core-8942a0c26a9a/Devcat.Core/EventArgs.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacket.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacketAnalyzer.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/ICryptoTransform.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork.GuildService/InGameGuildInfo.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork.GuildService/DirectPurchaseGuildItemResultCode.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/IMessage.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.ItemServiceOperations/SlotInfo.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.ItemServiceOperations/ItemAttributeElement.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork.GuildService/GuildStorageBriefLogElement.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork.GuildService/GuildStorageItemLogElement.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork.GuildService/GuildStorageOperationType.cs')
 $taskOriginalHash='be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683'
 if((Get-FileHash '../ServidorVindictus/Server/zh-tw-x-gm/Bin/ServiceCore.dll').Hash.ToLowerInvariant()-cne$taskOriginalHash){throw 'Original assembly changed'}
 $taskCopies=@();$taskOriginals=@()
 foreach($taskType in @(@('FindNewbieRecommendGuildMessage',0,'1a16885d-f10c-3778-b694-bb3ec0448294'),@('FindNewbieRecommendGuildResultMessage',0,'99f557b5-d1f3-3c4e-b5a2-9063c3dc1a9b'),@('RequestNewbieRecommendGuildMessage',0,'d84b903e-f448-3d7e-87b1-1d506f1dedcc'),@('NoticeNewbieRecommendMessage',0,'35251c7c-a752-3f8f-b8d5-b8c875311683'),@('DirectPurchaseGuildItemMessage',0,'dbc653b2-4d96-3906-bfff-4849fa819fe1'),@('DirectPurchaseGuildItemResultMessage',0,'4e5db23c-be88-3d8d-b9c6-07d2bdc040f9'))){
  foreach($taskArch in 'x86','x64'){$taskTable=Get-Content ('docs/evidence/reverse/framework-types-'+$taskArch+'.json') -Raw|ConvertFrom-Json;$taskRow=@($taskTable.messages|Where-Object name -eq ('ServiceCore.EndPointNetwork.GuildService.'+$taskType[0]));if($taskTable.source_sha256-cne$taskOriginalHash-or$taskRow.Count-ne1-or$taskRow[0].category-ne$taskType[1]-or$taskRow[0].guid-cne$taskType[2]){throw 'Guild storage type identity changed'}}
  $taskSource='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork.GuildService/'+$taskType[0]+'.cs';$taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskSource));$taskCode=$taskCode.Replace('[Serializable]','[Serializable, System.Runtime.InteropServices.Guid("'+$taskType[2]+'")]');$taskCopy='.cache/framework/GuildNewbieWire-'+$taskType[0]+'.cs';[IO.File]::WriteAllText((Join-Path $PWD $taskCopy),$taskCode,[Text.UTF8Encoding]::new($false));$taskCopies+=$taskCopy;$taskOriginals+=$taskSource
 }
 $taskProfiles=@()
 foreach($taskArch in 'x86','x64'){
  $taskExe='.cache/framework/GuildNewbieWire-'+$taskArch+'.exe';$taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/unsafe+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
  foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+='/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLib+'.dll'}
  & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs @taskSources @taskCopies *> ('.cache/verification/guild-newbie-wire-build-'+$taskArch+'.log')
  if($LASTEXITCODE-ne0){throw 'Guild newbie oracle build failed; see local log'}
  if(Select-String ('.cache/verification/guild-newbie-wire-build-'+$taskArch+'.log') -Pattern 'warning [A-Z]+[0-9]+:'){throw 'Guild newbie oracle warning'}
  & (Join-Path $PWD $taskExe) *> ('.cache/verification/guild-newbie-wire-'+$taskArch+'.json')
  if($LASTEXITCODE-ne0){throw 'Guild newbie oracle execution failed; see local log'}
  $taskProfiles+=Get-Content ('.cache/verification/guild-newbie-wire-'+$taskArch+'.json') -Raw|ConvertFrom-Json
 }
 if(($taskProfiles[0].wire_cases|ConvertTo-Json -Depth 30 -Compress)-cne($taskProfiles[1].wire_cases|ConvertTo-Json -Depth 30 -Compress)){throw 'Guild newbie oracle x86/x64 disagree'}
 $taskAll=$taskSources+$taskOriginals+@('scripts/reverse/generate-guild-newbie-wire-fixtures.ps1')
 [ordered]@{profiles=$taskProfiles;source_sha256=$taskOriginalHash;sources=@($taskAll|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});original_services_executed=$false;method='Recovered Framework serializer and reader, guild storage message fields, nested slot/attribute/log types and nullability; synthetic values, no original services.'}|ConvertTo-Json -Depth 30 -Compress|Set-Content tests/fixtures/framework-guild-newbie-wire.json -Encoding utf8NoBOM
 Write-Output ('Guild newbie oracle: '+$taskProfiles[0].wire_cases.Count+' wire cases per architecture; x86=x64; zero warnings.')
}finally{Pop-Location}
