$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
Push-Location $taskProject
try {
 New-Item -ItemType Directory -Force .cache/framework,.cache/verification|Out-Null
 $taskSources=@('scripts/reverse/framework/MailWireOracle.cs')
 foreach($taskFile in 'Packet.cs','ClassInfo.cs','SerializeWriter.cs','SerializeWriterHelper.cs','SerializedSizeCalculator.cs','SerializedSizeCalculatorHelper.cs','SerializeReader.cs','SerializeReaderHelper.cs','MessageAnalyzer.cs','MEMORYSTATUSEX.cs'){$taskSources+='legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net.Message/'+$taskFile}
 $taskSources+=@('legacy/decompiled/Core-8942a0c26a9a/Devcat.Core/EventArgs.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacket.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacketAnalyzer.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/ICryptoTransform.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/IMessage.cs')
 foreach($taskFile in 'BriefMailInfo.cs','MailItemInfo.cs','AttachedTransferItemInfo.cs'){$taskSources+='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/'+$taskFile}
 $taskSources+=@('legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.ItemServiceOperations/TransferItemInfo.cs','legacy/decompiled/Utility-37359981d509/Utility/Extention.cs')
 $taskOriginalHash='be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683'
 if((Get-FileHash '../ServidorVindictus/Server/zh-tw-x-gm/Bin/ServiceCore.dll').Hash.ToLowerInvariant()-cne$taskOriginalHash){throw 'Original assembly changed'}
 $taskCopies=@();$taskOriginals=@()
 foreach($taskType in @(@('QueryMailListMessage',530,'6f7864f6-5e49-33a3-a899-6a6801411695'),@('QueryMailInfoMessage',531,'55a3b131-002a-3a3a-9e65-14132b97951d'),@('SendMailMessage',567,'fc081bf4-efe6-312b-9de9-b6c07f604817'),@('GetMailItemMessage',464,'5c2de52a-ae2c-3c6e-9060-14ca27b9eef3'),@('DeleteMailMessage',452,'316f4412-0b62-3af6-9ea6-cf98aecce9cb'),@('ReturnMailMessage',561,'6eb38330-0409-37d3-9c8b-8062e9f45d48'),@('MailListMessage',495,'74393467-8b2a-3100-9eb1-b5a34ec3310b'),@('MailInfoMessage',494,'720d93db-8a10-349d-9f31-81ae382857a6'),@('MailSentMessage',497,'06e86b5b-c00a-335e-a28f-92f6a517b156'),@('MailReceivedMessage',496,'67e4050b-e402-3a58-be39-fce090eb16dd'),@('GetMailItemCompletedMessage',426,'203f4aaf-d073-3e30-8f1f-4d5458b4e338'))){
  foreach($taskArch in 'x86','x64'){$taskTable=Get-Content ('docs/evidence/reverse/framework-types-'+$taskArch+'.json') -Raw|ConvertFrom-Json;$taskRow=@($taskTable.messages|Where-Object name -eq ('ServiceCore.EndPointNetwork.'+$taskType[0]));if($taskTable.source_sha256-cne$taskOriginalHash-or$taskRow.Count-ne1-or$taskRow[0].category-ne$taskType[1]-or$taskRow[0].guid-cne$taskType[2]){throw 'Mail type identity changed'}}
  $taskSource='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/'+$taskType[0]+'.cs';$taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskSource));$taskCode=$taskCode.Replace('[Serializable]','[Serializable, System.Runtime.InteropServices.Guid("'+$taskType[2]+'")]');$taskCopy='.cache/framework/MailWire-'+$taskType[0]+'.cs';[IO.File]::WriteAllText((Join-Path $PWD $taskCopy),$taskCode,[Text.UTF8Encoding]::new($false));$taskCopies+=$taskCopy;$taskOriginals+=$taskSource
 }
 $taskProfiles=@()
 foreach($taskArch in 'x86','x64'){
  $taskExe='.cache/framework/MailWire-'+$taskArch+'.exe';$taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/unsafe+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
  foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+='/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLib+'.dll'}
  & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs @taskSources @taskCopies *> ('.cache/verification/mail-wire-build-'+$taskArch+'.log')
  if($LASTEXITCODE-ne0){throw 'Mail oracle build failed; see local log'}
  if(Select-String ('.cache/verification/mail-wire-build-'+$taskArch+'.log') -Pattern 'warning [A-Z]+[0-9]+:'){throw 'Mail oracle warning'}
  & (Join-Path $PWD $taskExe) *> ('.cache/verification/mail-wire-'+$taskArch+'.json')
  if($LASTEXITCODE-ne0){throw 'Mail oracle execution failed; see local log'}
  $taskProfiles+=Get-Content ('.cache/verification/mail-wire-'+$taskArch+'.json') -Raw|ConvertFrom-Json
 }
 if(($taskProfiles[0].wire_cases|ConvertTo-Json -Depth 30 -Compress)-cne($taskProfiles[1].wire_cases|ConvertTo-Json -Depth 30 -Compress)){throw 'Mail oracle x86/x64 disagree'}
 $taskAll=$taskSources+$taskOriginals+@('scripts/reverse/generate-mail-wire-fixtures.ps1')
 [ordered]@{profiles=$taskProfiles;source_sha256=$taskOriginalHash;sources=@($taskAll|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});original_services_executed=$false;method='Recovered Framework serializer and reader, mail message fields and nullability, MailBox.SendMail fee term; synthetic values, no original services.'}|ConvertTo-Json -Depth 30 -Compress|Set-Content tests/fixtures/framework-mail-wire.json -Encoding utf8NoBOM
 Write-Output ('Mail oracle: '+$taskProfiles[0].wire_cases.Count+' wire cases per architecture; x86=x64; zero warnings.')
}finally{Pop-Location}
