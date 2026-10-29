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
 $taskBin=Join-Path $EvidenceRoot '../ServidorVindictus/Server/zh-tw-x-gm/Bin'
 foreach($taskAssembly in @(@('ServiceCore.dll','be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683'),@('PlayerService.dll','f23d8b9df77c'),@('Utility.dll','37359981d509'))){
  if(-not (Get-FileHash (Join-Path $taskBin $taskAssembly[0])).Hash.ToLowerInvariant().StartsWith($taskAssembly[1])){throw 'Original assembly changed'}
 }
 $taskOriginalHash='be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683'
 $taskSources=@()
 foreach($taskFile in 'Packet.cs','ClassInfo.cs','SerializeWriter.cs','SerializeWriterHelper.cs','SerializedSizeCalculator.cs','SerializedSizeCalculatorHelper.cs','SerializeReader.cs','SerializeReaderHelper.cs','MessageAnalyzer.cs','MEMORYSTATUSEX.cs'){$taskSources+=Join-Path $taskDecompiled ('Core-8942a0c26a9a/Devcat.Core.Net.Message/'+$taskFile)}
 foreach($taskFile in 'Core-8942a0c26a9a/Devcat.Core/EventArgs.cs','Core-8942a0c26a9a/Devcat.Core.Net/IPacket.cs','Core-8942a0c26a9a/Devcat.Core.Net/IPacketAnalyzer.cs','Core-8942a0c26a9a/Devcat.Core.Net/ICryptoTransform.cs','ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/IMessage.cs','PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore.Static/DistortTable.cs'){$taskSources+=Join-Path $taskDecompiled $taskFile}
 # Utility.DictionaryExtension.TryGetValue(IDictionary, key) only; the rest needs MultiDictionary.
 $taskExtension=Join-Path $taskDecompiled 'Utility-37359981d509/Utility/DictionaryExtension.cs'
 $taskText=[IO.File]::ReadAllText($taskExtension);$taskStart=$taskText.IndexOf('public static V TryGetValue<T, V>(this IDictionary<T, V> dictionary, T key)');$taskEnd=$taskText.IndexOf('public static HashSet<V> TryGetValue<T, V>(this MultiDictionary')
 if($taskStart-lt0-or$taskEnd-le$taskStart){throw 'DictionaryExtension structure changed'}
 $taskTemplate='scripts/reverse/framework/DyeAmpleOracle.cs.in'
 $taskCode=[IO.File]::ReadAllText((Join-Path $PWD $taskTemplate)).Replace('__TRYGETVALUE__',$taskText.Substring($taskStart,$taskEnd-$taskStart).TrimEnd())
 [IO.File]::WriteAllText((Join-Path $PWD '.cache/framework/DyeAmpleOracle.cs'),$taskCode,[Text.UTF8Encoding]::new($false))
 $taskCopies=@('.cache/framework/DyeAmpleOracle.cs');$taskOriginals=@($taskExtension)
 foreach($taskType in @(@('DyeAmpleUsedMessage',270,'25fbb3e8-bab4-3548-a0a1-224aa97b4f25'),@('DyeAmpleRequestMessage',269,'c55d23ca-6885-3155-9375-5035631f3ff5'),@('DyeAmpleCompleteMessage',268,'c9765023-10d1-3a0a-bf97-6b45e7312258'))){
  foreach($taskArch in 'x86','x64'){$taskTable=Get-Content ('docs/evidence/reverse/framework-types-'+$taskArch+'.json') -Raw|ConvertFrom-Json;$taskRow=@($taskTable.messages|Where-Object name -eq ('ServiceCore.EndPointNetwork.'+$taskType[0]));if($taskTable.source_sha256-cne$taskOriginalHash-or$taskRow.Count-ne1-or$taskRow[0].category-ne$taskType[1]-or$taskRow[0].guid-cne$taskType[2]){throw 'Dye ampoule type identity changed'}}
  $taskSource=Join-Path $taskDecompiled ('ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/'+$taskType[0]+'.cs');$taskMessage=[IO.File]::ReadAllText($taskSource);$taskMessage=$taskMessage.Replace('[Serializable]','[Serializable, System.Runtime.InteropServices.Guid("'+$taskType[2]+'")]');$taskCopy='.cache/framework/DyeAmple-'+$taskType[0]+'.cs';[IO.File]::WriteAllText((Join-Path $PWD $taskCopy),$taskMessage,[Text.UTF8Encoding]::new($false));$taskCopies+=$taskCopy;$taskOriginals+=$taskSource
 }
 $taskProfiles=@()
 foreach($taskArch in 'x86','x64'){
  $taskExe='.cache/framework/DyeAmpleOracle-'+$taskArch+'.exe';$taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/unsafe+','/checked-','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
  foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+='/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLib+'.dll'}
  & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs @taskSources @taskCopies *> ('.cache/verification/dye-ample-build-'+$taskArch+'.log')
  if($LASTEXITCODE-ne0){throw 'Dye ampoule oracle build failed; see local log'}
  if(Select-String ('.cache/verification/dye-ample-build-'+$taskArch+'.log') -Pattern 'warning [A-Z]+[0-9]+:'){throw 'Dye ampoule oracle warning'}
  & (Join-Path $PWD $taskExe) *> ('.cache/verification/dye-ample-'+$taskArch+'.json')
  if($LASTEXITCODE-ne0){throw 'Dye ampoule oracle execution failed; see local log'}
  $taskProfiles+=Get-Content ('.cache/verification/dye-ample-'+$taskArch+'.json') -Raw|ConvertFrom-Json
 }
 foreach($taskPart in 'wire_cases','undistort','int_parse'){if(($taskProfiles[0].$taskPart|ConvertTo-Json -Depth 30 -Compress)-cne($taskProfiles[1].$taskPart|ConvertTo-Json -Depth 30 -Compress)){throw 'Dye ampoule oracle x86/x64 disagree'}}
 # Hashes are recorded by path relative to the evidence checkout, as the other fixtures do.
 $taskAll=@($taskSources+$taskOriginals)|ForEach-Object{if([IO.Path]::IsPathRooted($_)){[ordered]@{path=[IO.Path]::GetRelativePath($EvidenceRoot,$_).Replace('\','/');sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}}else{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}}}
 foreach($taskOwn in $taskTemplate,'scripts/reverse/generate-dye-ample-fixtures.ps1'){$taskAll+=[ordered]@{path=$taskOwn;sha256=(Get-FileHash $taskOwn).Hash.ToLowerInvariant()}}
 [ordered]@{profiles=$taskProfiles;source_sha256=$taskOriginalHash;sources=@($taskAll);original_services_executed=$false;method='Recovered Framework serializer and reader, the three dye ampoule messages, DistortTable.Undistort over synthetic tables and Framework int.TryParse; synthetic values, no original services or material files.'}|ConvertTo-Json -Depth 30 -Compress|ForEach-Object{[IO.File]::WriteAllText((Join-Path $PWD 'tests/fixtures/framework-dye-ample.json'),($_+"`n"),[Text.UTF8Encoding]::new($false))}
 Write-Output ('Dye ampoule oracle: '+$taskProfiles[0].wire_cases.Count+' wire cases, '+$taskProfiles[0].undistort.Count+' undistort cases, '+$taskProfiles[0].int_parse.Count+' parses per architecture; x86=x64; zero warnings.')
}finally{Pop-Location}
