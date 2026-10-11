[CmdletBinding()]
param()
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskPairs=@(
    @('Estudo','estudo'), @('Estudo','Estudo '), @('Estudo','Estudo  '),
    @('',' '), @('A ','a'), @('I','i'), @('a-b','ab'), @("a'b",'ab'),
    @(' a','a'), @('a b','ab'), @(('a'+[char]9),'a'), @(('a'+[char]9+'b'),'ab'),
    @('É','é'), @('é','e'), @('ß','ss'), @('I','ı'), @('İ','i'),
    @('Ａ','a'), @('Ａ','A'), @('é',("e"+[char]0x301)), @('Z9_','z9_')
)
$taskValues=[Collections.Generic.List[string]]::new()
$taskId=0
foreach($taskPair in $taskPairs){
    $taskLeft=$taskPair[0].Replace("'","''")
    $taskRight=$taskPair[1].Replace("'","''")
    $taskValues.Add("("+$taskId+",N'"+$taskLeft+"',N'"+$taskRight+"')")
    $taskId++
}
$taskSql=@"
SET NOCOUNT ON;
SELECT
    (SELECT collation_name FROM sys.columns WHERE object_id=OBJECT_ID('dbo.[User]') AND name='Name') AS observed_collation,
    JSON_QUERY((SELECT v.case_id, v.left_value, v.right_value,
        CAST(CASE WHEN v.left_value COLLATE SQL_Latin1_General_CP1_CI_AS = v.right_value COLLATE SQL_Latin1_General_CP1_CI_AS THEN 1 ELSE 0 END AS bit) AS equal
        FROM (VALUES {VALUES}) v(case_id,left_value,right_value)
        ORDER BY v.case_id FOR JSON PATH)) AS cases,
    (SELECT COUNT_BIG(*) FROM dbo.[User]) AS reference_user_count,
    (SELECT COUNT_BIG(*) FROM dbo.[User] WHERE Name COLLATE Latin1_General_BIN2 LIKE N'%[^ -~]%') AS reference_non_printable_ascii_name_count
FOR JSON PATH, WITHOUT_ARRAY_WRAPPER;
"@
$taskSql=$taskSql.Replace('{VALUES}',($taskValues -join ','))
$taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
$taskResult=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'oracle',$taskSql)
if($taskResult.ExitCode -ne 0){throw ('Read-only collation oracle failed: '+$taskResult.Stderr)}
$taskJsonStart=$taskResult.Stdout.IndexOf('{')
$taskJsonEnd=$taskResult.Stdout.LastIndexOf('}')
if($taskJsonStart -lt 0 -or $taskJsonEnd -lt $taskJsonStart){throw 'Missing JSON from read-only collation oracle.'}
$taskData=$taskResult.Stdout.Substring($taskJsonStart,$taskJsonEnd-$taskJsonStart+1) | ConvertFrom-Json
if($taskData.observed_collation -ne 'SQL_Latin1_General_CP1_CI_AS'){throw 'Unexpected User.Name collation; do not reuse the synthetic profile.'}
$taskMismatches=0
foreach($taskCase in $taskData.cases){
    $taskAscii=$true
    foreach($taskUnit in ($taskCase.left_value+$taskCase.right_value).ToCharArray()){
        if([int]$taskUnit -lt 32 -or [int]$taskUnit -gt 126){$taskAscii=$false;break}
    }
    $taskExpected=$null
    if($taskAscii){
        $taskExpected=$taskCase.left_value.ToLowerInvariant().TrimEnd(' ') -ceq $taskCase.right_value.ToLowerInvariant().TrimEnd(' ')
        if($taskExpected -ne $taskCase.equal){$taskMismatches++}
    }
    $taskCase | Add-Member -NotePropertyName printable_ascii_contract -NotePropertyValue $taskAscii
    $taskCase | Add-Member -NotePropertyName ascii_key_equality -NotePropertyValue $taskExpected
}
$taskData | Add-Member -NotePropertyName method -NotePropertyValue 'SELECT-only synthetic SQL Server CI_AS comparison + aggregate counts; no account values exported or SQL writes'
$taskData | Add-Member -NotePropertyName verified_at_utc -NotePropertyValue ([DateTimeOffset]::UtcNow.ToString('o'))
$taskData | Add-Member -NotePropertyName ascii_key_mismatches -NotePropertyValue $taskMismatches
$taskData | Add-Member -NotePropertyName query_sha256 -NotePropertyValue ([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($taskSql))).ToLowerInvariant())
$taskData | Add-Member -NotePropertyName source_schema_sha256 -NotePropertyValue (Get-FileHash -LiteralPath (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json') -Algorithm SHA256).Hash.ToLowerInvariant()
$taskData | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $taskProject 'docs/evidence/database/collation-oracle.json') -Encoding utf8
Write-Host ('Collation oracle: cases='+$taskData.cases.Count+'; ASCII key mismatches='+$taskMismatches+'; existing non-ASCII/control names='+$taskData.reference_non_printable_ascii_name_count)
if($taskMismatches -ne 0){throw 'The proposed restricted ASCII comparison key differs from the SQL reference.'}