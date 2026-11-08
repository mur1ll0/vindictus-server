# Importa, uma única vez, o cliente base para o servidor (docs/future/game-launcher.md 4.10; decisão do
# operador de 08/11/2026, "comando no host").
#
# 1. Copia (só leitura na origem) o 1.69 EU original e as evidências que a receita lê para a pasta de dados
#    do cliente no servidor, ${CLIENT_SERVER_DIR} (padrão .cache/client-server), em inputs/workspace com o
#    mesmo leiaute da pasta VindictusPrivate: Vindictus1.69EN/Vindictus EU/en-EU, o trecho do Vindictus.z03
#    com a Mss32.dll boa (256 KB, não o volume de 1,9 GB), os heroes.db3 de referência e o
#    heroesContents.db3 do servidor original; mais a nmcogame.dll compilada (tools/nmcogame/build.ps1).
# 2. Monta a imagem do construtor do cliente e roda `client-builder seed`: a receita monta no contêiner a
#    árvore do cliente do servidor (camada 3, com os backups) e põe no rascunho do painel o que difere da
#    versão publicada (de preferência só o conteúdo publicado depois dela).
#
# Depois disso o original não é mais necessário nesta máquina: as versões novas saem do painel
# ("Versões do cliente" → rascunho → Publicar). Num servidor dedicado, copie a pasta ${CLIENT_SERVER_DIR}.
#
# Uso (pasta do projeto, Docker em execução):
#   .\scripts\dev\import-client-base.ps1                 # copia e monta
#   .\scripts\dev\import-client-base.ps1 -SkipCopy       # só monta (a cópia já foi feita)
# Leva ~15 min para copiar (~13 GB) e ~25 min para montar.
param(
    [string]$Workspace = '',
    [switch]$SkipCopy
)
$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '../..')
Set-Location $root
if (-not $Workspace) { $Workspace = (Resolve-Path (Join-Path $root '..')).Path }
$server = Join-Path $root ($env:CLIENT_SERVER_DIR ?? '.cache/client-server')
$inputs = Join-Path $server 'inputs/workspace'
$logs = Join-Path $root '.cache/verification/client-builder'
New-Item -ItemType Directory -Force $logs, $inputs | Out-Null

function Copy-Evidence([string]$relative) {
    $source = Join-Path $Workspace $relative
    if (-not (Test-Path $source -PathType Leaf)) { Write-Host "  (sem $relative)"; return }
    $target = Join-Path $inputs $relative
    New-Item -ItemType Directory -Force (Split-Path $target) | Out-Null
    Copy-Item -LiteralPath $source -Destination $target -Force
}

if (-not $SkipCopy) {
    $original = Join-Path $Workspace 'Vindictus1.69EN/Vindictus EU/en-EU'
    if (-not (Test-Path (Join-Path $original 'bin/client.dll'))) { throw "Original 1.69 EU não encontrado em $original." }
    Write-Host "Copiando o 1.69 EU original para $inputs (log: $logs\import-robocopy.log)…"
    $target = Join-Path $inputs 'Vindictus1.69EN/Vindictus EU/en-EU'
    # /E sem /MIR: nunca apaga nada; a origem só é lida.
    & robocopy $original $target /E /COPY:DT /DCOPY:T /R:1 /W:1 /NP /NFL /NDL /LOG:"$logs\import-robocopy.log" | Out-Null
    if ($LASTEXITCODE -ge 8) { throw "robocopy falhou ($LASTEXITCODE); veja $logs\import-robocopy.log." }

    Write-Host 'Copiando as evidências da receita…'
    Copy-Evidence 'Client/sql/heroes.db3'
    Copy-Evidence 'TutorialRagezone/DB/client/sql/heroes.db3'
    Copy-Evidence 'ServidorVindictus/Server/zh-tw-x-gm/Bin/heroesContents.db3'
    # Só o registro da Mss32.dll do volume (recover_mss32.py, SLICE): 256 KB a partir do deslocamento dela.
    $volume = Join-Path $Workspace 'Vindictus1.69EN/Vindictus.z03'
    $slice = Join-Path $inputs 'Vindictus1.69EN/Vindictus.z03.mss32'
    New-Item -ItemType Directory -Force (Split-Path $slice) | Out-Null
    $in = [System.IO.File]::Open($volume, 'Open', 'Read', 'Read')
    try {
        $buffer = New-Object byte[] (256 * 1024)
        [void]$in.Seek(1544037482, 'Begin')
        $read = $in.Read($buffer, 0, $buffer.Length)
        [System.IO.File]::WriteAllBytes($slice, $buffer[0..($read - 1)])
    } finally { $in.Dispose() }
    $nmcogame = Join-Path $root 'tools/nmcogame/target/i686-win7-windows-msvc/release/nmcogame.dll'
    if (-not (Test-Path $nmcogame)) { throw 'Falta a nmcogame.dll: rode tools/nmcogame/build.ps1.' }
    Copy-Item -LiteralPath $nmcogame -Destination (Join-Path $server 'inputs/nmcogame.dll') -Force
}

Write-Host 'Montando a imagem do construtor do cliente…'
$env:COMPOSE_PARALLEL_LIMIT = '1'
& docker compose build client-builder *> (Join-Path $logs 'import-build.log')
if ($LASTEXITCODE -ne 0) { throw "A imagem não montou; veja $logs\import-build.log." }

Write-Host "Montando a árvore do cliente no servidor (log: $logs\import-seed.log)…"
& docker compose run --rm --no-deps client-builder client-builder seed --store /client-dist `
    --server-dir /client-server --edits /content-state/content-edits.json `
    --nmcogame /client-server/inputs/nmcogame.dll *> (Join-Path $logs 'import-seed.log')
if ($LASTEXITCODE -ne 0) { throw "A montagem falhou; veja $logs\import-seed.log e $server\build-state\seed.log." }
Write-Host 'Pronto. Painel → Versões do cliente mostra no rascunho o que difere da versão publicada.'
Write-Host 'Suba o serviço: docker compose up -d --wait client-builder'
