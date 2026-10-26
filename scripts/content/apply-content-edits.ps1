#requires -Version 7.4
# Apply the published admin panel content edits (B4.4) on the host:
#  1. export the cumulative overlay from PostgreSQL (.cache/private-content/content-edits.json);
#  2. re-run the content exporters that read it (npc-scripts.json, cash-shop.json);
#  3. rebuild ShopInfo of the client copy (.cache/play-client; close the client first);
#  4. rewrite the cash shop ShopList.aspx;
#  5. restart the game container (players are disconnected) and wait until it is healthy;
#  6. record the version as applied.
# -DryRun stops after step 1 and shows what the client copy would change. The database URL is
# composed from .env for the child processes only and is never printed.
[CmdletBinding()]
param([switch]$DryRun,[string]$ClientCopy)
$ErrorActionPreference='Stop'
$project=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
if(-not$ClientCopy){$ClientCopy=Join-Path $project '.cache/play-client'}
$exe=Join-Path $project 'target/debug/vindictus-server.exe'
if(-not(Test-Path -LiteralPath $exe)){throw 'Build the native binary first (cargo build --locked)'}
if(@(Get-Process -Name 'vindictus' -ErrorAction SilentlyContinue).Count-gt0 -and -not$DryRun){throw 'Close the game client before changing its copy'}
$edits=Join-Path $project '.cache/private-content/content-edits.json'
$settings=@{}
foreach($line in [IO.File]::ReadAllLines((Join-Path $project '.env'))){if($line-match'^(APP_DATABASE_USER|APP_DATABASE_PASSWORD|POSTGRES_DB|POSTGRES_HOST_PORT)=(.*)$'){$settings[$Matches[1]]=$Matches[2]}}
if([string]::IsNullOrEmpty($settings.APP_DATABASE_PASSWORD)){throw 'Local application role credential is required'}
$user=if($settings.APP_DATABASE_USER){$settings.APP_DATABASE_USER}else{'vindictus_app'}
$database=if($settings.POSTGRES_DB){$settings.POSTGRES_DB}else{'vindictus'}
$port=if($settings.POSTGRES_HOST_PORT){[int]$settings.POSTGRES_HOST_PORT}else{5433}
$previous=$env:DATABASE_URL
try{
 $env:DATABASE_URL='postgres://'+[Uri]::EscapeDataString($user)+':'+[Uri]::EscapeDataString($settings.APP_DATABASE_PASSWORD)+'@127.0.0.1:'+$port+'/'+[Uri]::EscapeDataString($database)
 $settings.Clear()
 $exported=& $exe content-edits-export --output $edits
 if($LASTEXITCODE-ne0){throw 'Content edits export failed'}
 $version=($exported|ConvertFrom-Json).version
 Write-Host ('Overlay exported: version '+$version)
 if($DryRun){
  & py -3.13 (Join-Path $project 'scripts/client/apply_lab_content_additions.py') --client-copy $ClientCopy --edits $edits
  return
 }
 foreach($export in @('export_npc_scripts.py|npc-scripts.json','export_cash_shop.py|cash-shop.json')){
  $script,$file=$export.Split('|')
  & py -3.13 (Join-Path $project ('scripts/content/'+$script)) --output (Join-Path $project ('.cache/private-content/'+$file))
  if($LASTEXITCODE-ne0){throw ($script+' failed')}
 }
 & py -3.13 (Join-Path $project 'scripts/client/apply_lab_content_additions.py') --client-copy $ClientCopy --apply --edits $edits
 if($LASTEXITCODE-ne0){throw 'Client copy update failed'}
 & pwsh -NoProfile -File (Join-Path $project 'scripts/dev/lab-export-shop-list.ps1')
 if($LASTEXITCODE-ne0){throw 'ShopList.aspx export failed'}
 Push-Location $project
 try{
  & docker compose restart game | Out-Null
  & docker compose up -d --wait game | Out-Null
  if($LASTEXITCODE-ne0){throw 'The game container did not become healthy; see docker compose logs game'}
 }finally{Pop-Location}
 & $exe content-edits-applied --version $version
 if($LASTEXITCODE-ne0){throw 'Recording the applied version failed'}
 Write-Host ('Content edits applied up to version '+$version+'; open the client again.')
}finally{$env:DATABASE_URL=$previous}
