# nmcogame.dll do laboratório (32 bits, Windows 7+): mesmo alvo do launcher.
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
$target = 'i686-win7-windows-msvc'
rustup component add rust-src | Out-Null
$env:RUSTC_BOOTSTRAP = '1'
$env:RUSTFLAGS = '-C target-feature=+crt-static'
cargo build --release --target $target '-Zbuild-std=std,panic_abort'
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host "pronto: target\$target\release\nmcogame.dll"
