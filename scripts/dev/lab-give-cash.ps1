#requires -Version 7.4
# Laboratory cash credit for the account of the first Fiona (the NISMS charge the cash shop reads).
# The database URL is composed from .env for the child process only and is never printed.
[CmdletBinding()]
param([int]$Amount=1000000,[long]$Cid=0)
$ErrorActionPreference='Stop'
$project=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
if($Amount-lt1){throw 'Amount must be positive'}
$exe=Join-Path $project 'target/debug/vindictus-server.exe'
if(-not(Test-Path -LiteralPath $exe)){throw 'Build the native binary first (cargo build --locked)'}
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
 $arguments=@('lab-give-cash','--amount',$Amount)
 if($Cid-gt0){$arguments+=@('--cid',$Cid)}
 & $exe @arguments
 if($LASTEXITCODE-ne0){throw 'Credit refused; see the message above'}
}finally{$env:DATABASE_URL=$previous}
