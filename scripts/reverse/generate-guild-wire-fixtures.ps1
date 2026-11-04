$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
Push-Location $taskProject
try {
 [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
 New-Item -ItemType Directory -Force .cache/framework,.cache/verification|Out-Null
 $taskSources=@('scripts/reverse/framework/GuildWireOracle.cs')
 foreach($taskFile in 'Packet.cs','ClassInfo.cs','SerializeWriter.cs','SerializeWriterHelper.cs','SerializedSizeCalculator.cs','SerializedSizeCalculatorHelper.cs','SerializeReader.cs','SerializeReaderHelper.cs','MessageAnalyzer.cs','MEMORYSTATUSEX.cs'){$taskSources+='legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net.Message/'+$taskFile}
 $taskSources+=@('legacy/decompiled/Core-8942a0c26a9a/Devcat.Core/EventArgs.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacket.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacketAnalyzer.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/ICryptoTransform.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/IMessage.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork.GuildService/InGameGuildInfo.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork.GuildService/GuildMemberInfo.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork.GuildService/GuildMemberRank.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork.GuildService/GuildOperationInfo.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork.GuildService/GuildResultEnum.cs')
 $taskOriginalHash='be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683'
 if((Get-FileHash '../ServidorVindictus/Server/zh-tw-x-gm/Bin/ServiceCore.dll').Hash.ToLowerInvariant()-cne$taskOriginalHash){throw 'Original assembly changed'}
 $taskCopies=@();$taskOriginals=@()
 foreach($taskType in @(@('OpenGuildMessage',338,'37c909f5-c8f6-33d2-8f2c-7dda5b1fe08d'),@('GuildResultMessage',281,'4d108cd7-8273-3e5b-afcc-2d077d73e7e0'),@('GuildInfoMessage',278,'647a5f98-b588-328f-b58d-ddc8164d3014'),@('GuildMemberListMessage',336,'e9a63382-19a4-39b9-915a-765a485a84b0'),@('QueryGuildListMessage',339,'34da3a6d-e9a2-3f2f-83a6-e73965a7b636'),@('GuildListMessage',279,'d74f17bb-616b-3a80-a17f-46217557ad03'),@('JoinGuildMessage',284,'7abea10d-5632-3388-bea1-c342d53465f8'),@('LeaveGuildMessage',285,'eb9a9076-bd3a-3358-87b9-14a00200e1f8'),@('CloseGuildMessage',277,'89f37dba-2fc1-3b7d-9512-465648c48278'),@('ReconnectGuildMessage',286,'918435ec-3213-3b76-b1b8-569b1f7657cb'),@('GuildOperationMessage',337,'0c77318b-cd6a-3bd6-8d6c-1e26f68f8717'),@('ChangeMasterMessage',84,'8f635448-72bd-3685-bcea-b6cd22d0db8d'),@('InviteGuildMessage',283,'51b65671-7ce1-3ca7-8572-1b02fb80076a'),@('GuildNoticeEditMessage',280,'3df39210-4621-3c4a-b85a-a1c864c4f86d'),@('GuildStoryRequestMessage',282,'efc67527-b0af-310f-a8b7-b39c1922e670'),@('GuildChatMemberInfoMessage',85,'95a85ad2-b272-3422-9b92-3a664dc0cba9'),@('GuildChatMessage',86,'76a1e908-29cf-3ecd-8275-9453e1534766'),@('JoinGuildChatRoomMessage',88,'6ea314db-4675-36a1-979a-d0942107da04'),@('GuildGainGPMessage',89,'d829f2cb-baba-3bbf-828b-cab4fa8aaae4'),@('GuildLevelUpMessage',90,'2d17a076-99cb-33c5-8eff-d05926933c7a'))){
  foreach($taskArch in 'x86','x64'){$taskTable=Get-Content ('docs/evidence/reverse/framework-types-'+$taskArch+'.json') -Raw|ConvertFrom-Json;$taskRow=@($taskTable.messages|Where-Object name -eq ('ServiceCore.EndPointNetwork.GuildService.'+$taskType[0]));if($taskTable.source_sha256-cne$taskOriginalHash-or$taskRow.Count-ne1-or$taskRow[0].category-ne$taskType[1]-or$taskRow[0].guid-cne$taskType[2]){throw 'Guild type identity changed'}}
  $taskSource='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork.GuildService/'+$taskType[0]+'.cs';$taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskSource));$taskCode=$taskCode.Replace('[Serializable]','[Serializable, System.Runtime.InteropServices.Guid("'+$taskType[2]+'")]');$taskCopy='.cache/framework/GuildWire-'+$taskType[0]+'.cs';[IO.File]::WriteAllText((Join-Path $PWD $taskCopy),$taskCode,[Text.UTF8Encoding]::new($false));$taskCopies+=$taskCopy;$taskOriginals+=$taskSource
 }
 $taskProfiles=@()
 foreach($taskArch in 'x86','x64'){
  $taskExe='.cache/framework/GuildWire-'+$taskArch+'.exe';$taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/unsafe+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
  foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+='/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLib+'.dll'}
  & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs @taskSources @taskCopies *> ('.cache/verification/guild-wire-build-'+$taskArch+'.log')
  if($LASTEXITCODE-ne0){throw 'Guild oracle build failed; see local log'}
  if(Select-String ('.cache/verification/guild-wire-build-'+$taskArch+'.log') -Pattern 'warning [A-Z]+[0-9]+:'){throw 'Guild oracle warning'}
  & (Join-Path $PWD $taskExe) *> ('.cache/verification/guild-wire-'+$taskArch+'.json')
  if($LASTEXITCODE-ne0){throw 'Guild oracle execution failed; see local log'}
  $taskProfiles+=Get-Content ('.cache/verification/guild-wire-'+$taskArch+'.json') -Raw|ConvertFrom-Json
 }
 if(($taskProfiles[0].wire_cases|ConvertTo-Json -Depth 30 -Compress)-cne($taskProfiles[1].wire_cases|ConvertTo-Json -Depth 30 -Compress)){throw 'Guild oracle x86/x64 disagree'}
 $taskAll=$taskSources+$taskOriginals+@('scripts/reverse/generate-guild-wire-fixtures.ps1')
 [ordered]@{profiles=$taskProfiles;source_sha256=$taskOriginalHash;sources=@($taskAll|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});original_services_executed=$false;method='Recovered Framework serializer and reader, guild message fields, nested guild info/member/operation types and nullability; synthetic values, no original services.'}|ConvertTo-Json -Depth 30 -Compress|Set-Content tests/fixtures/framework-guild-wire.json -Encoding utf8NoBOM
 Write-Output ('Guild oracle: '+$taskProfiles[0].wire_cases.Count+' wire cases per architecture; x86=x64; zero warnings.')
}finally{Pop-Location}
