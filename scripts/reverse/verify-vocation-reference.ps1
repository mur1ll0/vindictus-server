$ErrorActionPreference='Stop'
$taskProject=$PWD.Path
. ./scripts/dev/docker-process.ps1
$taskTokens=$null;$taskErrors=$null
$taskAst=[Management.Automation.Language.Parser]::ParseFile((Join-Path $taskProject 'scripts/database/Import-CharacterVocation.ps1'),[ref]$taskTokens,[ref]$taskErrors)
if($taskErrors.Count){throw 'Owned vocation export syntax invalid'}
$taskFunctions=$taskAst.FindAll({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -in @('Get-TaskHash','Get-TaskExport')},$false)
foreach($taskDefinition in $taskFunctions){. ([scriptblock]::Create($taskDefinition.Extent.Text))}
$taskExport=Get-TaskExport
$taskData=$taskExport.Json|ConvertFrom-Json -DateKind String
if(@($taskData.vocations).Count-ne3-or@($taskData.skills).Count-ne33){throw 'Source profile mismatch'}
$taskQuery=@"
SET NOCOUNT ON;
SELECT CONVERT(varchar(20),DATEDIFF_BIG(DAY,CONVERT(datetime2,'0001-01-01'),CONVERT(date,v))*CONVERT(bigint,864000000000)+DATEDIFF_BIG(NANOSECOND,CONVERT(datetime2,CONVERT(date,v)),v)/100) AS ticks
FROM (VALUES (CAST(NULL AS datetime2)),(CONVERT(datetime2,'0001-01-01T00:00:00.0000000')),(CONVERT(datetime2,'9999-12-31T23:59:59.9999999')),(CONVERT(datetime2,'2001-01-01T00:00:00.0000001'))) t(v)
FOR JSON PATH,INCLUDE_NULL_VALUES;
"@
$taskSh='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
$taskResult=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskSh,'readonly-vocation-tick-probe',$taskQuery)
if($taskResult.ExitCode-ne0){throw 'Read-only synthetic datetime2 probe failed'}
$taskText=$taskResult.Stdout;$a=$taskText.IndexOf('[');$b=$taskText.LastIndexOf(']')
$taskRows=$taskText.Substring($a,$b-$a+1)|ConvertFrom-Json
$taskExpected=@($null,'0','3155378975999999999',([datetime]::Parse('2001-01-01T00:00:00.0000001',[Globalization.CultureInfo]::InvariantCulture)).Ticks.ToString())
if($taskRows.Count-ne4){throw 'Synthetic tick row count mismatch'}
for($taskIndex=0;$taskIndex-lt4;$taskIndex++){if($taskRows[$taskIndex].ticks-ne$taskExpected[$taskIndex]){throw 'Synthetic datetime2 precision mismatch'}}
Write-Output 'Readonly vocation reference PASS;3/33 rows;5/4 fields;identity51;datetime2 synthetic4 exact;no export file or source writes.'
