$ErrorActionPreference='Stop'
$taskSources=@('scripts/reverse/framework/CharacterCreationWireOracle.cs')
foreach($taskFile in 'Packet.cs','ClassInfo.cs','SerializeWriter.cs','SerializeWriterHelper.cs','SerializedSizeCalculator.cs','SerializedSizeCalculatorHelper.cs','SerializeReader.cs','SerializeReaderHelper.cs'){$taskSources+=('legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net.Message/'+$taskFile)}
$taskSources+=@('legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/IPacket.cs','legacy/decompiled/Core-8942a0c26a9a/Devcat.Core.Net/ICryptoTransform.cs','legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/IMessage.cs')
$taskMetadata=Get-Content docs/evidence/reverse/framework-types-x64.json -Raw|ConvertFrom-Json
$taskX86=Get-Content docs/evidence/reverse/framework-types-x86.json -Raw|ConvertFrom-Json
$taskOriginalHash='be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683'
if((Get-FileHash ../ServidorVindictus/Server/zh-tw-x-gm/Bin/ServiceCore.dll).Hash.ToLowerInvariant()-cne$taskOriginalHash){throw 'Original assembly hash changed'}
$taskOriginalPaths=@()
$taskProfiles=@()
$taskCopies=@()
foreach($taskName in 'CreateCharacterMessage','CreateCharacterFailMessage','CharacterTemplate','CustomizeItemRequestInfo','CreateCharacterResult'){
 $taskPath='legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/'+$taskName+'.cs';$taskOriginalPaths+=$taskPath;$taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskPath))
 $taskRow=@($taskMetadata.messages|Where-Object name -eq ('ServiceCore.EndPointNetwork.'+$taskName))
 if($taskRow.Count-eq1){
  $taskOther=@($taskX86.messages|Where-Object name -eq ('ServiceCore.EndPointNetwork.'+$taskName))
  if($taskOther.Count-ne1-or$taskOther[0].guid-cne$taskRow[0].guid-or$taskOther[0].category-ne$taskRow[0].category){throw 'Creation metadata architecture disagreement'}
  $taskCode=$taskCode.Replace('[Serializable]','[Serializable, System.Runtime.InteropServices.Guid("'+$taskRow[0].guid+'")]')}
 $taskCopy='.cache/framework/CharacterCreation-'+$taskName+'.cs';[IO.File]::WriteAllText((Join-Path $PWD $taskCopy),$taskCode,[Text.UTF8Encoding]::new($false));$taskCopies+=$taskCopy
}
foreach($taskArch in 'x86','x64'){
 $taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/unsafe+','/langversion:latest',('/platform:'+$taskArch),('/out:.cache/framework/CharacterCreationWire-'+$taskArch+'.exe'))
 foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLib+'.dll')}
 $taskArgs+=$taskSources;$taskArgs+=$taskCopies
 & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs *> ('.cache/verification/character-creation-wire-build-'+$taskArch+'.log');if($LASTEXITCODE-ne0){throw 'Creation oracle compilation failed; local log only'}
 if(Select-String ('.cache/verification/character-creation-wire-build-'+$taskArch+'.log') -Pattern 'warning [A-Z]+[0-9]+:'){throw 'Creation oracle compiler warnings'}
 & (Join-Path $PWD ('.cache/framework/CharacterCreationWire-'+$taskArch+'.exe')) *> ('.cache/verification/character-creation-wire-'+$taskArch+'.json');if($LASTEXITCODE-ne0){throw 'Creation oracle failed; local log only'}
 $taskObserved=Get-Content ('.cache/verification/character-creation-wire-'+$taskArch+'.json') -Raw|ConvertFrom-Json
 if($taskObserved.cases.Count-ne255){throw 'Creation case count differs'}
 $taskProfiles+=$taskObserved
 Write-Output ('Creation wire original:'+ $taskArch+'; cases='+$taskObserved.cases.Count+'; source DTO/serializer roundtrip passed.')
}

if(($taskProfiles[0].cases|ConvertTo-Json -Depth 16 -Compress)-cne($taskProfiles[1].cases|ConvertTo-Json -Depth 16 -Compress)){throw 'Creation architecture disagreement'}
[ordered]@{profiles=$taskProfiles;original_assembly_sha256=$taskOriginalHash;sources=@(($taskSources+$taskOriginalPaths+@('scripts/reverse/generate-character-creation-wire-fixtures.ps1'))|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});original_services_executed=$false}|ConvertTo-Json -Depth 18 -Compress|Set-Content tests/fixtures/framework-character-creation-wire.json -Encoding utf8NoBOM
