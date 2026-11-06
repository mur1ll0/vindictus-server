# Instala o nmcogame.dll do laboratório na cópia do cliente (.cache/play-client) guardando o
# original como nmcogame.dll.nexon; -Restore volta o original. Nunca toca no Client/ original.
param([switch]$Restore, [string]$ClientCopy)
$ErrorActionPreference = 'Stop'
$project = Resolve-Path (Join-Path $PSScriptRoot '../..')
if (-not $ClientCopy) { $ClientCopy = Join-Path $project '.cache/play-client' }
$target = Join-Path $ClientCopy 'nmcogame.dll'
$backup = Join-Path $ClientCopy 'nmcogame.dll.nexon'
if ($Restore) {
    if (-not (Test-Path $backup)) { throw 'Sem nmcogame.dll.nexon para restaurar' }
    Copy-Item $backup $target -Force
    Write-Host 'original restaurado'
    exit 0
}
$built = Join-Path $PSScriptRoot 'target/i686-win7-windows-msvc/release/nmcogame.dll'
if (-not (Test-Path $built)) { throw 'Compile antes com build.ps1' }
if (-not (Test-Path $backup)) { Copy-Item $target $backup }
Copy-Item $built $target -Force
Write-Host "instalado em $target (log: nmcogame-lab.log na pasta do jogo)"
