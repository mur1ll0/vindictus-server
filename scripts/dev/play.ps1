#requires -Version 5.1
# Minimal laboratory launcher (trilha L, L4): asks login and password (no echo), gets a one-time
# ticket from the panel listener (POST /api/launcher/login, the same check as the panel login) and
# starts the client copy with -launcher_ticket, so Start Game logs in without the ID/Pass dialog
# (scripts/client/patch_client_launcher_login.py). The password and the ticket are never printed
# or written to disk. Replaced by the real launcher (docs/future/game-launcher.md).
[CmdletBinding()]
param(
    [string]$Username,
    [string]$Server = 'http://127.0.0.1:27090',
    [string]$ClientCopy
)
$ErrorActionPreference = 'Stop'
$project = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
if (-not $ClientCopy) { $ClientCopy = Join-Path $project '.cache/play-client' }
$exe = Join-Path $ClientCopy 'Vindictus.exe'
if (-not (Test-Path -LiteralPath $exe)) { throw "Client not found: $exe" }
if (Get-Process -Name 'Vindictus' -ErrorAction SilentlyContinue) { throw 'The game is already running; close it first.' }
if (-not $Username) { $Username = Read-Host 'Login' }
$secure = Read-Host 'Senha' -AsSecureString
$pointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
try {
    $password = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($pointer)
    $body = @{ username = $Username; password = $password } | ConvertTo-Json -Compress
} finally {
    [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($pointer)
    Remove-Variable password -ErrorAction SilentlyContinue
}
try {
    $reply = Invoke-RestMethod -Method Post -Uri ($Server.TrimEnd('/') + '/api/launcher/login') `
        -ContentType 'application/json' -Body ([Text.Encoding]::UTF8.GetBytes($body))
} catch {
    $status = $_.Exception.Response.StatusCode.value__
    switch ($status) {
        401 { throw 'Login ou senha inválidos.' }
        429 { throw 'Muitas tentativas. Aguarde um minuto.' }
        default { throw "O servidor não emitiu o ticket (HTTP $status)." }
    }
} finally {
    Remove-Variable body -ErrorAction SilentlyContinue
}
$arguments = @('-stage', '-dev', '-lang', 'en-eu-x-gm', '-noupdate', '-threads', '4', '-launcher_ticket', $reply.ticket)
Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory $ClientCopy | Out-Null
Write-Host "Jogo aberto. Clique em Start Game em até $($reply.expires_in) segundos."
