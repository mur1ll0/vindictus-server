# Read-only preparation of the finite Murael naming proof. Private terms/names stay in .cache.
$ErrorActionPreference='Stop'
& py -3.13 -X utf8 scripts/reverse/read-character-name-profile.py
if($LASTEXITCODE-ne0){throw 'Original naming profile recovery failed'}
& py -3.13 -X utf8 scripts/reverse/prepare-forbidden-candidate.py
if($LASTEXITCODE-ne0){throw 'Private forbidden candidate preparation failed'}
$ErrorActionPreference='Stop'
foreach($taskArch in 'x86','x64'){
 $taskArgs=@('C:/Program Files/dotnet/sdk/9.0.301/Roslyn/bincore/csc.dll','/nologo','/noconfig','/nostdlib+','/langversion:latest',('/platform:'+$taskArch),('/out:.cache/framework/ForbiddenCandidate-'+$taskArch+'.exe'))
 foreach($taskLib in 'mscorlib','System','System.Core','System.Web.Extensions'){$taskArgs+=('/r:C:/Program Files (x86)/Reference Assemblies/Microsoft/Framework/.NETFramework/v4.8/'+$taskLib+'.dll')}
 $taskArgs+='.cache/framework/ForbiddenCandidateOracle.cs'
 & 'C:/Program Files/dotnet/dotnet.exe' @taskArgs *> ('.cache/verification/forbidden-candidate-build-'+$taskArch+'.log')
 if($LASTEXITCODE-ne0){throw 'Forbidden candidate compilation failed; values suppressed'}
 $taskStart=[Diagnostics.ProcessStartInfo]::new((Join-Path $PWD ('.cache/framework/ForbiddenCandidate-'+$taskArch+'.exe')))
 $taskStart.UseShellExecute=$false;$taskStart.CreateNoWindow=$true;$taskStart.RedirectStandardInput=$true;$taskStart.RedirectStandardOutput=$true;$taskStart.RedirectStandardError=$true
 $taskProcess=[Diagnostics.Process]::Start($taskStart)
 $taskOut=$taskProcess.StandardOutput.ReadToEndAsync();$taskErr=$taskProcess.StandardError.ReadToEndAsync()
 $taskProcess.StandardInput.Write([IO.File]::ReadAllText((Join-Path $PWD '.cache/private-content/name-proof-input.json')));$taskProcess.StandardInput.Close();$taskProcess.WaitForExit()
 [IO.File]::WriteAllText((Join-Path $PWD ('.cache/verification/forbidden-candidate-'+$taskArch+'.json')),$taskOut.GetAwaiter().GetResult(),[Text.UTF8Encoding]::new($false))
 [IO.File]::WriteAllText((Join-Path $PWD ('.cache/verification/forbidden-candidate-'+$taskArch+'.stderr.log')),$taskErr.GetAwaiter().GetResult(),[Text.UTF8Encoding]::new($false))
 if($taskProcess.ExitCode-ne0){throw 'Forbidden candidate execution failed; values suppressed'}
 $taskProcess.Dispose()
 $taskObserved=Get-Content ('.cache/verification/forbidden-candidate-'+$taskArch+'.json') -Raw|ConvertFrom-Json
 Write-Output ('Candidate forbidden measurement:'+ $taskArch+'; cultures='+$taskObserved.cultures+'; filtered='+$taskObserved.filtered_rows+'; forbidden='+$taskObserved.forbidden)
}
$ErrorActionPreference='Stop'
. ./scripts/dev/docker-process.ps1
$taskSql=@"
SET NOCOUNT ON;
WITH values_to_compare AS(
 SELECT N'CharacterInfo' AS source_table,Name AS value FROM dbo.CharacterInfo WHERE Name IS NOT NULL
 UNION ALL SELECT N'ReservedName',CharacterName FROM dbo.ReservedName
 UNION ALL SELECT N'CharacterNameLog',oldCharacterName FROM dbo.CharacterNameLog)
