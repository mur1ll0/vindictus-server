$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
Push-Location $taskProject
try {
 [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
 New-Item -ItemType Directory -Force .cache/framework,.cache/verification|Out-Null
 $taskSources=@('scripts/reverse/framework/GuildStorageWireOracle.cs')
 foreach($taskFile in 'Packet.cs','ClassInfo.cs','SerializeWriter.cs','SerializeWriterHelper.cs','SerializedSizeCalculator.cs','SerializedSizeCalculatorHelper.cs','SerializeReader.cs','SerializeReaderHelper.cs','MessageAnalyzer.cs','MEMORYSTATUSEX.cs'){$taskSources+='legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net.Message/'+$taskFile}
 $taskSources+=@('legacy/decompiled/Core-8942a0c26a9a/Devcat.Core/EventArgs.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacket.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacketAnalyzer.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/ICryptoTransform.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/IMessage.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.ItemServiceOperations/SlotInfo.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.ItemServiceOperations/ItemAttributeElement.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork.GuildService/GuildStorageBriefLogElement.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork.GuildService/GuildStorageItemLogElement.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork.GuildService/GuildStorageOperationType.cs')
 $taskOriginalHash='be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683'
 if((Get-FileHash '../ServidorVindictus/Server/zh-tw-x-gm/Bin/ServiceCore.dll').Hash.ToLowerInvariant()-cne$taskOriginalHash){throw 'Original assembly changed'}
 $taskCopies=@();$taskOriginals=@()
 foreach($taskType in @(@('GuildInventoryInfoMessage',87,'58f0a03b-7aae-3521-bb10-7c39acc518e0'),@('AddGuildStorageItemMessage',0,'3a237aa6-7470-3080-a641-274aac6d4d48'),@('PickGuildStorageItemMessage',0,'4a24b568-3e57-331b-894b-4f638dbf19b0'),@('PurchaseGuildStorageMessage',0,'c700797e-33e2-3192-bc91-c83c9131f339'),@('ArrangeGuildStorageItemMessage',0,'a895bf47-377e-3ad7-a87c-43fea7945f06'),@('HandleGuildStorageSessionMessage',0,'bef7785d-6e8a-3a95-a0d9-23cf767ee4f1'),@('UpdateGuildStorageSettingMessage',0,'dc4f3599-51af-344d-bca5-deea3f868484'),@('GuildStorageLogsMessage',0,'9b995088-968a-3864-8045-5ae4e19e569a'))){
  foreach($taskArch in 'x86','x64'){$taskTable=Get-Content ('docs/evidence/reverse/framework-types-'+$taskArch+'.json') -Raw|ConvertFrom-Json;$taskRow=@($taskTable.messages|Where-Object name -eq ('ServiceCore.EndPointNetwork.GuildService.'+$taskType[0]));if($taskTable.source_sha256-cne$taskOriginalHash-or$taskRow.Count-ne1-or$taskRow[0].category-ne$taskType[1]-or$taskRow[0].guid-cne$taskType[2]){throw 'Guild storage type identity changed'}}
  $taskSource='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork.GuildService/'+$taskType[0]+'.cs';$taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskSource));$taskCode=$taskCode.Replace('[Serializable]','[Serializable, System.Runtime.InteropServices.Guid("'+$taskType[2]+'")]');$taskCopy='.cache/framework/GuildStorageWire-'+$taskType[0]+'.cs';[IO.File]::WriteAllText((Join-Path $PWD $taskCopy),$taskCode,[Text.UTF8Encoding]::new($false));$taskCopies+=$taskCopy;$taskOriginals+=$taskSource
 }
 $taskProfiles=@()
 foreach($taskArch in 'x86','x64'){
  $taskExe='.cache/framework/GuildStorageWire-'+$taskArch+'.exe';$taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/unsafe+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
  foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+='/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLib+'.dll'}
  & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs @taskSources @taskCopies *> ('.cache/verification/guild-storage-wire-build-'+$taskArch+'.log')
  if($LASTEXITCODE-ne0){throw 'Guild storage oracle build failed; see local log'}
  if(Select-String ('.cache/verification/guild-storage-wire-build-'+$taskArch+'.log') -Pattern 'warning [A-Z]+[0-9]+:'){throw 'Guild storage oracle warning'}
  & (Join-Path $PWD $taskExe) *> ('.cache/verification/guild-storage-wire-'+$taskArch+'.json')
  if($LASTEXITCODE-ne0){throw 'Guild storage oracle execution failed; see local log'}
  $taskProfiles+=Get-Content ('.cache/verification/guild-storage-wire-'+$taskArch+'.json') -Raw|ConvertFrom-Json
 }
 if(($taskProfiles[0].wire_cases|ConvertTo-Json -Depth 30 -Compress)-cne($taskProfiles[1].wire_cases|ConvertTo-Json -Depth 30 -Compress)){throw 'Guild storage oracle x86/x64 disagree'}
 $taskAll=$taskSources+$taskOriginals+@('scripts/reverse/generate-guild-storage-wire-fixtures.ps1')
 [ordered]@{profiles=$taskProfiles;source_sha256=$taskOriginalHash;sources=@($taskAll|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});original_services_executed=$false;method='Recovered Framework serializer and reader, guild storage message fields, nested slot/attribute/log types and nullability; synthetic values, no original services.'}|ConvertTo-Json -Depth 30 -Compress|Set-Content tests/fixtures/framework-guild-storage-wire.json -Encoding utf8NoBOM
 Write-Output ('Guild storage oracle: '+$taskProfiles[0].wire_cases.Count+' wire cases per architecture; x86=x64; zero warnings.')
}finally{Pop-Location}
