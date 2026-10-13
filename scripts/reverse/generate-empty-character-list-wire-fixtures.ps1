$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try {
New-Item -ItemType Directory -Force .cache/framework,.cache/verification|Out-Null
$taskTypes=@(,@('CharacterListMessage',479,'f2750037-d9ab-3b34-9697-e1626e224040'))
$taskSources=@('scripts/reverse/framework/EmptyCharacterListWireOracle.cs')
foreach($taskFile in 'Packet.cs','ClassInfo.cs','SerializeWriter.cs','SerializeWriterHelper.cs','SerializedSizeCalculator.cs','SerializedSizeCalculatorHelper.cs','SerializeReader.cs','SerializeReaderHelper.cs'){$taskSources+=('legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net.Message/'+$taskFile)}
$taskSources+=@('legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacket.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/ICryptoTransform.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/IMessage.cs')
foreach($taskFile in 'CostumeInfo.cs','PetStatusInfo.cs','PetStatElement.cs','PetSkillElement.cs','PetAccessoryElement.cs'){$taskSources+=('legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/'+$taskFile)}
foreach($taskFile in 'CharacterSummary.cs','BaseCharacter.cs','VocationEnum.cs'){$taskSources+=('legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.CharacterServiceOperations/'+$taskFile)}
$taskOriginalHash='be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683'
if((Get-FileHash '../ServidorVindictus/Server/zh-tw-x-gm/Bin/ServiceCore.dll').Hash.ToLowerInvariant()-ne$taskOriginalHash){throw 'Original metadata assembly hash changed'}
$taskCopies=@()
foreach($taskType in $taskTypes){
 foreach($taskArch in 'x86','x64'){
  $taskTable=Get-Content ('docs/evidence/reverse/framework-types-'+$taskArch+'.json') -Raw|ConvertFrom-Json
  $taskRow=@($taskTable.messages|Where-Object name -eq ('ServiceCore.EndPointNetwork.'+$taskType[0]))
  if($taskTable.source_sha256-ne$taskOriginalHash-or$taskRow.Count-ne1-or$taskRow[0].category-ne$taskType[1]-or$taskRow[0].guid-ne$taskType[2]){throw 'Original type identity mismatch'}
 }
 $taskSource='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/'+$taskType[0]+'.cs'
 $taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskSource))
 if(([regex]::Matches($taskCode,'\[Serializable\]')).Count-ne1){throw 'Unexpected source type shape'}
 $taskCode=$taskCode.Replace('[Serializable]','[Serializable, System.Runtime.InteropServices.Guid("'+$taskType[2]+'")]')
 $taskCopy='.cache/framework/Wire-'+$taskType[0]+'.cs'
 [IO.File]::WriteAllText((Join-Path $PWD $taskCopy),$taskCode,[Text.UTF8Encoding]::new($false))
 $taskSources+=$taskSource;$taskCopies+=$taskCopy
}
$taskProfiles=@()
foreach($taskArch in 'x86','x64'){
 $taskExe='.cache/framework/EmptyCharacterListWire-'+$taskArch+'.exe'
 $taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/unsafe+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
 foreach($taskLibrary in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLibrary+'.dll')}
 $taskArgs+=@($taskSources|Where-Object{$_-notlike '*ServiceCore.EndPointNetwork/CharacterListMessage.cs'});$taskArgs+=$taskCopies
 & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs *> ('.cache/verification/empty-character-list-wire-build-'+$taskArch+'.log')
 if($LASTEXITCODE-ne0){throw 'Password wire oracle compilation failed; local log'}
 if((Get-Content ('.cache/verification/empty-character-list-wire-build-'+$taskArch+'.log') -Raw)-match '(?i)warning'){throw 'Oracle compilation warning'}
 & (Join-Path $PWD $taskExe) *> ('.cache/verification/empty-character-list-wire-'+$taskArch+'.json')
 if($LASTEXITCODE-ne0){throw 'Password wire oracle execution failed; local log'}
 $taskProfile=Get-Content ('.cache/verification/empty-character-list-wire-'+$taskArch+'.json') -Raw|ConvertFrom-Json
 if($taskProfile.architecture-ne$taskArch-or$taskProfile.cases.Count-ne183){throw 'Oracle case count differs'}
 $taskProfiles+=$taskProfile
}
if(($taskProfiles[0].cases|ConvertTo-Json -Depth 7 -Compress)-cne($taskProfiles[1].cases|ConvertTo-Json -Depth 7 -Compress)){throw 'Wire profiles disagree'}
$taskResult=[ordered]@{profiles=$taskProfiles;source_sha256=$taskOriginalHash;sources=@($taskSources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});original_services_executed=$false;method='Recovered Framework serializer/reader; message copies only pin original metadata GUID; synthetic fields and allocator shim.'}
$taskResult|ConvertTo-Json -Depth 9 -Compress|Set-Content 'tests/fixtures/framework-empty-character-list-wire.json' -Encoding utf8NoBOM
Write-Host 'Empty CharacterList oracle:2 architectures;183cases/profile;0wire disagreements.'

} finally {Pop-Location}
