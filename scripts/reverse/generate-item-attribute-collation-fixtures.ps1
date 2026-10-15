$ErrorActionPreference='Stop'
. ./scripts/dev/docker-process.ps1
$taskCodes=@(32..126)+@(200,201,202,203,204,205)
$taskParts=foreach($taskCode in $taskCodes){
 if($taskCode-le126){'('+$taskCode+',CHAR('+$taskCode+'))'}
 else{switch($taskCode){200{"(200,'')"}201{"(201,'A ')"}202{"(202,'a')"}203{"(203,' A')"}204{"(204,' a')"}205{"(205,'  ')"}}}
}
$taskValues=$taskParts -join ','

$taskQuery="SET NOCOUNT ON;WITH samples(code,value) AS (SELECT code,CONVERT(varchar(50),value) COLLATE SQL_Latin1_General_CP1_CI_AS FROM (VALUES $taskValues) v(code,value)) SELECT l.code AS l,r.code AS r FROM samples l CROSS JOIN samples r WHERE l.value=r.value ORDER BY l.code,r.code FOR JSON PATH;"
$taskShell='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
$r=Invoke-ProjectDocker -Arguments @('compose','--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'attribute-collation-readonly',$taskQuery)
if($r.ExitCode-ne0){throw 'Readonly collation oracle failed; details suppressed'}
$taskJson=$r.Stdout.Replace([string][char]13,'').Replace([string][char]10,'').Trim()
$taskPairs=$taskJson|ConvertFrom-Json
$taskKeys=@($taskCodes|ForEach-Object{$v=if($_-le126){[string][char]$_}else{switch($_){200{''}201{'A '}202{'a'}203{' A'}204{' a'}205{'  '}}};[ordered]@{code=$_;hex=[Convert]::ToHexString([Text.Encoding]::ASCII.GetBytes($v)).ToLowerInvariant()}})
[ordered]@{collation='SQL_Latin1_General_CP1_CI_AS';source_index='NCL_ItemAttribute_ItemID_Attribute';source_read_only=$true;keys=$taskKeys;equal_pairs=@($taskPairs);comparisons=$taskKeys.Count*$taskKeys.Count;query_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($taskQuery))).ToLowerInvariant()}|ConvertTo-Json -Depth 8 -Compress|Set-Content tests/fixtures/sql-item-attribute-collation.json -Encoding utf8NoBOM
$taskMeta="SET NOCOUNT ON;SELECT (SELECT COUNT(*) FROM dbo.Item) AS items,(SELECT COUNT(*) FROM dbo.ItemAttribute) AS attributes,(SELECT COUNT(*) FROM dbo.Equippable) AS equipment,(SELECT COUNT(*) FROM dbo.ItemStorageStatus) AS storage FOR JSON PATH,WITHOUT_ARRAY_WRAPPER;"
$r=Invoke-ProjectDocker -Arguments @('compose','--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskShell,'item-state-counts',$taskMeta)
if($r.ExitCode-ne0){throw 'Readonly count query failed'}
Write-Output ('Reference row counts: '+$r.Stdout.Trim())
Write-Output ('Synthetic SQL collation oracle: comparisons='+($taskKeys.Count*$taskKeys.Count)+'; equal pairs='+$taskPairs.Count)
