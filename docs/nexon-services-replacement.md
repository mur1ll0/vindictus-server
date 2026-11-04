# Substituir os serviços da Nexon por serviços próprios

Análise de 03/11/2026 (pedido do operador: não depender de serviços da Nexon). Fontes: tabelas de importação/exportação PE (parser Python próprio, só cabeçalhos) dos binários do 1.69 EN (`Vindictus1.69EN/Vindictus EU/en-EU`), contagem de strings, `docs/friends-nexon-messenger-investigation.md`, `docs/pvp-srcds-investigation.md` e `docs/future/game-launcher.md`. Nenhum binário executado.

## Inventário

| Dependência da Nexon | Onde | Situação | Substituição própria |
|---|---|---|---|
| Login/passport, Nexon Launcher (`nexon_api.dll`, `nxapi_init`) | `Vindictus.exe`, `engine.dll` | Já substituído: login por login/senha e ticket do launcher Rust | Feito |
| Billing/cash shop | serviços C# | Já substituído (B3 cash shop) | Feito |
| Anticheat do cliente (`NGClient.aes`, BlackCipher, HackShield, XignCode) | `Vindictus.exe` importa `NGClient.aes` #1/#2 | Cliente do laboratório roda sem os serviços; nada a portar | Não portar |
| Telemetria `gamelog.heroes.nexon.com` | `srcds.exe`, `dedicated.dll` (2 refs `nexon.com`) | Domínio morto; falha silenciosa | Ignorar ou bloquear no hosts |
| Nexon Messenger (amigos, presença, categorias) | `engine.dll` → `nmcogame.dll` → `nmconew.dll` → NMServer `:47611`; no servidor, `NMLinkW.dll` | Não existe servidor utilizável: o `NMServer.exe` Go só tem login | **Messenger próprio em Rust** (abaixo) |
| Nexon Game Security no DS | `srcds.exe` (VMProtect) | Não executado | **Lançador próprio do DS** (abaixo) |
| Playfeed, Nexon.Com Group/Community, SSO | serviços C# | Ramos desligados no perfil EU (`InGameGuild`, sem playfeed) | Não portar |

## Messenger próprio

A superfície do cliente é pequena. O `engine.dll` importa do `nmcogame.dll` só cinco funções: `NMCO_SetLocaleAndRegion`, `NMCO_SetVersionFileUrlA`, `NMCO_SetPatchOption`, `NMCO_MemoryFree` e `NMCO_CallNMFunction`. O `nmcogame.dll` (371 KB, sem VMProtect) repassa ao `nmconew.dll`, que exporta `NMCO_CallNMFunc`, `NMCO_CallNMFunc2` e `NMCO_MemoryFree`. O `client.dll` não importa nada do Messenger.

Caminhos, do recomendado ao menos:

1. **`nmcogame.dll` próprio (Rust, 32 bits, como `tools/launcher`)**: exporta as seis funções e fala com um módulo `messenger` do nosso servidor por um protocolo nosso. Sai do caminho o `nmconew.dll` e o protocolo da Nexon. As contas, a presença e as guildas vêm direto do servidor Rust; o que o `NMLinkW.dll` empurrava (nível, apelido, guilda online, mensagem de sistema) vira chamada interna no ponto único de difusão (`party_hub`).
   - Falta recuperar: o formato serializado de `NMCO_CallNMFunction` (código da função + buffer) e como os eventos voltam ao jogo (janela/mensagem ou callback). É uma análise dirigida: chamadas a `NMCO_CallNMFunction` no `engine.dll` e as seis exportações do `nmcogame.dll`, sem analisar o binário inteiro.
   - Distribuição: o launcher entrega o `nmcogame.dll` substituto e o `nmconew.ini` deixa de ser usado.
2. **Servidor compatível com o `nmconew.dll`**: mantém as DLLs originais e reimplementa o protocolo do NMServer (cifra XOR e login em três etapas, legíveis no `NMServer.exe` Go; os pacotes de amigos exigem análise do `nmconew.dll`, 2,2 MB). Superfície maior e protocolo da Nexon.

## Servidor dedicado sem o `srcds.exe`

O `srcds.exe` é só o lançador: carrega `bin/dedicated.dll`, que exporta `DedicatedMain`, `CreateInterface` e `AntiCpXCnt`, e entrega a linha de comando. Um lançador próprio (Rust, 32 bits) que faz `LoadLibrary("bin\\dedicated.dll")` e chama `DedicatedMain` com os argumentos do DSService elimina o NGS e o VMProtect do `srcds.exe`.

- Restam binários da Nexon: `dedicated.dll` e `server.dll` também têm a seção `.vmp0`, e o `dedicated.dll` tem duas referências a `nexon.com` e o `AntiCpXCnt`. Só uma execução isolada mostra se sobem sem a Nexon.
- O DS é processo Windows: roda no host, fora do Docker Linux, gerido pelo nosso DSService (I-S4).
- O jogo em si (`server.dll`, motor Source) não é reescrevível no escopo da migração.

## Alternativa sem DS: partida PvP hospedada por um jogador

O cliente já roda o `server.dll` como host nas missões (P2P pelo relay, B2/B5), e os modos parciais do PvP original usam esse mesmo caminho (`PartialMatchingSystem`). A exigência de DS para Arena, DeathMatch e GodMatch é política do `PvpService` (`FullMatchingSystem`/`ArenaMatchingSystem`), não do cliente. Testar no cliente se um jogador hospeda p05/p03/p04 pelo `PvpGameHosted` elimina o DS inteiro. O custo é a vantagem do host (latência zero). Não exige executar binário legado: é um experimento do nosso servidor com o cliente do laboratório.

## Recomendação

- Amigos: `nmcogame.dll` próprio + módulo `messenger` no servidor (SO4), depois de SO1/SO2.
- PvP: primeiro o experimento do host jogador (PV2–PV3 em P2P); só se o cliente recusar, o lançador próprio do DS numa VM isolada.
