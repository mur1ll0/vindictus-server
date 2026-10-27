#requires -Version 7.4
# Laboratory class switch for one offline character, by name, for client tests of classes the
# 1.69 client does not create (class 10 = Delia). Prints the previous class; run again with it to
# undo. The query runs inside the postgres container with the application role; no credential is
# printed or passed on the command line.
[CmdletBinding()]
param([Parameter(Mandatory)][string]$Name,[Parameter(Mandatory)][ValidateRange(0,10)][int]$Class)
$ErrorActionPreference='Stop'
$project=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
if($Name-notmatch'^[A-Za-z0-9]{2,16}$'){throw 'Name must be 2-16 ASCII letters or digits'}
$hex=[Convert]::ToHexString([Text.Encoding]::Unicode.GetBytes($Name)).ToLowerInvariant()
# The old class comes from the locked row; a connected character is left untouched.
$query="WITH t AS (SELECT id,class FROM game_character_info WHERE name_utf16=decode('$hex','hex') AND NOT is_connected AND status=0 AND (SELECT count(*) FROM game_character_info WHERE name_utf16=decode('$hex','hex'))=1 FOR UPDATE) UPDATE game_character_info c SET class=$Class FROM t WHERE c.id=t.id RETURNING t.id||'|'||t.class"
$shell='PGPASSWORD="$APP_DATABASE_PASSWORD" psql --username "$APP_DATABASE_USER" --dbname "$POSTGRES_DB" --no-psqlrc --quiet --tuples-only --no-align --set ON_ERROR_STOP=1 --command "$1"'
Push-Location $project
try{$out=& docker compose exec -T postgres bash -c $shell 'lab-set-class' $query}finally{Pop-Location}
if($LASTEXITCODE-ne0){throw 'Query failed'}
$rows=@($out|Where-Object{$_-match'^\d+\|\d+$'})
if($rows.Count-ne1){throw "Expected one offline character named $Name, changed $($rows.Count)"}
$cid,$old=$rows[0].Split('|')
"cid=$cid class $old -> $Class (undo: scripts/dev/lab-set-class.ps1 -Name $Name -Class $old)"
