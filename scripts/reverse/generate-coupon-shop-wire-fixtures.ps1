$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
Push-Location $taskProject
try {
 [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
 New-Item -ItemType Directory -Force .cache/framework,.cache/verification|Out-Null
 $taskSources=@('scripts/reverse/framework/CouponShopWireOracle.cs')
 foreach($taskFile in 'Packet.cs','ClassInfo.cs','SerializeWriter.cs','SerializeWriterHelper.cs','SerializedSizeCalculator.cs','SerializedSizeCalculatorHelper.cs','SerializeReader.cs','SerializeReaderHelper.cs','MessageAnalyzer.cs','MEMORYSTATUSEX.cs'){$taskSources+='legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net.Message/'+$taskFile}
 $taskSources+=@('legacy/decompiled/Core-8942a0c26a9a/Devcat.Core/EventArgs.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacket.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacketAnalyzer.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/ICryptoTransform.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/IMessage.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.ItemServiceOperations/ShopTimeRestrictedResult.cs')
 $taskOriginalHash='be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683'
 if((Get-FileHash '../ServidorVindictus/Server/zh-tw-x-gm/Bin/ServiceCore.dll').Hash.ToLowerInvariant()-cne$taskOriginalHash){throw 'Original assembly changed'}
 $taskCopies=@();$taskOriginals=@()
 foreach($taskType in @(@('CouponShopItemInfoQueryMessage',165,'ee913526-5550-3735-8cce-2157cafa92b7'),@('CouponShopItemInfoResultMessage',164,'fa215570-02da-3151-9471-c410aaebcdb9'),@('CouponShopItemBuyMessage',0,'d12c42b9-06a1-3033-b291-8a5d1de23c1c'))){
  foreach($taskArch in 'x86','x64'){$taskTable=Get-Content ('docs/evidence/reverse/framework-types-'+$taskArch+'.json') -Raw|ConvertFrom-Json;$taskRow=@($taskTable.messages|Where-Object name -eq ('ServiceCore.EndPointNetwork.'+$taskType[0]));if($taskTable.source_sha256-cne$taskOriginalHash-or$taskRow.Count-ne1-or$taskRow[0].category-ne$taskType[1]-or$taskRow[0].guid-cne$taskType[2]){throw 'Coupon shop type identity changed'}}
  $taskSource='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/'+$taskType[0]+'.cs';$taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskSource));$taskCode=$taskCode.Replace('[Serializable]','[Serializable, System.Runtime.InteropServices.Guid("'+$taskType[2]+'")]');$taskCopy='.cache/framework/CouponShopWire-'+$taskType[0]+'.cs';[IO.File]::WriteAllText((Join-Path $PWD $taskCopy),$taskCode,[Text.UTF8Encoding]::new($false));$taskCopies+=$taskCopy;$taskOriginals+=$taskSource
 }
 $taskProfiles=@()
 foreach($taskArch in 'x86','x64'){
  $taskExe='.cache/framework/CouponShopWire-'+$taskArch+'.exe';$taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/unsafe+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
  foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+='/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLib+'.dll'}
  & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs @taskSources @taskCopies *> ('.cache/verification/coupon-shop-wire-build-'+$taskArch+'.log')
  if($LASTEXITCODE-ne0){throw 'Coupon shop oracle build failed; see local log'}
  if(Select-String ('.cache/verification/coupon-shop-wire-build-'+$taskArch+'.log') -Pattern 'warning [A-Z]+[0-9]+:'){throw 'Coupon shop oracle warning'}
  & (Join-Path $PWD $taskExe) *> ('.cache/verification/coupon-shop-wire-'+$taskArch+'.json')
  if($LASTEXITCODE-ne0){throw 'Coupon shop oracle execution failed; see local log'}
  $taskProfiles+=Get-Content ('.cache/verification/coupon-shop-wire-'+$taskArch+'.json') -Raw|ConvertFrom-Json
 }
 if(($taskProfiles[0].wire_cases|ConvertTo-Json -Depth 30 -Compress)-cne($taskProfiles[1].wire_cases|ConvertTo-Json -Depth 30 -Compress)){throw 'Coupon shop oracle x86/x64 disagree'}
 $taskAll=$taskSources+$taskOriginals+@('scripts/reverse/generate-coupon-shop-wire-fixtures.ps1')
 [ordered]@{profiles=$taskProfiles;source_sha256=$taskOriginalHash;sources=@($taskAll|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});original_services_executed=$false;method='Recovered Framework serializer and reader, coupon shop message fields and nullability; synthetic values, no original services.'}|ConvertTo-Json -Depth 30 -Compress|Set-Content tests/fixtures/framework-coupon-shop-wire.json -Encoding utf8NoBOM
 Write-Output ('Coupon shop oracle: '+$taskProfiles[0].wire_cases.Count+' wire cases per architecture; x86=x64; zero warnings.')
}finally{Pop-Location}
