$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try {
$taskText=[IO.File]::ReadAllText((Join-Path $PWD 'legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore/FeatureMatrix.cs'))
$taskBegin=$taskText.IndexOf('public static bool IsEnable(string featureName)');$taskEnd=$taskText.IndexOf('public static string GetString(string featureName)',$taskBegin)
if($taskBegin-lt0-or$taskEnd-le$taskBegin){throw 'Recovered feature method boundaries changed'}
$taskCode=@"
using System;
using System.Linq;
using System.Globalization;
using System.Collections.Generic;
using System.Web.Script.Serialization;
class FeatureMatrix {
 public static string CurrentVer="3.09.00";
 public static Dictionary<string,string> Map=new Dictionary<string,string>();
 public static string GetString(string k){string v;return Map.TryGetValue(k,out v)?v:"";}
$($taskText.Substring($taskBegin,$taskEnd-$taskBegin))
}
class LobbyFeatureOracle {
 static void Main(){var cases=new List<object>();foreach(string culture in new[]{"","en-US","zh-TW","zh-CN","tr-TR"}){
 CultureInfo.CurrentCulture=CultureInfo.GetCultureInfo(culture);
 foreach(string value in new[]{"2.00.14","2.01.04","2.02.09"}){FeatureMatrix.Map["syntheticFeature"]=value;cases.Add(new{culture,current=FeatureMatrix.CurrentVer,value,enabled=FeatureMatrix.IsEnable("syntheticFeature")});}}
 Console.WriteLine(new JavaScriptSerializer().Serialize(new{architecture=IntPtr.Size==4?"x86":"x64",cases}));}
}
"@
[IO.File]::WriteAllText((Join-Path $PWD '.cache/framework/LobbyFeatureOracle.cs'),$taskCode,[Text.UTF8Encoding]::new($false))
foreach($taskArch in 'x86','x64'){
 $taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$taskArch),('/out:.cache/framework/LobbyFeatureOracle-'+$taskArch+'.exe'))
 foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLib+'.dll')}
 $taskArgs+='.cache/framework/LobbyFeatureOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs *> ('.cache/verification/lobby-feature-build-'+$taskArch+'.log');if($LASTEXITCODE-ne0){throw 'Feature oracle build failed; local log'}
 & (Join-Path $PWD ('.cache/framework/LobbyFeatureOracle-'+$taskArch+'.exe')) *> ('.cache/verification/lobby-feature-'+$taskArch+'.json');if($LASTEXITCODE-ne0){throw 'Feature oracle failed; local log'}
}
Write-Host 'Recovered lobby features:2 architectures x5cultures x3pairs; outputs local.'

$taskProfiles=@('x86','x64')|ForEach-Object{Get-Content ('.cache/verification/lobby-feature-'+$_+'.json') -Raw|ConvertFrom-Json}
if(($taskProfiles[0].cases|ConvertTo-Json -Compress)-cne($taskProfiles[1].cases|ConvertTo-Json -Compress)){throw 'Lobby version architecture disagreement'}
$taskPairs=@();foreach($taskValue in '2.00.14','2.01.04','2.02.09'){
 $taskGroup=@($taskProfiles[0].cases|Where-Object value -eq $taskValue)
 if($taskGroup.Count-ne5-or@($taskGroup.enabled|Sort-Object -Unique).Count-ne1){throw 'Lobby version culture disagreement'}
 $taskPairs+=[ordered]@{current='3.09.00';value=$taskValue;enabled=$taskGroup[0].enabled}
}
$taskPairs|ConvertTo-Json -Depth 4|Set-Content src/domain/feature_lobby_version_pairs.json -Encoding utf8NoBOM
$taskSources=@('legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore/FeatureMatrix.cs','scripts/reverse/generate-lobby-feature-fixtures.ps1')
[ordered]@{profiles=$taskProfiles;sources=@($taskSources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}});original_services_executed=$false;sqlserver_access=$false}|ConvertTo-Json -Depth 8|Set-Content tests/fixtures/framework-lobby-features.json -Encoding utf8NoBOM
} finally {Pop-Location}
