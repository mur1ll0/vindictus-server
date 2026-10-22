$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
Push-Location $taskProject
try {
 New-Item -ItemType Directory -Force .cache/framework,.cache/verification|Out-Null
 $taskSources=@('scripts/reverse/framework/MissionWireOracle.cs')
 foreach($taskFile in 'Packet.cs','ClassInfo.cs','SerializeWriter.cs','SerializeWriterHelper.cs','SerializedSizeCalculator.cs','SerializedSizeCalculatorHelper.cs','SerializeReader.cs','SerializeReaderHelper.cs','MessageAnalyzer.cs','MEMORYSTATUSEX.cs'){$taskSources+='legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net.Message/'+$taskFile}
 $taskSources+=@('legacy/decompiled/Core-8942a0c26a9a/Devcat.Core/EventArgs.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacket.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacketAnalyzer.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/ICryptoTransform.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/IMessage.cs')
 foreach($taskFile in 'CostumeInfo.cs','ShipOptionInfo.cs','PartyMemberInfo.cs','PartyInfoState.cs','ReadyState.cs','ShipInfo.cs','GameJoinMemberInfo.cs','PetStatusInfo.cs','PetStatElement.cs','PetSkillElement.cs','PetAccessoryElement.cs','StatusEffectElement.cs','GameInfo.cs','QuestSectorInfos.cs','QuestSectorInfo.cs','QuestSectorConnectionInfo.cs','TrialFactorInfo.cs','TreasureBoxInfo.cs'){$taskSources+='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/'+$taskFile}
 foreach($taskFile in 'BattleInventory.cs','ConsumablesInfo.cs'){$taskSources+='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.MicroPlayServiceOperations/'+$taskFile}
 foreach($taskFile in 'BriefSkillEnhance.cs','BaseCharacter.cs'){$taskSources+='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.CharacterServiceOperations/'+$taskFile}
 $taskOriginalHash='be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683'
 if((Get-FileHash '../ServidorVindictus/Server/zh-tw-x-gm/Bin/ServiceCore.dll').Hash.ToLowerInvariant()-cne$taskOriginalHash){throw 'Original assembly changed'}
 $taskCopies=@();$taskOriginals=@()
 foreach($taskType in @(@('AcceptQuestMessage',224,'aecbcd0a-82b3-31ee-b771-fe9c53dabdd8'),@('PartyInfoMessage',511,'78d2aa77-13f9-302f-b9fe-67ca096b92f3'),@('UpdateShipMessage',618,'c87e7f92-c8c2-36b1-83e8-bc9fbd04e5d4'),@('ConnectionRequestMessage',425,'31fc12c3-d398-355f-a15c-786e69ccc95f'),@('LaunchShipGrantedMessage',473,'78df701a-83a3-33f5-be94-8d27ecb6e23b'),@('RegisterServerMessage',554,'b8a9e8c0-377b-35ad-b3be-bb911152a565'),@('StartGameMessage',591,'7c04c215-b83c-3053-b80c-966064258660'),@('GameStartGrantedMessage',462,'dc3acd0b-6946-3b6c-bcaf-29057e0dfc59'),@('MemberInfoMessage',498,'b89c49cb-0aa1-308d-a0c5-9fe9bb1f58ed'))){
  foreach($taskArch in 'x86','x64'){$taskTable=Get-Content ('docs/evidence/reverse/framework-types-'+$taskArch+'.json') -Raw|ConvertFrom-Json;$taskRow=@($taskTable.messages|Where-Object name -eq ('ServiceCore.EndPointNetwork.'+$taskType[0]));if($taskTable.source_sha256-cne$taskOriginalHash-or$taskRow.Count-ne1-or$taskRow[0].category-ne$taskType[1]-or$taskRow[0].guid-cne$taskType[2]){throw 'Mission type identity changed'}}
  $taskSource='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/'+$taskType[0]+'.cs';$taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskSource));$taskCode=$taskCode.Replace('[Serializable]','[Serializable, System.Runtime.InteropServices.Guid("'+$taskType[2]+'")]');$taskCopy='.cache/framework/MissionWire-'+$taskType[0]+'.cs';[IO.File]::WriteAllText((Join-Path $PWD $taskCopy),$taskCode,[Text.UTF8Encoding]::new($false));$taskCopies+=$taskCopy;$taskOriginals+=$taskSource
 }
 $taskProfiles=@()
 foreach($taskArch in 'x86','x64'){
  $taskExe='.cache/framework/MissionWire-'+$taskArch+'.exe';$taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/unsafe+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
  foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+='/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLib+'.dll'}
  & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs @taskSources @taskCopies *> ('.cache/verification/mission-wire-build-'+$taskArch+'.log')
  if($LASTEXITCODE-ne0){throw 'Mission oracle build failed; see local log'}
  if(Select-String ('.cache/verification/mission-wire-build-'+$taskArch+'.log') -Pattern 'warning [A-Z]+[0-9]+:'){throw 'Mission oracle warning'}
  & (Join-Path $PWD $taskExe) *> ('.cache/verification/mission-wire-'+$taskArch+'.json')
  if($LASTEXITCODE-ne0){throw 'Mission oracle execution failed; see local log'}
  $taskProfiles+=Get-Content ('.cache/verification/mission-wire-'+$taskArch+'.json') -Raw|ConvertFrom-Json
 }
 if(($taskProfiles[0].wire_cases|ConvertTo-Json -Depth 30 -Compress)-cne($taskProfiles[1].wire_cases|ConvertTo-Json -Depth 30 -Compress)){throw 'Mission oracle x86/x64 disagree'}
 $taskAll=$taskSources+$taskOriginals+@('scripts/reverse/generate-mission-wire-fixtures.ps1')
 [ordered]@{profiles=$taskProfiles;source_sha256=$taskOriginalHash;sources=@($taskAll|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});original_services_executed=$false;method='Recovered Framework serializer and reader, message fields and nullability; synthetic values, no original services.'}|ConvertTo-Json -Depth 30 -Compress|Set-Content tests/fixtures/framework-mission-wire.json -Encoding utf8NoBOM
 Write-Output ('Mission oracle: '+$taskProfiles[0].wire_cases.Count+' wire cases per architecture; x86=x64; zero warnings.')
}finally{Pop-Location}
