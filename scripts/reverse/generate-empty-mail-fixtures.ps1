$ErrorActionPreference='Stop'
. scripts/dev/docker-process.ps1
$root=$PWD.Path
$schema=Get-Content docs/evidence/database/study_heroes.schema.json -Raw|ConvertFrom-Json -DateKind String
$module=@($schema.sql_modules|Where-Object object_id -eq 46623209)[0]
$source=$module.definition
$hash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($source))).ToLowerInvariant()
if($hash-cne'c17336ad0fb0aca9d1cb6320580dd58538227ee48bd7a4292cf6e06cd33f484c'){throw 'Mail oracle source changed'}
$begin=$source.IndexOf('BEGIN')+5;$end=$source.LastIndexOf('END')
$select=$source.Substring($begin,$end-$begin).Trim().Replace('[dbo].[Mail]','synthetic_mail')
$rows=@(@(1,5,6,0),@(2,7,8,1),@(3,9,9,1),@(4,11,11,0),@(5,-1,12,0),@(6,21,13,0),@(7,6,5,0))
$values=($rows|ForEach-Object{'('+($_ -join ',')+')'})-join ','
$cases=@()
foreach($cid in @(-1,0,5,6,7,8,9,10,11,12,13,21)){
 $query="SET NOCOUNT ON;DECLARE @CID bigint=$cid;WITH synthetic_mail AS(SELECT MailID,FromCID,ToCID,CONVERT(bit,IsDeleted)AS IsDeleted,DATEADD(day,MailID,CONVERT(datetime,'2000-01-01'))AS ArriveTime FROM(VALUES $values)v(MailID,FromCID,ToCID,IsDeleted)) "+$select
 $sh='SQLCMDPASSWORD="$MSSQL_SA_PASSWORD" /opt/mssql-tools18/bin/sqlcmd -S localhost -U sa -C -b -d study_heroes -h -1 -W -s "|" -Q "$1"'
 $r=Invoke-ProjectDocker -Arguments @('compose','--profile','legacy','exec','-T','legacy-sqlserver','bash','-c',$sh,'readonly-synthetic-mail-oracle',$query)
 if($r.ExitCode-ne0){throw 'Synthetic original Mail SELECT oracle failed;local diagnostic withheld'}
 $ids=@($r.Stdout -split "`n"|Where-Object{$_-match'^\s*(-?\d+)\|'}|ForEach-Object{[long]($_.Split('|')[0].Trim())}|Sort-Object)
 $cases+=@{cid=[long]$cid;mail_ids=$ids;empty=($ids.Count-eq0)}
}
if((@($cases|Where-Object empty|ForEach-Object cid)-join ',')-cne'0,8,10'){throw 'Synthetic Mail result parsing or source semantics changed'}
$result=[ordered]@{scope='complete original QueryMailList SELECT including TOP300/UNIONALL/ORDER;only dbo.Mail replaced by synthetic CTE;no procedure execution or reference DML';procedure_sha256=$hash;generator_sha256=(Get-FileHash scripts/reverse/generate-empty-mail-fixtures.ps1).Hash.ToLowerInvariant();rows=@($rows|ForEach-Object{@{mail_id=[long]$_[0];from_cid=[long]$_[1];to_cid=[long]$_[2];is_deleted=[bool]$_[3]}});cases=$cases}
$result|ConvertTo-Json -Depth 8|Set-Content tests/fixtures/sqlserver-empty-mail.json -Encoding utf8NoBOM
Write-Output 'Original SQL Mail SELECT oracle:12 synthetic CID cases;7rows;no reference DML/procedure execution.'
