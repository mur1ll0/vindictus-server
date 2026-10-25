#requires -Version 7.4
# Laboratory ShopList.aspx for the in-game cash shop (InGameCashShopDialog_V2), written to the
# bootstrap HTTP root that serves ingame_cashshop_shopListURL. Reads the private assets only.
[CmdletBinding()]
param([string]$AssetsFile,[int]$PerCategory=10)
$ErrorActionPreference='Stop'
$project=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
if(-not$AssetsFile){$AssetsFile=Join-Path $project '.cache/private-content/minimum-creation-assets.json'}
$exe=Join-Path $project 'target/debug/vindictus-server.exe'
if(-not(Test-Path -LiteralPath $exe)){throw 'Build the native binary first (cargo build --locked)'}
$output=Join-Path $project '.cache/client-bootstrap/en-eu-x-gm/ShopList.aspx'
if($PerCategory-lt1){throw 'PerCategory must be positive'}
& $exe lab-export-shop-list --assets-file $AssetsFile --output $output --per-category $PerCategory
if($LASTEXITCODE-ne0){throw 'ShopList refused; see the message above'}
