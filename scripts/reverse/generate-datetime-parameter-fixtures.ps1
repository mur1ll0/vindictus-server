$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
Push-Location $taskProject
try {
 New-Item -ItemType Directory -Force -Path .cache/framework,.cache/verification|Out-Null
 $taskSettings=@{};foreach($taskLine in [IO.File]::ReadAllLines((Join-Path $PWD '.env'))){if($taskLine -match '^([A-Z_]+)=(.*)$'){$taskSettings[$Matches[1]]=$Matches[2]}}
 $taskPassword=if($taskSettings.ContainsKey('MSSQL_SA_PASSWORD')){$taskSettings.MSSQL_SA_PASSWORD}else{$taskSettings.SQLSERVER_SA_PASSWORD}
 if(-not $taskPassword){throw 'Reference SQL password unavailable'}
 $taskBuilder=[System.Data.SqlClient.SqlConnectionStringBuilder]::new();$taskBuilder['Data Source']='127.0.0.1,14333';$taskBuilder['Initial Catalog']='study_heroes';$taskBuilder['User ID']='sa';$taskBuilder['Password']=$taskPassword;$taskBuilder['TrustServerCertificate']=$true;$taskBuilder['Encrypt']=$false;$taskBuilder['Connect Timeout']=10
 $env:DATETIME_ORACLE_SQLSERVER=$taskBuilder.ConnectionString
 $taskHarness='scripts/reverse/framework/SqlDateTimeParameterOracle.cs';$taskProfiles=@()
 foreach($taskArch in 'x86','x64') {
  $taskExe='.cache/framework/DateTimeParameter-'+$taskArch+'.exe'
  $taskArgs=@('C:\Program Files\dotnet\sdk\9.0.301\Roslyn\bincore\csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$taskArch),('/out:'+$taskExe))
  foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions','System.Data'){$taskArgs+=('/r:C:\Program Files (x86)\Reference Assemblies\Microsoft\Framework\.NETFramework\v4.8\'+$taskLib+'.dll')}
  $taskArgs+=$taskHarness
  & 'C:\Program Files\dotnet\dotnet.exe' @taskArgs *> .cache/verification/datetime-parameter-build.log
  if($LASTEXITCODE -ne 0){Get-Content .cache/verification/datetime-parameter-build.log -Tail 6;throw 'Oracle build failed'}
  $taskRaw=& (Join-Path $PWD $taskExe) 2> .cache/verification/datetime-parameter-error.log
  if($LASTEXITCODE -ne 0){throw 'DateTime parameter oracle failed; see private diagnostic log'}
  $taskData=$taskRaw|ConvertFrom-Json -DateKind String
  if($taskData.architecture -ne $taskArch){throw 'Wrong oracle architecture'}
  $taskProfiles+=$taskData
 }
 if(($taskProfiles[0].cases|ConvertTo-Json -Compress) -ne ($taskProfiles[1].cases|ConvertTo-Json -Compress)){throw 'Architecture disagreement'}
 $taskFixture=[ordered]@{profiles=$taskProfiles;source=[ordered]@{path=$taskHarness;sha256=(Get-FileHash $taskHarness).Hash.ToLowerInvariant()};sqlserver_persistent_writes=$false;source_services_executed=$false;query='SELECT CONVERT(binary(8),@p); synthetic SqlDbType.DateTime parameters only'}
 [IO.File]::WriteAllText((Join-Path $PWD 'tests/fixtures/framework-datetime-parameters.json'),(($taskFixture|ConvertTo-Json -Depth 8).Replace("`r`n","`n")+"`n"),[Text.UTF8Encoding]::new($false))
 Write-Output ('DateTime oracle PASS; profiles=2; cases/profile='+$taskProfiles[0].cases.Count+'; persistent_writes=0')
} finally {Remove-Item Env:DATETIME_ORACLE_SQLSERVER -ErrorAction SilentlyContinue;Pop-Location}
