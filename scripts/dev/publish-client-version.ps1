# Publica uma versão do cliente para o launcher (docs/future/game-launcher.md 4.7 e 4.9).
#
# 1. Monta a versão do zero a partir do 1.69 EU (scripts/client/build_client_version.py), com o
#    conteúdo publicado no painel; arquivos iguais aos da última versão montada viram hard links
#    somente leitura. O endereço do servidor não entra na versão: o launcher grava no heroes.db3 de
#    cada jogador o host do servidor de atualização (como no pw-universal). -EndpointHost só serve
#    para um cliente distribuído sem o launcher.
# 2. Publica no armazém do serviço updates (client-dist publish pela imagem do Docker, sem Rust no
#    host): só o que o armazém não tem, deltas contra a versão anterior, manifesto e versao.json
#    assinados. Os launchers atualizam na próxima abertura.
#
# Uso (pasta do projeto, Docker com a imagem vindictus-server:local montada):
#   .\scripts\dev\publish-client-version.ps1 -Title "Conteúdo de 08/11"
#   .\scripts\dev\publish-client-version.ps1 -Version 2 -Title "Só até a Delia"
# Leva ~25 min para montar (cópia de ~16 GB e os pacotes da Miri) e alguns minutos para publicar.
param(
    [Parameter(Mandatory)][string]$Title,
    [string]$EndpointHost = '127.0.0.1',
    [string]$Notes = '',
    [ValidateSet(1, 2, 3)][int]$Version = 3,
    [string]$Name = ''
)
$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '../..')
Set-Location $root
if (-not [System.Net.IPAddress]::TryParse($EndpointHost, [ref]$null) -or $EndpointHost.Contains(':')) {
    throw 'EndpointHost deve ser um IPv4.'
}
$versions = Join-Path $root '.cache/client-versions'
$store = Join-Path $root ($env:CLIENT_DIST_DIR ?? '.cache/client-dist')
$keys = Join-Path $root ($env:CLIENT_DIST_KEYS_DIR ?? '.cache/client-dist-keys')
$keystream = Join-Path $root '.cache/play-client/hfs-keystream.bin'
if (-not $Name) { $Name = "v$Version-" + (Get-Date -Format 'yyyyMMdd-HHmm') }
$out = Join-Path $versions $Name
$logs = Join-Path $root ".cache/verification/client-publish/$Name"
New-Item -ItemType Directory -Force $logs | Out-Null

# A última versão montada (com relatório) dá os hard links.
$previous = Get-ChildItem $versions -Filter '*.report.json' -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
$link = @()
if ($previous) {
    $link = @('--link-from', (Join-Path $versions ($previous.Name -replace '\.report\.json$', '')))
}
Write-Host "Montando a versão $Version em $out (log: $logs\build.log)…"
& py -3.13 scripts/client/build_client_version.py --version $Version --out $out --endpoint-host $EndpointHost @link *> (Join-Path $logs 'build.log')
if ($LASTEXITCODE -ne 0) { throw "A montagem falhou; veja $logs\build.log." }

Write-Host 'Publicando no armazém do updates…'
$mounts = @(
    '-v', "${store}:/client-dist",
    '-v', "${keys}:/keys:ro",
    '-v', "${out}:/tree:ro",
    '-v', "${keystream}:/ks.bin:ro"
)
& docker run --rm @mounts vindictus-server:local client-dist publish --store /client-dist --keys /keys `
    --tree /tree --keystream /ks.bin --title $Title --notes $Notes *> (Join-Path $logs 'publish.json')
if ($LASTEXITCODE -ne 0) { throw "A publicação falhou; veja $logs\publish.json." }
Get-Content (Join-Path $logs 'publish.json') -Tail 1
Write-Host "Publicado. Painel → Versões do cliente mostra a versão nova."