SELECT source_table,CONVERT(varchar(max),CONVERT(varbinary(max),value),2) AS utf16_hex,CONVERT(bit,CASE WHEN value COLLATE SQL_Latin1_General_CP1_CI_AS=N'Murael' COLLATE SQL_Latin1_General_CP1_CI_AS THEN 1 ELSE 0 END) AS equal FROM values_to_compare FOR JSON PATH;
"@
$taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
$taskResult=Invoke-ProjectDocker -Arguments @('compose','--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'candidate-name-readonly-equality',$taskSql)
if($taskResult.ExitCode-ne0){throw 'Readonly candidate equality measurement failed; values suppressed'}
$taskFirst=$taskResult.Stdout.IndexOf('[');$taskLast=$taskResult.Stdout.LastIndexOf(']')
if($taskFirst-lt0-or$taskLast-lt$taskFirst){throw 'Private name comparison JSON missing'}
$taskRows=$taskResult.Stdout.Substring($taskFirst,$taskLast-$taskFirst+1).Replace([string][char]13,'').Replace([string][char]10,'')|ConvertFrom-Json
if($taskRows.Count-gt1000){throw 'Measured candidate reference profile exceeds bound'}
$taskForbidden=@(Get-Content .cache/verification/forbidden-candidate-x86.json -Raw|ConvertFrom-Json;Get-Content .cache/verification/forbidden-candidate-x64.json -Raw|ConvertFrom-Json)
if($taskForbidden[0].candidate-cne'Murael'-or$taskForbidden[1].candidate-cne'Murael'-or$taskForbidden[0].forbidden-ne$taskForbidden[1].forbidden-or$taskForbidden[0].filtered_rows-ne768-or$taskForbidden[1].filtered_rows-ne768-or$taskForbidden[0].cultures-ne859-or$taskForbidden[1].cultures-ne859){throw 'Candidate forbidden reference profile differs'}
& ./scripts/reverse/generate-name-candidate-equality.ps1
$taskSynthetic=Get-Content tests/fixtures/sql-name-candidate-equality.json -Raw|ConvertFrom-Json
$taskMeasuredReferences=@($taskRows)+@($taskSynthetic.cases|ForEach-Object{[ordered]@{source_table='MeasuredSynthetic';utf16_hex=$_.utf16_hex;equal=$_.equal}})
$taskNaming=Get-Content .cache/verification/character-name-profile.json -Raw|ConvertFrom-Json
[ordered]@{format_version=1;candidate='Murael';content_database_sha256=$taskNaming.manifest.database_sha256;original_config_sha256=$taskNaming.manifest.config_sha256;language_tag='en-EU';naming_rule=$taskNaming.features.NamingRule;max_bytes=20;repeating=-1;protect_period=30;japanese=$false;forbidden=$taskForbidden[0].forbidden;forbidden_filtered_rows=768;forbidden_cultures=859;sql_collation='SQL_Latin1_General_CP1_CI_AS';references=$taskMeasuredReferences}|ConvertTo-Json -Depth 7 -Compress|Set-Content .cache/private-content/character-name-proof.json -Encoding utf8NoBOM
[ordered]@{verified_at_utc=[DateTimeOffset]::UtcNow.ToString('o');source_read_only=$true;synthetic_candidate='Murael';measured_reference_values=$taskRows.Count;matching_reference_values=@($taskRows|Where-Object equal).Count;forbidden=$taskForbidden[0].forbidden;forbidden_filtered_rows=768;forbidden_cultures=859;architectures=2;profile_private=$true;unsupported_values_refused=$true;runtime_handler_implemented=$false}|ConvertTo-Json|Set-Content .cache/verification/character-name-proof-summary.json -Encoding utf8NoBOM
Write-Output ('Candidate equality SELECT:reference values='+$taskRows.Count+'; matches='+@($taskRows|Where-Object equal).Count+'; no private names published.')

$taskSources=@('legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/CharacterContents.cs','legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/CharacterDataContext.cs','legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore.Functions/CharacterMaker.cs','legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore.Processor/CheckCharacterNameProcessor.cs','legacy/decompiled/FrontendServiceCore-6be475728091/FrontendServiceCore.States/CharacterList.cs','scripts/reverse/framework/ForbiddenCandidateOracle.cs.in','scripts/reverse/prepare-forbidden-candidate.py','scripts/reverse/prepare-character-name-proof.ps1')
$taskSummary=Get-Content .cache/verification/character-name-proof-summary.json -Raw|ConvertFrom-Json
$taskSummary|Add-Member -NotePropertyName sources -NotePropertyValue @($taskSources|ForEach-Object{[ordered]@{path=$_;sha256=(Get-FileHash $_).Hash.ToLowerInvariant()}})
$taskSummary|Add-Member -NotePropertyName manifest -NotePropertyValue $taskNaming.manifest
$taskSummary|ConvertTo-Json -Depth 8|Set-Content .cache/verification/character-name-proof-summary.json -Encoding utf8NoBOM
