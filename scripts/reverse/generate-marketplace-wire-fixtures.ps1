$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
Push-Location $taskProject
try {
 [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
 New-Item -ItemType Directory -Force .cache/framework,.cache/verification|Out-Null
 $taskSources=@('scripts/reverse/framework/MarketplaceWireOracle.cs')
 foreach($taskFile in 'Packet.cs','ClassInfo.cs','SerializeWriter.cs','SerializeWriterHelper.cs','SerializedSizeCalculator.cs','SerializedSizeCalculatorHelper.cs','SerializeReader.cs','SerializeReaderHelper.cs','MessageAnalyzer.cs','MEMORYSTATUSEX.cs'){$taskSources+='legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net.Message/'+$taskFile}
 $taskSources+=@('legacy/decompiled/Core-8942a0c26a9a/Devcat.Core/EventArgs.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacket.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacketAnalyzer.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/ICryptoTransform.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/IMessage.cs')
 foreach($taskFile in 'DetailOption.cs','TradeItemInfo.cs','SortOrder.cs'){$taskSources+='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/'+$taskFile}
 $taskOriginalHash='be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683'
 if((Get-FileHash '../ServidorVindictus/Server/zh-tw-x-gm/Bin/ServiceCore.dll').Hash.ToLowerInvariant()-cne$taskOriginalHash){throw 'Original assembly changed'}
 $taskCopies=@();$taskOriginals=@()
 foreach($taskType in @(@('TradeCategorySearchMessage',305,'77610fcf-fc87-3633-8f8e-de283eff2e6f'),@('TradeItemClassListSearchMessage',306,'92300b56-a6fc-35ec-aa0c-c344e2615e6f'),@('TradeCancelMyItemMessage',302,'63d59401-6b18-305a-af2f-98a77d2d977b'),@('TradeCancelMyItemResultMessage',303,'bbae86b3-29cf-3b81-afd5-5736213f0c2d'),@('TradeRequestMyItemMessage',311,'e0188571-a1a3-3b93-8772-9c44ad8d1114'),@('TradeMyItemInfos',308,'1d431b50-2a5d-3e6e-829f-8c1a5057b5b3'),@('TradePurchaseItemMessage',309,'6d284225-0676-3001-9369-ba59fe31cb4f'),@('TradePurchaseItemResultMessage',310,'813c7eaf-a144-334e-81cd-9d066f11000e'),@('TradeSearchResult',312,'f6cbae04-367a-3926-b010-aaed1b28790a'),@('InsertTradeOrderMessage',427,'5dd5d7fb-1389-353f-8bcf-7d2fbd9edb82'),@('InsertTradeOrderResultMessage',428,'7ab66731-d827-3eee-bcec-1c50b5a066fc'),@('InsertTradeTirOrderMessage',429,'97f6218c-8733-3b4b-bdd7-af26cf089413'),@('StartTradeSessionMessage',592,'99350e05-2420-364a-a3bc-4be82c6f1cf7'),@('EndTradeSessionMessage',594,'84675e2c-42fe-3895-9f1f-b913dd2d1918'))){
  foreach($taskArch in 'x86','x64'){$taskTable=Get-Content ('docs/evidence/reverse/framework-types-'+$taskArch+'.json') -Raw|ConvertFrom-Json;$taskRow=@($taskTable.messages|Where-Object name -eq ('ServiceCore.EndPointNetwork.'+$taskType[0]));if($taskTable.source_sha256-cne$taskOriginalHash-or$taskRow.Count-ne1-or$taskRow[0].category-ne$taskType[1]-or$taskRow[0].guid-cne$taskType[2]){throw 'Marketplace type identity changed'}}
  $taskSource='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/'+$taskType[0]+'.cs';$taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskSource));$taskCode=$taskCode.Replace('[Serializable]','[Serializable, System.Runtime.InteropServices.Guid("'+$taskType[2]+'")]');$taskCopy='.cache/framework/MarketplaceWire-'+$taskType[0]+'.cs';[IO.File]::WriteAllText((Join-Path $PWD $taskCopy),$taskCode,[Text.UTF8Encoding]::new($false));$taskCopies+=$taskCopy;$taskOriginals+=$taskSource
 }
 $taskProfiles=@()
 foreach($taskArch in 'x86','x64'){
  $taskExe='.cache/framework/MarketplaceWire-'+$taskArch+'.exe';$taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/unsafe+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
  foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+='/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLib+'.dll'}
  & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs @taskSources @taskCopies *> ('.cache/verification/marketplace-wire-build-'+$taskArch+'.log')
  if($LASTEXITCODE-ne0){throw 'Marketplace oracle build failed; see local log'}
  if(Select-String ('.cache/verification/marketplace-wire-build-'+$taskArch+'.log') -Pattern 'warning [A-Z]+[0-9]+:'){throw 'Marketplace oracle warning'}
  & (Join-Path $PWD $taskExe) *> ('.cache/verification/marketplace-wire-'+$taskArch+'.json')
  if($LASTEXITCODE-ne0){throw 'Marketplace oracle execution failed; see local log'}
  $taskProfiles+=Get-Content ('.cache/verification/marketplace-wire-'+$taskArch+'.json') -Raw|ConvertFrom-Json
 }
 if(($taskProfiles[0].wire_cases|ConvertTo-Json -Depth 30 -Compress)-cne($taskProfiles[1].wire_cases|ConvertTo-Json -Depth 30 -Compress)){throw 'Marketplace oracle x86/x64 disagree'}
 $taskAll=$taskSources+$taskOriginals+@('scripts/reverse/generate-marketplace-wire-fixtures.ps1')
 [ordered]@{profiles=$taskProfiles;source_sha256=$taskOriginalHash;sources=@($taskAll|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});original_services_executed=$false;method='Recovered Framework serializer and reader, trade message fields and nullability; synthetic values, no original services.'}|ConvertTo-Json -Depth 30 -Compress|Set-Content tests/fixtures/framework-marketplace-wire.json -Encoding utf8NoBOM
 Write-Output ('Marketplace oracle: '+$taskProfiles[0].wire_cases.Count+' wire cases per architecture; x86=x64; zero warnings.')
}finally{Pop-Location}
