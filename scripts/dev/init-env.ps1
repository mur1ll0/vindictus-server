[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$taskProject = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$taskEnvPath = Join-Path $taskProject '.env'
if (Test-Path -LiteralPath $taskEnvPath) { throw '.env already exists; existing credentials were preserved. Edit deliberately if required.' }
$taskBootstrap = [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(24))
$taskApp = [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(24))
$taskSqlServer = 'Study9!_' + [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(24))
try {
    $taskTemplate = [IO.File]::ReadAllText((Join-Path $taskProject '.env.example'))
    $taskTemplate = $taskTemplate.Replace('REPLACE_WITH_GENERATED_BOOTSTRAP_PASSWORD',$taskBootstrap)
    $taskTemplate = $taskTemplate.Replace('REPLACE_WITH_DIFFERENT_GENERATED_URL_SAFE_PASSWORD',$taskApp)
    $taskTemplate = $taskTemplate.Replace('REPLACE_WITH_GENERATED_STRONG_PASSWORD',$taskSqlServer)
    [IO.File]::WriteAllText($taskEnvPath,$taskTemplate,[Text.UTF8Encoding]::new($false))
    New-Item -ItemType Directory -Force -Path (Join-Path $taskProject '.cache') | Out-Null
    $taskLocalUrl = 'postgres://vindictus_app:' + $taskApp + '@127.0.0.1:5433/vindictus'
    [IO.File]::WriteAllText((Join-Path $taskProject '.cache/database-url'),$taskLocalUrl,[Text.UTF8Encoding]::new($false))
    Write-Host 'Created ignored .env and .cache/database-url with distinct random credentials. No credentials were printed.'
} finally { $taskBootstrap=$null; $taskApp=$null; $taskSqlServer=$null; $taskTemplate=$null; $taskLocalUrl=$null }
