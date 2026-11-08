# Pacote do launcher para outros computadores (docs/future/game-launcher.md 4.9).
#
# Compila o launcher (tools/launcher/build.ps1, Windows 7 a 11, 32 bits), publica o exe no armazém
# para a autoatualização (client-dist launcher, pela imagem do Docker) e monta
# .cache/launcher-package/VindictusLauncher.zip com o exe e o vindictus-launcher.json apontando para
# este servidor. No outro computador: extrair numa pasta, abrir o vindictus-launcher.exe, escolher uma
# pasta vazia para o jogo, "Instalar o jogo aqui", criar a conta ou entrar, Jogar.
#
# Uso: .\scripts\dev\package-launcher.ps1 -PublicHost 192.168.1.13 [-SkipBuild]
param(
    [Parameter(Mandatory)][string]$PublicHost,
    [switch]$SkipBuild
)
$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '../..')
Set-Location $root
if (-not [System.Net.IPAddress]::TryParse($PublicHost, [ref]$null) -or $PublicHost.Contains(':')) {
    throw 'PublicHost deve ser um IPv4 (o mesmo PUBLIC_HOST do .env).'
}
$launcher = Join-Path $root 'tools/launcher'
if (-not $SkipBuild) {
    & (Join-Path $launcher 'build.ps1')
    if ($LASTEXITCODE -ne 0) { throw 'A compilação do launcher falhou.' }
}
$exe = Join-Path $launcher 'target/i686-win7-windows-msvc/release/vindictus-launcher.exe'
$version = (Select-String -Path (Join-Path $launcher 'Cargo.toml') -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value

$store = Join-Path $root ($env:CLIENT_DIST_DIR ?? '.cache/client-dist')
$keys = Join-Path $root ($env:CLIENT_DIST_KEYS_DIR ?? '.cache/client-dist-keys')
& docker run --rm -v "${store}:/client-dist" -v "${keys}:/keys:ro" -v "${exe}:/launcher.exe:ro" vindictus-server:local `
    client-dist launcher --store /client-dist --keys /keys --exe /launcher.exe --version $version
if ($LASTEXITCODE -ne 0) { throw 'A publicação do launcher no armazém falhou.' }

$package = Join-Path $root '.cache/launcher-package'
$folder = Join-Path $package 'Vindictus'
Remove-Item -Recurse -Force $folder -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $folder | Out-Null
Copy-Item $exe (Join-Path $folder 'vindictus-launcher.exe')
@{ server = "http://${PublicHost}:27090"; game_folder = ''; last_login = ''; game_as_admin = $false } |
    ConvertTo-Json | Set-Content -Encoding utf8NoBOM (Join-Path $folder 'vindictus-launcher.json')
@"
Vindictus (laboratório)

1. Abra o vindictus-launcher.exe.
2. Em "Pasta do jogo", clique em "Escolher…" e escolha uma pasta vazia (precisa de ~16 GB livres).
3. Clique em "Instalar o jogo aqui" e espere o download (~12 GB). Pode fechar e continuar depois.
4. Sem conta: "Criar conta". Com conta: login e senha, "Entrar", "Jogar", "Start Game".

Servidor: http://${PublicHost}:27090  (atualização: porta 8090)
O launcher se atualiza sozinho quando houver versão nova.
"@ | Set-Content -Encoding utf8NoBOM (Join-Path $folder 'LEIA-ME.txt')
$zip = Join-Path $package 'VindictusLauncher.zip'
Remove-Item -Force $zip -ErrorAction SilentlyContinue
Compress-Archive -Path (Join-Path $folder '*') -DestinationPath $zip
Write-Host "Pronto: $zip (launcher $version, servidor $PublicHost)"
