$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
Push-Location $taskProject
try {
 [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
 New-Item -ItemType Directory -Force .cache/framework,.cache/verification|Out-Null
 $taskSources=@('scripts/reverse/framework/RankWireOracle.cs')
 foreach($taskFile in 'Packet.cs','ClassInfo.cs','SerializeWriter.cs','SerializeWriterHelper.cs','SerializedSizeCalculator.cs','SerializedSizeCalculatorHelper.cs','SerializeReader.cs','SerializeReaderHelper.cs','MessageAnalyzer.cs','MEMORYSTATUSEX.cs'){$taskSources+='legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net.Message/'+$taskFile}
 $taskSources+=@('legacy/decompiled/Core-8942a0c26a9a/Devcat.Core/EventArgs.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacket.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacketAnalyzer.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/ICryptoTransform.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/IMessage.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.RankServiceOperations/RankResultInfo.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.RankServiceOperations/RandomRankResultInfo.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.RankServiceOperations/RankAlarmInfo.cs')
 $taskOriginalHash='be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683'
 if((Get-FileHash '../ServidorVindictus/Server/zh-tw-x-gm/Bin/ServiceCore.dll').Hash.ToLowerInvariant()-cne$taskOriginalHash){throw 'Original assembly changed'}
 $taskCopies=@();$taskOriginals=@()
 foreach($taskType in @(@('QueryRandomRankInfoMessage',147,'051924be-e16a-3172-9a8b-0e65b13edb6f'),@('RandomRankInfoMessage',148,'7411ad56-8be6-3a81-bdd3-a69f242c3fe2'),@('QueryRankAlarmInfoMessage',333,'81ccd497-b4c1-35d7-b08c-77ddf2073089'),@('RankAlarmInfoMessage',334,'b1f6d4f1-e607-30fa-915f-1e5e1c14ed0c'),@('QueryRankAllInfoMessage',382,'76e547c7-7cf9-3523-96be-0d5367e969aa'),@('QueryRankCharacterInfoMessage',383,'d2180bc7-06fd-38e3-95e9-7801bcb01860'),@('QueryRankFavoritesInfoMessage',384,'22ab80f4-a54c-3a02-8a37-daad929429a9'),@('QueryRankGoalInfoMessage',385,'05c1e7bd-6448-31a6-9798-2d9c2cbec28e'),@('QueryRankHeavyFavoritesInfoMessage',386,'ecdf4e69-d15e-37a0-a873-534ca32e79dd'),@('QueryRankInfoMessage',387,'9a9b066c-03e8-3ebf-a2ab-e171a2b6665d'),@('QueryRankOtherCharacterInfoMessage',388,'46da8595-f780-3157-86e8-ed68f230bd60'),@('RankCharacterInfoMessage',389,'069fe707-679c-3fad-a66f-f2cae935c923'),@('RankFavoritesInfoMessage',390,'bb38aa48-cb2a-3db0-a98a-862a571abda7'),@('RankGoalInfoMessage',391,'5652a397-6ea3-3b22-9c37-7c6969a8902f'),@('RankHeavyFavoritesInfoMessage',392,'ff2edfd7-44a3-356d-9f05-6dcfbd9447a1'),@('RankAllInfoMessage',393,'61439335-cfa2-35b9-9d41-d26c4778dd90'),@('RankInfoMessage',394,'7d4ee0f1-991f-3ca7-ae3d-2fd9ec4be411'),@('RankOtherCharacterInfoMessage',395,'70712a6c-a1c3-330a-bb24-c8bb9d0b6830'),@('RequestSetRankFavoriteInfoMessage',396,'8824e598-669b-3370-87a4-1407030f5525'),@('RequestSetRankGoalInfoMessage',397,'af164256-d1f0-311f-9894-896845db1f36'))){
  foreach($taskArch in 'x86','x64'){$taskTable=Get-Content ('docs/evidence/reverse/framework-types-'+$taskArch+'.json') -Raw|ConvertFrom-Json;$taskRow=@($taskTable.messages|Where-Object name -eq ('ServiceCore.EndPointNetwork.'+$taskType[0]));if($taskTable.source_sha256-cne$taskOriginalHash-or$taskRow.Count-ne1-or$taskRow[0].category-ne$taskType[1]-or$taskRow[0].guid-cne$taskType[2]){throw 'Rank type identity changed'}}
  $taskSource='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/'+$taskType[0]+'.cs';$taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskSource));$taskCode=$taskCode.Replace('[Serializable]','[Serializable, System.Runtime.InteropServices.Guid("'+$taskType[2]+'")]');$taskCopy='.cache/framework/RankWire-'+$taskType[0]+'.cs';[IO.File]::WriteAllText((Join-Path $PWD $taskCopy),$taskCode,[Text.UTF8Encoding]::new($false));$taskCopies+=$taskCopy;$taskOriginals+=$taskSource
 }
 $taskProfiles=@()
 foreach($taskArch in 'x86','x64'){
  $taskExe='.cache/framework/RankWire-'+$taskArch+'.exe';$taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/unsafe+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
  foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+='/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLib+'.dll'}
  & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs @taskSources @taskCopies *> ('.cache/verification/rank-wire-build-'+$taskArch+'.log')
  if($LASTEXITCODE-ne0){throw 'Rank oracle build failed; see local log'}
  if(Select-String ('.cache/verification/rank-wire-build-'+$taskArch+'.log') -Pattern 'warning [A-Z]+[0-9]+:'){throw 'Rank oracle warning'}
  & (Join-Path $PWD $taskExe) *> ('.cache/verification/rank-wire-'+$taskArch+'.json')
  if($LASTEXITCODE-ne0){throw 'Rank oracle execution failed; see local log'}
  $taskProfiles+=Get-Content ('.cache/verification/rank-wire-'+$taskArch+'.json') -Raw|ConvertFrom-Json
 }
 if(($taskProfiles[0].wire_cases|ConvertTo-Json -Depth 30 -Compress)-cne($taskProfiles[1].wire_cases|ConvertTo-Json -Depth 30 -Compress)){throw 'Rank oracle x86/x64 disagree'}
 $taskAll=$taskSources+$taskOriginals+@('scripts/reverse/generate-rank-wire-fixtures.ps1')
 [ordered]@{profiles=$taskProfiles;source_sha256=$taskOriginalHash;sources=@($taskAll|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});original_services_executed=$false;method='Recovered Framework serializer and reader, ranking message fields, nested rank result/alarm/random types and nullability; synthetic values, no original services.'}|ConvertTo-Json -Depth 30 -Compress|Set-Content tests/fixtures/framework-rank-wire.json -Encoding utf8NoBOM
 Write-Output ('Rank oracle: '+$taskProfiles[0].wire_cases.Count+' wire cases per architecture; x86=x64; zero warnings.')
}finally{Pop-Location}
