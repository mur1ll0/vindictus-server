#requires -Version 7.4
# Laboratory class switch for one offline character, by name, for client tests of classes the
# 1.69 client does not create (class 10 = Delia). Prints the previous class; run again with it to
# undo. The query runs inside the postgres container with the application role; no credential is
# printed or passed on the command line.
# The new class's starting skills (DefaultSkillInfo) that the character lacks are added as
# learned skills at rank 1, after its own rows: the selection load requires the first rows to be
# the creating initializer's skills (renaming them was refused, D10). Row ids are UTC ticks, as
# Service.MakeEntityID. Added skills stay when switching back; the old class's window hides them.
# The base costume follows too: inner armor, hair and eyebrows still at the old class's
# csf/create_<class>.csf default take the new class's (a Fiona inner on Delia's body rendered
# white legs at character select, D9).
[CmdletBinding()]
param([Parameter(Mandatory)][string]$Name,[Parameter(Mandatory)][ValidateRange(0,10)][int]$Class)
$ErrorActionPreference='Stop'
$project=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
if($Name-notmatch'^[A-Za-z0-9]{2,16}$'){throw 'Name must be 2-16 ASCII letters or digits'}
$hex=[Convert]::ToHexString([Text.Encoding]::Unicode.GetBytes($Name)).ToLowerInvariant()
# DefaultSkillInfo of the 1.69 content (heroesContents.db3, in ID order). The 1.69 has no row for
# class 10: Delia's are the laboratory choice matching Fiona's (first-weapon mastery, guard).
$defaults=@{
 0=@('skill_lethita_slipdash','skill_combat_mastery','skill_lethita_spear_dash')
 1=@('skill_fiona_shield_mastery','skill_fiona_guard')
 2=@('skill_evy_charge_magic','skill_evy_magic_arrow','skill_evy_magic_firebolt','skill_evy_mana_mine','skill_evy_neo_mana_shield')
 3=@('skill_kalok_special_grappling','skill_combat_mastery')
 4=@('skill_kay_multi_shot','skill_combat_mastery','skill_kay_kick_thrust')
 5=@('skill_vella_wind_step','skill_combat_mastery')
 6=@('skill_combat_mastery')
 7=@('skill_combat_mastery','skill_lynn_speedy_move')
 8=@('skill_arisha_castlet_mastery','skill_arisha_mana_stone')
 9=@('skill_hagie_magic_mastery')
 10=@('skill_delia_bastard_mastery','skill_delia_guard')
}
$rows=foreach($c in $defaults.Keys){for($i=0;$i-lt$defaults[$c].Count;$i++){"($c,$i,decode('$([Convert]::ToHexString([Text.Encoding]::Unicode.GetBytes($defaults[$c][$i])).ToLowerInvariant())','hex'))"}}
# csf/create_<class>.csf defaults: (costume type, key) = inner (0,1), hair (0,3), eyebrows (1,0).
$looks=@{0=@(0,1002,1000);1=@(10,2001,0);2=@(10,3001,3000);3=@(2002,-1,4008);4=@(3007,5001,5001);5=@(4001,6001,6000);6=@(6013,6017,7000);7=@(1002,6036,8000);8=@(1002,6044,9005);9=@(8001,8001,10001);10=@(1002,6036,8000)}
$parts=@(@(0,1),@(0,3),@(1,0))
$looksRows=foreach($c in $looks.Keys){for($i=0;$i-lt3;$i++){"($c,$($parts[$i][0]),$($parts[$i][1]),$($looks[$c][$i]))"}}
# The old class comes from the locked row; a connected character is left untouched.
$query="WITH d(class,pos,skill) AS (VALUES $($rows-join',')), l(class,type,key,sn) AS (VALUES $($looksRows-join',')), t AS (SELECT id,class FROM game_character_info WHERE name_utf16=decode('$hex','hex') AND NOT is_connected AND status=0 AND (SELECT count(*) FROM game_character_info WHERE name_utf16=decode('$hex','hex'))=1 FOR UPDATE), s AS (INSERT INTO game_skills(row_id,cid,skill_utf16,rank,locked,current_ap) SELECT (extract(epoch FROM clock_timestamp())*10000000)::bigint+621355968000000000+n.pos,t.id,n.skill,1,0,0 FROM t, d n WHERE n.class=$Class AND NOT EXISTS(SELECT 1 FROM game_skills x WHERE x.cid=t.id AND x.skill_utf16=n.skill) RETURNING row_id), b AS (UPDATE game_costumes g SET costume_sn=n.sn FROM t, l o, l n WHERE g.cid=t.id AND g.is_base AND o.class=t.class AND n.class=$Class AND o.type=n.type AND o.key=n.key AND g.costume_type=o.type AND g.costume_key=o.key AND g.costume_sn=o.sn AND o.sn<>n.sn RETURNING g.row_id) UPDATE game_character_info c SET class=$Class FROM t WHERE c.id=t.id RETURNING t.id||'|'||t.class||'|'||(SELECT count(*) FROM s)||'|'||(SELECT count(*) FROM b)"
$shell='PGPASSWORD="$APP_DATABASE_PASSWORD" psql --username "$APP_DATABASE_USER" --dbname "$POSTGRES_DB" --no-psqlrc --quiet --tuples-only --no-align --set ON_ERROR_STOP=1 --command "$1"'
Push-Location $project
try{$out=& docker compose exec -T postgres bash -c $shell 'lab-set-class' $query}finally{Pop-Location}
if($LASTEXITCODE-ne0){throw 'Query failed'}
$rows=@($out|Where-Object{$_-match'^\d+\|\d+\|\d+\|\d+$'})
if($rows.Count-ne1){throw "Expected one offline character named $Name, changed $($rows.Count)"}
$cid,$old,$skills,$looks=$rows[0].Split('|')
"cid=$cid class $old -> $Class, $skills starting skill(s) added and $looks base costume part(s) changed (undo: scripts/dev/lab-set-class.ps1 -Name $Name -Class $old)"
