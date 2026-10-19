$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
. (Join-Path $taskProject 'scripts/dev/docker-process.ps1')
$taskQuery=@"
SET NOCOUNT ON;
SELECT JSON_QUERY((SELECT p.object_id,p.name,OBJECT_DEFINITION(p.object_id) AS definition FROM sys.procedures p WHERE p.name IN('UserCaptchaGet','UserCaptchaSet','UserCaptchaCheck') ORDER BY p.name FOR JSON PATH)) AS procedures,
JSON_QUERY((SELECT name,column_id,TYPE_NAME(user_type_id) AS declared_type,is_nullable,is_identity FROM sys.columns WHERE object_id=OBJECT_ID('dbo.UserCaptcha') ORDER BY column_id FOR JSON PATH)) AS columns,
JSON_QUERY((SELECT CONVERT(varchar(20),CONVERT(bigint,CONVERT(int,SUBSTRING(CONVERT(binary(8),v),1,4)))*25920000+CONVERT(int,SUBSTRING(CONVERT(binary(8),v),5,4))) AS units300 FROM (VALUES (CAST(NULL AS datetime)),(CONVERT(datetime,'17530101')),(CONVERT(datetime,'9999-12-31T23:59:59.997',126)),(CONVERT(datetime,'1899-12-31T23:59:59.997',126)),(CONVERT(datetime,'19000101')),(CONVERT(datetime,'1900-01-01T00:00:00.003',126)),(CONVERT(datetime,'1900-01-01T00:00:00.007',126))) t(v) FOR JSON PATH,INCLUDE_NULL_VALUES)) AS probes,
(SELECT COUNT_BIG(*) FROM dbo.UserCaptcha) AS rows,
(SELECT COUNT(*) FROM sys.foreign_keys WHERE parent_object_id=OBJECT_ID('dbo.UserCaptcha')) AS foreign_keys,
(SELECT COUNT(*) FROM sys.indexes i JOIN sys.index_columns ic ON ic.object_id=i.object_id AND ic.index_id=i.index_id WHERE i.object_id=OBJECT_ID('dbo.UserCaptcha') AND i.is_primary_key=1 AND COL_NAME(ic.object_id,ic.column_id)='UID' AND ic.key_ordinal=1) AS uid_primary_key
FOR JSON PATH,WITHOUT_ARRAY_WRAPPER;
"@
$taskSh='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -w 65535 -y 0 -Q "$1"'
$r=Invoke-ProjectDocker -Arguments @('compose','--project-directory',$taskProject,'--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$taskSh,'readonly-captcha-contract',$taskQuery)
if($r.ExitCode-ne0){throw 'Read-only captcha metadata query failed;private details suppressed'}
$a=$r.Stdout.IndexOf('{');$b=$r.Stdout.LastIndexOf('}');if($a-lt0-or$b-lt$a){throw 'Missing captcha metadata'}
$data=$r.Stdout.Substring($a,$b-$a+1).Replace([string][char]13,'').Replace([string][char]10,'')|ConvertFrom-Json -DateKind String
$schema=Get-Content (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json') -Raw|ConvertFrom-Json -DateKind String
if($data.rows-ne24-or$data.foreign_keys-ne0-or$data.uid_primary_key-ne1-or$data.columns.Count-ne6-or$data.procedures.Count-ne3){throw 'Captcha source profile changed'}
$taskUnits=@($null,'-1391644800000','76683386879999','-1','0','1','2')
if($data.probes.Count-ne7){throw 'Synthetic SQL datetime count mismatch'}
for($i=0;$i-lt7;$i++){if($data.probes[$i].units300-cne$taskUnits[$i]){throw 'Synthetic SQL datetime precision mismatch'}}
$expected=@($schema.columns|Where-Object object_id -eq 850102069|Sort-Object column_id)
for($i=0;$i-lt6;$i++){foreach($key in @('name','column_id','declared_type','is_nullable','is_identity')){if($expected[$i].$key-ne$data.columns[$i].$key){throw 'Captcha source columns changed'}}}
$hashes=@()
foreach($p in $data.procedures){
 $original=@($schema.sql_modules|Where-Object object_id -eq $p.object_id)
 if($original.Count-ne1-or$original[0].definition-cne$p.definition){throw 'Captcha procedure changed since schema recovery'}
 $hashes+=@{name=$p.name;sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($p.definition))).ToLowerInvariant()}
 if($p.name-eq'UserCaptchaGet'-and$p.definition-notmatch'(?is)SELECT\s+\*\s+FROM\s+dbo.UserCaptcha\s+WITH\(NOLOCK\)\s+WHERE\s+\[UID\]\s*=\s*@uid'){throw 'UserCaptchaGet contract changed'}
}
$taskCsharp=@()
foreach($name in @('CaptchaManager','UserCaptchaGetResult','CharacterDataContext')){
 $path='legacy/decompiled/PlayerService-f23d8b9df77c/PlayerService.CharacterServiceCore/'+$name+'.cs'
 $taskCsharp+=@{path=$path;sha256=(Get-FileHash (Join-Path $taskProject $path)).Hash.ToLowerInvariant()}
}
$report=[ordered]@{stage='M15.1b.2e.10b.21a';measured_at_utc=[DateTimeOffset]::UtcNow.ToString('o');source_read_only=$true;procedures_executed=$false;source_rows=24;source_columns=6;datetime_synthetic_cases=7;datetime_binary_units_exact=$true;uid_primary_key=$true;identity=$false;foreign_keys=0;procedures=$hashes;csharp=$taskCsharp;schema_sha256=(Get-FileHash (Join-Path $taskProject 'docs/evidence/database/study_heroes.schema.json')).Hash.ToLowerInvariant();get_contract='SELECT * FROM dbo.UserCaptcha WITH(NOLOCK) WHERE UID=@uid;0 or 1 row';native_absence_guard='stable transaction/table SHARE lock;stronger than source dirty-read isolation';constructor_available=$false;character_creation_available=$false}
$report|ConvertTo-Json -Depth 6|Set-Content (Join-Path $taskProject 'docs/evidence/auth/user-captcha-source.json') -Encoding utf8NoBOM
Write-Output 'Readonly UserCaptcha contract PASS:24 rows;6 columns;UID PK;no identity/FK;3 procedure hashes unchanged;no procedure executed.'
