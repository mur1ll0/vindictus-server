$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
Push-Location $taskRoot
try {
 New-Item -ItemType Directory -Force .cache/framework,.cache/verification|Out-Null
 function SourceMethod([string]$path,[string]$signature){
  $s=[IO.File]::ReadAllText((Join-Path $taskRoot $path));$i=$s.IndexOf($signature,[StringComparison]::Ordinal)
  if($i-lt0){throw 'Source method missing'}
  $b=$s.IndexOf('{',$i);$depth=1;$end=$b+1
  while($depth-gt0-and$end-lt$s.Length){if($s[$end]-eq'{'){$depth++};if($s[$end]-eq'}'){$depth--};$end++}
  if($depth-ne0){throw 'Source method braces changed'}
  return $s.Substring($i,$end-$i)
 }
 $taskSlot='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.ItemServiceCore/EquippableSlot.cs'
 $taskTemplate='scripts/reverse/framework/EquipmentSlotOracle.cs.in'
 $taskCode=[IO.File]::ReadAllText((Join-Path $taskRoot $taskTemplate))
 foreach($taskPair in @(@('__CONSTRUCTOR__','public EquippableSlot('),@('__LOAD_ITEM__','public void Load(Item data)'),@('__LOAD_EXPANSION__','public void Load(ExpandSlotInfo unlocked)'),@('__CLEAR__','public void Clear()'),@('__CHECK__','public ItemOperationResult CheckSettable('),@('__SET__','public ItemOperationResult Set(Item item)'),@('__SET_CHECK__','public ItemOperationResult Set(Item item, bool skipCheck)'),@('__RESET__','public ItemOperationResult Reset('),@('__UNLOCK__','public void Unlock()'))){$taskCode=$taskCode.Replace($taskPair[0],(SourceMethod $taskSlot $taskPair[1]))}
 [IO.File]::WriteAllText((Join-Path $taskRoot '.cache/framework/EquipmentSlotOracle.cs'),$taskCode,[Text.UTF8Encoding]::new($false))
 foreach($taskArch in 'x86','x64'){
  $taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/checked-','/langversion:latest',('/platform:'+$taskArch),('/out:.cache/framework/EquipmentSlotOracle-'+$taskArch+'.exe'))
  foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLib+'.dll')}
  $taskArgs+='.cache/framework/EquipmentSlotOracle.cs'
  & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs *> ('.cache/verification/equipment-slot-build-'+$taskArch+'.log')
  if($LASTEXITCODE-ne0){throw 'Equipment slot oracle compilation failed; inspect retained log'}
  & (Join-Path $taskRoot ('.cache/framework/EquipmentSlotOracle-'+$taskArch+'.exe')) *> ('.cache/verification/equipment-slot-'+$taskArch+'.json')
  if($LASTEXITCODE-ne0){throw 'Equipment slot oracle execution failed; inspect retained log'}
 }
 $taskX=Get-Content .cache/verification/equipment-slot-x86.json -Raw|ConvertFrom-Json -DateKind String
 $taskY=Get-Content .cache/verification/equipment-slot-x64.json -Raw|ConvertFrom-Json -DateKind String
 if($taskX.Count-ne128-or$taskY.Count-ne128){throw 'Unexpected equipment slot case count'}
 $taskPaths=@($taskSlot,$taskTemplate,'scripts/reverse/generate-equipment-slot-fixtures.ps1')
 $taskHashes=@($taskPaths|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash -LiteralPath $_).Hash.ToLowerInvariant()}})
 $taskResult=[ordered]@{scope='Original EquippableSlot constructor/load/check/set/clear/reset/unlock methods with synthetic metadata, items and traced attribute/enhancement/owner effects. No SQL, constraints, processor, character actor or network execution.';source_sha256=$taskHashes;x86=$taskX;x64=$taskY}|ConvertTo-Json -Depth 20 -Compress
 [IO.File]::WriteAllText((Join-Path $taskRoot 'tests/fixtures/framework-equipment-slot.json'),($taskResult+"`n"),[Text.UTF8Encoding]::new($false))
 Write-Output 'Equipment slot oracle:128 cases per architecture; original source methods, synthetic providers and traced effect order.'
}finally{Pop-Location}
