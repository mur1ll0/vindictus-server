#requires -Version 5.1
# Minimal laboratory launcher (trilha L, L4). Asks login and password once (no echo) and opens a
# launcher session on the panel listener (POST /api/launcher/login, the same check as the panel
# login). Each time the player chooses to play, it gets a one-time ticket for the session
# (POST /api/launcher/ticket) and starts the client copy with -launcher_ticket, so Start Game logs
# in without the ID/Pass dialog (scripts/client/patch_client_launcher_login.py). The ticket stays
# valid while this launcher is open; leaving (S or Ctrl+C) logs out and revokes unused tickets.
# Closing the window without leaving keeps the session until its 12-hour limit. The password, the
# session and the ticket are never printed or written to disk. Replaced by the real launcher
# (docs/future/game-launcher.md).
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
$api = $Server.TrimEnd('/') + '/api/launcher'

function Invoke-Launcher([string]$Path, [hashtable]$Headers, [byte[]]$Body) {
    try {
        if ($Body) {
            return Invoke-RestMethod -Method Post -Uri "$api/$Path" -Headers $Headers -ContentType 'application/json' -Body $Body
        }
        return Invoke-RestMethod -Method Post -Uri "$api/$Path" -Headers $Headers
    } catch {
        $status = $_.Exception.Response.StatusCode.value__
        switch ($status) {
            401 { if ($Path -eq 'login') { throw 'Login ou senha inválidos.' } else { throw 'Sessão do launcher encerrada. Entre de novo.' } }
            429 { throw 'Muitas tentativas. Aguarde um minuto.' }
            default { throw "O servidor recusou o pedido (HTTP $status)." }
        }
    }
}

if (-not $Username) { $Username = Read-Host 'Login' }
$secure = Read-Host 'Senha' -AsSecureString
$pointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
try {
    $password = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($pointer)
    $body = [Text.Encoding]::UTF8.GetBytes((@{ username = $Username; password = $password } | ConvertTo-Json -Compress))
} finally {
    [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($pointer)
    Remove-Variable password -ErrorAction SilentlyContinue
}
$login = Invoke-Launcher 'login' @{} $body
Remove-Variable body -ErrorAction SilentlyContinue
$auth = @{ Authorization = 'Bearer ' + $login.session }
Write-Host "Conectado como $Username."
try {
    while ($true) {
        $choice = Read-Host 'Enter para abrir o jogo, S para sair'
        if ($choice -match '^[sS]') { break }
        if (Get-Process -Name 'Vindictus' -ErrorAction SilentlyContinue) {
            Write-Host 'O jogo já está aberto; feche-o antes.'
            continue
        }
        $ticket = (Invoke-Launcher 'ticket' $auth $null).ticket
        $game = Start-Process -FilePath $exe -WorkingDirectory $ClientCopy -PassThru -ArgumentList @(
            '-stage', '-dev', '-lang', 'en-eu-x-gm', '-noupdate', '-threads', '4', '-launcher_ticket', $ticket)
        Remove-Variable ticket
        Write-Host 'Jogo aberto. Clique em Start Game quando quiser; o login vale enquanto este launcher estiver aberto.'
        $game.WaitForExit()
        Write-Host 'Jogo fechado.'
    }
} finally {
    try { Invoke-Launcher 'logout' $auth $null | Out-Null } catch { }
    Remove-Variable auth, login -ErrorAction SilentlyContinue
    Write-Host 'Sessão do launcher encerrada.'
}
