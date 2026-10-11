$ErrorActionPreference='Stop'
$taskProject=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
# Read-only parser: the hive is never loaded into the Windows registry.
& py -3.13 (Join-Path $taskProject 'scripts/reverse/read_offline_clock.py')
if($LASTEXITCODE-ne0){throw 'Offline time zone parser failed'}
