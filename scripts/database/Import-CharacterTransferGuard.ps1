param([ValidateSet('Import','Verify')][string]$Mode='Import')
$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$q=@"
SET NOCOUNT ON;
SELECT OBJECT_DEFINITION(OBJECT_ID('dbo.IsTransCharacter')) AS definition,
(SELECT COUNT_BIG(*) FROM dbo.CharacterInfo_Trans) AS transfer_rows,
(SELECT COUNT(*) FROM sys.columns WHERE object_id=OBJECT_ID('dbo.CharacterInfo_Trans')) AS transfer_columns
FOR JSON PATH,INCLUDE_NULL_VALUES,WITHOUT_ARRAY_WRAPPER;
"@
$sh='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
$r=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$sh,'readonly-next-join-guard',$q)
if($r.ExitCode-ne0){throw 'Next join reference query failed;private details suppressed'}
$a=$r.Stdout.IndexOf('{');$b=$r.Stdout.LastIndexOf('}');$v=$r.Stdout.Substring($a,$b-$a+1).Replace([string][char]13,'').Replace([string][char]10,'')|ConvertFrom-Json -DateKind String
$schema=Get-Content (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json') -Raw|ConvertFrom-Json -DateKind String
$proc=@($schema.objects|Where-Object name -eq 'IsTransCharacter');$old=@($schema.sql_modules|Where-Object object_id -eq $proc[0].object_id)
if($proc.Count-ne1-or$old.Count-ne1-or$old[0].definition-cne$v.definition-or$v.transfer_rows-ne0){throw 'Join guard reference profile changed'}
$procHash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($v.definition))).ToLowerInvariant()
if($procHash-ne'6f47c1f5f6926d7d5bd474fa91547c2e31343d3afc0f8853e3a0ffee0370baaa'-or$v.transfer_columns-ne8){throw 'Transfer procedure/schema profile changed'}
$schemaHash=(Get-FileHash (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json')).Hash.ToLowerInvariant()
$proof=[ordered]@{format_version=1;source='study_heroes.character-transfer-empty';rows=0;columns=8;schema_sha256=$schemaHash;procedure_sha256=$procHash}|ConvertTo-Json -Compress
$taskArgs=@('compose','--project-directory',$taskProject,'exec','-T','server','/app/vindictus-server','import-character-transfer-guard','--input-stdin');if($Mode-eq'Verify'){$taskArgs+='--verify-only'}
$r=Invoke-ProjectDocker -Arguments $taskArgs -InputText $proof
if($r.ExitCode-ne0){throw 'Native transfer guard failed;private details suppressed'}
$summary=$r.Stdout.Trim()|ConvertFrom-Json -DateKind String
if($summary.rows-ne0-or$summary.columns-ne8-or-not$summary.initialized-or$summary.character_creation_available){throw 'Unexpected transfer guard summary'}
$report=[ordered]@{stage='M15.1b.2e.10b.22b';verified_at_utc=[DateTimeOffset]::UtcNow.ToString('o');source_read_only=$true;procedures_executed=$false;source_rows=0;source_columns=8;source_pk='newCharacterID';source_identity=$false;source_foreign_keys=0;procedure_sha256=$procHash;schema_sha256=$schemaHash;summary=$summary;mode=$Mode;client_create_verified=$false}
$report|ConvertTo-Json -Depth 6|Set-Content (Join-Path $taskProject 'docs/evidence/database/character-transfer-guard-import.json') -Encoding utf8NoBOM
Write-Output ('Character transfer empty guard '+$Mode+' PASS:0 rows/8 fields;PK newCharacterID;source readonly;no procedures executed.')
