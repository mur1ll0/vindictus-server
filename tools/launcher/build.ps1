# Compila o launcher para rodar do Windows 7 ao 11 (mesma receita do pw-launcher, B275 do pw-universal).
#
# Desde o Rust 1.78 os alvos *-pc-windows-msvc exigem Windows 10 (a std importa WaitOnAddress,
# ProcessPrng, GetSystemTimePreciseAsFileTime). Os alvos *-win7-windows-msvc trocam essas chamadas por
# equivalentes do Windows 7, mas são Tier 3: a std é recompilada do fonte (-Zbuild-std), o que pede o
# componente rust-src e RUSTC_BOOTSTRAP=1 no Rust estável. O CRT vai estático (+crt-static): o exe não
# depende do Universal CRT, que o Windows 7 só tem com o KB2999226.
#
# O padrão é 32 bits: o mesmo exe roda no Windows 7 de 32 e de 64 bits e no 10/11, como o próprio jogo.
#
# Uso:  .\build.ps1            -> target\i686-win7-windows-msvc\release\vindictus-launcher.exe
#       .\build.ps1 -Bits 64   -> target\x86_64-win7-windows-msvc\release\vindictus-launcher.exe
param([ValidateSet(32, 64)][int]$Bits = 32)
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
$target = if ($Bits -eq 32) { 'i686-win7-windows-msvc' } else { 'x86_64-win7-windows-msvc' }
rustup component add rust-src | Out-Null
$env:RUSTC_BOOTSTRAP = '1'
$env:RUSTFLAGS = '-C target-feature=+crt-static'
cargo build --release --locked --target $target -Zbuild-std=std
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host "pronto: target\$target\release\vindictus-launcher.exe"
