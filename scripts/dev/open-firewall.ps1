# Libera no Firewall do Windows as portas TCP do laboratório para outros computadores
# (docs/future/game-launcher.md 4.9). Rode como administrador, uma vez; -Remove desfaz.
# Portas: 8081 bootstrap (lista de servidores, cash shop), 8090 updates (launcher), 27015 login,
# 27003 canal, 27005 relay das missões, 27080 compra do cash shop, 27090 painel e login do launcher,
# 47611 messenger. O PostgreSQL (5433) e a API operacional (8080) continuam só no loopback.
# Perfil Private: vale nas redes marcadas como privadas. Se o Windows marcou a rede (ou o adaptador do
# Radmin VPN) como pública, use -AllProfiles.
param([switch]$Remove, [switch]$AllProfiles)
$ErrorActionPreference = 'Stop'
$name = 'Vindictus laboratório (TCP)'
Get-NetFirewallRule -DisplayName $name -ErrorAction SilentlyContinue | Remove-NetFirewallRule
if ($Remove) {
    Write-Host 'Regra removida.'
    exit 0
}
$scope = if ($AllProfiles) { 'Any' } else { 'Private' }
New-NetFirewallRule -DisplayName $name -Direction Inbound -Protocol TCP -Action Allow -Profile $scope `
    -LocalPort 8081, 8090, 27003, 27005, 27015, 27080, 27090, 47611 | Out-Null
Write-Host "Regra '$name' criada (perfil $scope)."
