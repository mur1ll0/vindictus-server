param([string]$EvidenceRoot)
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
# EvidenceRoot: the checkout holding the ignored legacy/decompiled tree (a worktree has none).
if(-not $EvidenceRoot){$EvidenceRoot=$taskProject}
$EvidenceRoot=[IO.Path]::GetFullPath($EvidenceRoot)
Push-Location $taskProject
try {
 New-Item -ItemType Directory -Force .cache/framework,.cache/verification|Out-Null
 $taskDecompiled=Join-Path $EvidenceRoot 'legacy/decompiled'
 $taskSources=@('scripts/reverse/framework/CharacterDeleteWireOracle.cs')
 foreach($taskFile in 'Packet.cs','ClassInfo.cs','SerializeWriter.cs','SerializeWriterHelper.cs','SerializedSizeCalculator.cs','SerializedSizeCalculatorHelper.cs','SerializeReader.cs','SerializeReaderHelper.cs','MessageAnalyzer.cs','MEMORYSTATUSEX.cs'){$taskSources+=Join-Path $taskDecompiled ('Core-8942a0c26a9a/Devcat.Core.Net.Message/'+$taskFile)}
 foreach($taskFile in 'Core-8942a0c26a9a/Devcat.Core/EventArgs.cs','Core-8942a0c26a9a/Devcat.Core.Net/IPacket.cs','Core-8942a0c26a9a/Devcat.Core.Net/IPacketAnalyzer.cs','Core-8942a0c26a9a/Devcat.Core.Net/ICryptoTransform.cs','ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/IMessage.cs','ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/DeleteCharacterResult.cs','ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork.CharacterList/DeleteCharacterCancelResult.cs'){$taskSources+=Join-Path $taskDecompiled $taskFile}
 $taskOriginalHash='be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683'
 if((Get-FileHash (Join-Path $EvidenceRoot '../ServidorVindictus/Server/zh-tw-x-gm/Bin/ServiceCore.dll')).Hash.ToLowerInvariant()-cne$taskOriginalHash){throw 'Original assembly changed'}
 $taskCopies=@();$taskOriginals=@()
 foreach($taskType in @(@('ServiceCore.EndPointNetwork','DeleteCharacterMessage',451,'65699026-e61b-38d9-a403-28178a907f59'),@('ServiceCore.EndPointNetwork.CharacterList','DeleteCharacterCancelMessage',258,'7a123c93-9f06-3bba-8e0e-1bf3d7a1d512'),@('ServiceCore.EndPointNetwork','DeleteCharacterResultMessage',407,'19af38d6-d58a-39e5-bfc7-7b5b88c50e4a'),@('ServiceCore.EndPointNetwork.CharacterList','DeleteCharacterCancelResultMessage',259,'8f6e7d11-8bdc-3e5b-9704-44e95b136338'),@('ServiceCore.EndPointNetwork','SelectOrdersChangeMessage',489,'d7d63e49-9742-3287-a815-7a0483760e81'))){
  foreach($taskArch in 'x86','x64'){$taskTable=Get-Content ('docs/evidence/reverse/framework-types-'+$taskArch+'.json') -Raw|ConvertFrom-Json;$taskRow=@($taskTable.messages|Where-Object name -eq ($taskType[0]+'.'+$taskType[1]));if($taskTable.source_sha256-cne$taskOriginalHash-or$taskRow.Count-ne1-or$taskRow[0].category-ne$taskType[2]-or$taskRow[0].guid-cne$taskType[3]){throw 'Character delete type identity changed'}}
  $taskSource=Join-Path $taskDecompiled ('ServiceCore-be65d59d2385/'+$taskType[0]+'/'+$taskType[1]+'.cs');$taskCode=[IO.File]::ReadAllText($taskSource);$taskCode=$taskCode.Replace('[Serializable]','[Serializable, System.Runtime.InteropServices.Guid("'+$taskType[3]+'")]');$taskCopy='.cache/framework/CharacterDeleteWire-'+$taskType[1]+'.cs';[IO.File]::WriteAllText((Join-Path $PWD $taskCopy),$taskCode,[Text.UTF8Encoding]::new($false));$taskCopies+=$taskCopy;$taskOriginals+=$taskSource
 }
 $taskProfiles=@()
 foreach($taskArch in 'x86','x64'){
  $taskExe='.cache/framework/CharacterDeleteWire-'+$taskArch+'.exe';$taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/unsafe+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
  foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+='/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLib+'.dll'}
  & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs @taskSources @taskCopies *> ('.cache/verification/character-delete-wire-build-'+$taskArch+'.log')
  if($LASTEXITCODE-ne0){throw 'Character delete oracle build failed; see local log'}
  if(Select-String ('.cache/verification/character-delete-wire-build-'+$taskArch+'.log') -Pattern 'warning [A-Z]+[0-9]+:'){throw 'Character delete oracle warning'}
  & (Join-Path $PWD $taskExe) *> ('.cache/verification/character-delete-wire-'+$taskArch+'.json')
  if($LASTEXITCODE-ne0){throw 'Character delete oracle execution failed; see local log'}
  $taskProfiles+=Get-Content ('.cache/verification/character-delete-wire-'+$taskArch+'.json') -Raw|ConvertFrom-Json
 }
 if(($taskProfiles[0].wire_cases|ConvertTo-Json -Depth 30 -Compress)-cne($taskProfiles[1].wire_cases|ConvertTo-Json -Depth 30 -Compress)){throw 'Character delete oracle x86/x64 disagree'}
 # Hashes are recorded by path relative to the evidence checkout, as the other fixtures do.
 $taskAll=@($taskSources+$taskOriginals)|ForEach-Object{if([IO.Path]::IsPathRooted($_)){[ordered]@{path=[IO.Path]::GetRelativePath($EvidenceRoot,$_).Replace('\','/');sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}}else{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}}}
 $taskAll+=[ordered]@{path='scripts/reverse/generate-character-delete-wire-fixtures.ps1';sha256=(Get-FileHash 'scripts/reverse/generate-character-delete-wire-fixtures.ps1').Hash.ToLowerInvariant()}
 [ordered]@{profiles=$taskProfiles;source_sha256=$taskOriginalHash;sources=@($taskAll);original_services_executed=$false;method='Recovered Framework serializer and reader, message fields and nullability; synthetic values, no original services.'}|ConvertTo-Json -Depth 30 -Compress|ForEach-Object{[IO.File]::WriteAllText((Join-Path $PWD 'tests/fixtures/framework-character-delete-wire.json'),($_+"`n"),[Text.UTF8Encoding]::new($false))}
 Write-Output ('Character delete oracle: '+$taskProfiles[0].wire_cases.Count+' wire cases per architecture; x86=x64; zero warnings.')
}finally{Pop-Location}
