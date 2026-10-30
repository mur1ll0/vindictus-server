$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
Push-Location $taskProject
try {
 New-Item -ItemType Directory -Force .cache/framework,.cache/verification|Out-Null
 $taskSources=@('scripts/reverse/framework/PetWireOracle.cs')
 foreach($taskFile in 'Packet.cs','ClassInfo.cs','SerializeWriter.cs','SerializeWriterHelper.cs','SerializedSizeCalculator.cs','SerializedSizeCalculatorHelper.cs','SerializeReader.cs','SerializeReaderHelper.cs','MessageAnalyzer.cs','MEMORYSTATUSEX.cs'){$taskSources+='legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net.Message/'+$taskFile}
 $taskSources+=@('legacy/decompiled/Core-8942a0c26a9a/Devcat.Core/EventArgs.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacket.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacketAnalyzer.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/ICryptoTransform.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/IMessage.cs')
 foreach($taskFile in 'PetStatusInfo.cs','PetStatElement.cs','PetSkillElement.cs','PetAccessoryElement.cs','PetFeedElement.cs','PetOperationType.cs'){$taskSources+='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/'+$taskFile}
 $taskOriginalHash='be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683'
 if((Get-FileHash '../ServidorVindictus/Server/zh-tw-x-gm/Bin/ServiceCore.dll').Hash.ToLowerInvariant()-cne$taskOriginalHash){throw 'Original assembly changed'}
 $taskCopies=@();$taskOriginals=@()
 foreach($taskType in @(@('MovePetSlotMessage',313,'86698206-912c-36be-9cc1-257a396935fb'),@('RemovePetMessage',314,'9f8b0d6d-ae8e-388f-8841-11aa2aa37a0d'),@('SelectPetMessage',315,'5736fde9-bc21-3b82-a84e-72bb0a517a14'),@('QueryPetListMessage',316,'e94af65d-e66f-31fd-b8d0-53a042370b5c'),@('PetRevivedMessage',317,'e145a882-d89c-3494-97bf-607b1aeaaab5'),@('PetKilledMessage',318,'3079bc3a-9b67-3de0-a504-87a66664647f'),@('RemovePetSkillMessage',319,'8aaf9011-eb3a-3fc1-a40b-25602676547b'),@('PetCreateNameCheckMessage',320,'efd95e57-04fa-3e93-9271-9bcc29422fb0'),@('PetCreateNameCheckResultMessage',321,'133b280b-47e4-34cc-b16a-a8a03eb4dc97'),@('PetCreateMessage',322,'942945c3-e95f-3bb4-9be2-a7f984434f41'),@('UpdateCurrentPetMessage',323,'9fddbc1b-3c9e-3c79-8b50-457c8379c699'),@('PetListMessage',324,'62804a1d-30eb-310c-8792-0b6aa839ffd3'),@('PetFeedListMessage',325,'2fcaaf5c-a077-375d-933c-92d298581487'),@('RequestPetFoodShareMessage',326,'0d488183-a8d0-3730-99ab-1088836ab2d4'),@('RequestPetFoodUnshareMessage',327,'5922ce58-625f-3c91-8b08-7331f63b5a27'),@('PetChangeNameCheckMessage',328,'ad77125f-4472-336f-a50e-45d79ec35a0e'),@('PetChangeNameCheckResultMessage',329,'8b057824-a031-3410-a24b-f52be6c03cf3'),@('PetChangeNameMessage',330,'041d653f-01e7-371a-b86a-1346b110d36c'),@('PetChangeNameResultMessage',331,'a9c12ba2-27cd-3864-9786-42d4f295dff5'),@('PetOperationMessage',332,'bf092479-3775-3d89-8f77-53bcc8c681f4'))){
  foreach($taskArch in 'x86','x64'){$taskTable=Get-Content ('docs/evidence/reverse/framework-types-'+$taskArch+'.json') -Raw|ConvertFrom-Json;$taskRow=@($taskTable.messages|Where-Object name -eq ('ServiceCore.EndPointNetwork.'+$taskType[0]));if($taskTable.source_sha256-cne$taskOriginalHash-or$taskRow.Count-ne1-or$taskRow[0].category-ne$taskType[1]-or$taskRow[0].guid-cne$taskType[2]){throw 'Pet type identity changed'}}
  $taskSource='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/'+$taskType[0]+'.cs';$taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskSource));$taskCode=$taskCode.Replace('[Serializable]','[Serializable, System.Runtime.InteropServices.Guid("'+$taskType[2]+'")]');$taskCopy='.cache/framework/PetWire-'+$taskType[0]+'.cs';[IO.File]::WriteAllText((Join-Path $PWD $taskCopy),$taskCode,[Text.UTF8Encoding]::new($false));$taskCopies+=$taskCopy;$taskOriginals+=$taskSource
 }
 $taskProfiles=@()
 foreach($taskArch in 'x86','x64'){
  $taskExe='.cache/framework/PetWire-'+$taskArch+'.exe';$taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/unsafe+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
  foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+='/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLib+'.dll'}
  & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs @taskSources @taskCopies *> ('.cache/verification/pet-wire-build-'+$taskArch+'.log')
  if($LASTEXITCODE-ne0){throw 'Pet oracle build failed; see local log'}
  if(Select-String ('.cache/verification/pet-wire-build-'+$taskArch+'.log') -Pattern 'warning [A-Z]+[0-9]+:'){throw 'Pet oracle warning'}
  & (Join-Path $PWD $taskExe) *> ('.cache/verification/pet-wire-'+$taskArch+'.json')
  if($LASTEXITCODE-ne0){throw 'Pet oracle execution failed; see local log'}
  $taskProfiles+=Get-Content ('.cache/verification/pet-wire-'+$taskArch+'.json') -Raw|ConvertFrom-Json
 }
 if(($taskProfiles[0].wire_cases|ConvertTo-Json -Depth 30 -Compress)-cne($taskProfiles[1].wire_cases|ConvertTo-Json -Depth 30 -Compress)){throw 'Pet oracle x86/x64 disagree'}
 $taskAll=$taskSources+$taskOriginals+@('scripts/reverse/generate-pet-wire-fixtures.ps1')
 [ordered]@{profiles=$taskProfiles;source_sha256=$taskOriginalHash;sources=@($taskAll|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});original_services_executed=$false;method='Recovered Framework serializer and reader, pet message fields and nullability; synthetic values, no original services.'}|ConvertTo-Json -Depth 30 -Compress|Set-Content tests/fixtures/framework-pet-wire.json -Encoding utf8NoBOM
 Write-Output ('Pet oracle: '+$taskProfiles[0].wire_cases.Count+' wire cases per architecture; x86=x64; zero warnings.')
}finally{Pop-Location}
