# PvP, servidor dedicado `srcds.exe` e portas de DS — investigação I-P1/I-P2/I-P3

Investigação de 03/11/2026 das perguntas I-P1, I-P2 e I-P3 de
[`social-systems.md`](social-systems.md) (bloco B5 social/PvP, decisão D-S2).
O `srcds.exe` foi analisado por **REA (Ghidra, leitura estática)**; os `.gfx`, os
mapas e os `.config` são **evidência de leitura**. **Nenhum binário legado foi
executado** nesta investigação (ver I-P1 e `AGENTS.md`).

## Resumo executivo

- **I-P3:** não existe `ServiceCore.dll.config` de en-EU nem de en-US no
  `ServidorVindictus`. O único arquivo é o do **`zh-tw-x-gm`** — por isso o
  operador só achou esse. Ele traz `DSExecPath`, `DSPort 26000`,
  `DSServicePort 27018`, `ClientDSServicePort 28018`,
  `DSAuxParams -allowdebug -dev -nonmco -lang zh-tw-x-gm` e o **relay de PvP em
  `27006`** (UDP 27016/27006). Valores completos abaixo.
- **I-P2:** a UI do 1.69 EN **mostra o botão de PvP** (flag `UI_PvPButton` ligada)
  e expõe os modos **Arena (p05), DeathMatch (p03) e GodMatch (p04)**. CTF (p01),
  Survivor (p02) e FruitMatch (p06) estão desligados no EU. Mapas no HFS:
  **p02–p07** (p01 ausente). A UI de partida é `ArenaDialog.gfx` +
  `ArenaPlayerCountDialog.gfx`.
- **I-P1:** **decidi não executar o `srcds.exe`.** A REA mostra que ele embute o
  **Nexon Game Security** (anticheat que pode carregar componente de sistema),
  chama `gamelog.heroes.nexon.com` e está **empacotado com VMProtect** (conduta não
  auditável). O comportamento esperado no console está documentado abaixo a partir
  da evidência estática + arquitetura do Source; há uma receita segura caso o
  operador queira a saída real numa VM isolada.

---

## I-P3 — `ServiceCore.dll.config`: portas de DS, PvP e relays

### Não há config de en-EU/en-US — só o `zh-tw-x-gm`

Busca dirigida (sem varredura de disco, que travou a máquina do operador):

```
ServidorVindictus/Server/
  ├─ HeroesCommandClient/  HeroesOpTool/  RCClient/  RCServer/
  ├─ NMServer.exe   InstallServices.bat
  └─ zh-tw-x-gm/Bin/ServiceCore.dll.config   ← único ServiceCore.dll.config
```

- `ServidorVindictus/Server` só tem a pasta de região **`zh-tw-x-gm`**. Não existe
  `en-EU`/`en-US` ali.
- `Vindictus1.69EN/.../en-EU` é o **cliente** (tem `srcds.exe`, mas é config de
  cliente, não o `ServiceCore.dll.config` do serviço).
- `C:\Windictus 2023` é cliente (tem `srcds.exe`, sem `ServiceCore.dll.config`).

Ou seja, o `zh-tw-x-gm` é o **único** template de servidor preservado. Os valores
de uma implantação en-EU seriam os mesmos na estrutura, mudando o `DSExecPath` (o
caminho real do en-EU) e o `-lang` do `DSAuxParams` para `en-eu`/`Vindictus-EU`;
esses valores específicos do en-EU **não existem na evidência**.

### Valores do `zh-tw-x-gm/Bin/ServiceCore.dll.config`

Servidor dedicado (seção `<ServiceCore.Properties.Settings>`):

| Setting | Valor |
|---|---|
| `DSExecPath` | `C:\Development\vindictus\zh-tw-x-gm\client\srcds.exe` |
| `DSPort` | **26000** (porta base do DS; cada instância usa `DSPort + i`) |
| `DSServicePort` | **27018** (canal de controle DSService ↔ DS) |
| `ClientDSServicePort` | **28018** |
| `DSAuxParams` | `-allowdebug -dev -nonmco -lang zh-tw-x-gm` |
| `DSGiantCount` | -1 |
| `DSRedirectConsole` | False |
| `DSGiantRaidMachineCount` | 0 |
| `DSNormalRaidMachineCount` | 0 |
| `DSPvpMachineCount` | 0 |
| `DSDevMachineCount` | -1 (dev roda o DS localmente) |

Relays e portas de rede (seção `<heroes>`):

| Relay | TCP | UDP |
|---|---|---|
| `frontend` | 27015 | — (capacity 65536) |
| `channelrelay` | 27003 | — |
| `relay` (batalha ProudNet) | **27005** | 27015, 27005 |
| **`pvprelay` (relay de PvP)** | **27006** | **27016, 27006** |
| `pingrelay` | 27017 | 27017, 27018 |

Outras portas citadas: `ReportServiceListenPort` 27020, `AdminClientServicePort`
27011, `LocationServicePort` 42, `CashShopPort` 6500. Strings de conexão SQL Server
apontam `127.0.0.1,1433` (referência legada; não migrar bruto).

> Confere com o que o `DSEntity.cs` monta (ver `social-systems.md`): o DS é lançado
> com `-connectport <DSServicePort>` (27018) e `-port <DSPort + i>` (26000+i). O
> `-port 27050` do comando do operador **não** vem do original.

---

## I-P2 — botão e modos de PvP na UI do 1.69 EN, e mapas p0x

### O botão de PvP aparece? Sim.

A exibição é controlada pela **FeatureMatrix** do `heroes.db3` EN-EU
(`CurrentVer 3.09.00`), já extraída em `social-systems.md`:

- **Ligadas:** `UI_PvPButton`, `Pvp_Launch`, `Arena`, `DeathMatch`, `GodMatch`,
  `CombatFormula_PVP`, `PVPPartyCountLimit`, `PvpBanPenalty`.
- **Desligadas:** `PvP` (os modos P2P parciais), `PvP_Tournament`.

Como `UI_PvPButton` está ligada, **o cliente mostra o botão de PvP**. Os modos que
aparecem são os de servidor dedicado habilitados: **Arena, DeathMatch e GodMatch**.
Os modos P2P (CTF, Survivor, FruitMatch) ficam ocultos porque `PvP` está desligada.

### UI de partida (scaleform)

Só há duas telas de PvP em `en-EU/scaleform`:

- **`ArenaDialog.gfx`** (60.898 B) — matchmaking. Ponte `fscommand("ArenaDialog", …)`.
  Vocabulário AS2: `Button_Matchup`/`MatchupOk`/`Matchup0`, `Button_Cancel`,
  `Button_View`/`ViewMode`/`ViewNoPlyaer`, `MemberList`
  (`WaitingPlayer`/`SpecatorPlayer`), `TotalMemberList`, `Consecutive` (vitórias
  seguidas), `currentMode`/`modeName`/`SetMode`, `OnSetPvpHpGaugePosition`,
  `isMatchup`/`isMatchupOk`. É a tela de fila/espera/espectador e placar dos modos
  habilitados.
- **`ArenaPlayerCountDialog.gfx`** (16.014 B) — contagem
  Waiting/View/Total de jogadores.

Não há `.gfx` separado por modo (DeathMatch/GodMatch) nem um "PvpDialog"
dedicado — os modos compartilham o fluxo da Arena e as mensagens de PvP do servidor
(`PvpQueryChannelList` 130, `PvpRegister` 442, `PvpChannelList` 129, etc.;
ver etapas PV2–PV4 em `social-systems.md`).

### Mapas p0x no HFS do 1.69 EN

Em `en-EU/hfs_extracted/maps`, cada modo é uma pasta `p0x/` com um único `.bsp`:

| Pasta | `.bsp` | Tamanho | Modo (de `social-systems.md`) | Estado no EU |
|---|---|---|---|---|
| p01 | **ausente** | — | CTF | desligado |
| p02 | p02.bsp | 7,6 MB | Survivor | desligado (mapa presente) |
| p03 | p03.bsp | 6,4 MB | **DeathMatch** | **ligado** |
| p04 | p04.bsp | 25,2 MB | **GodMatch** | **ligado** |
| p05 | p05.bsp | 20,6 MB | **Arena** | **ligado** |
| p06 | p06.bsp | 7,8 MB | FruitMatch | desligado (mapa presente) |
| p06_halloween | p06_halloween.bsp | 7,8 MB | FruitMatch (sazonal) | — |
| p07 | p07.bsp | 4,9 MB | extra (não mapeado nas flags) | mapa presente |

p01 (CTF) **não** está empacotado no 1.69 EN — coerente com o modo desligado. p02,
p06 e p07 existem em disco mesmo com os modos desligados/indefinidos nas flags.

---

## I-P1 — `srcds.exe`: REA completa e por que não o executei

### Identidade do binário (en-EU)

| Campo | Valor |
|---|---|
| Caminho | `Vindictus1.69EN/Vindictus EU/en-EU/srcds.exe` |
| SHA-256 | `a13b544ff7715c444c77e40fa23933b8748799b9fc25605e38122d45e6e18b51` |
| Formato | PE32 x86, 843.536 B, CRT Visual Studio 2008 |
| Manifesto | `requestedExecutionLevel level="asInvoker"` (não pede admin) |
| Recursos | só ícones + manifesto (sem VERSIONINFO) |

(O `srcds.exe` do `C:\Windictus 2023` é outra versão, 1.034.312 B.)

### O que o `srcds.exe` é — um lançador do Source empacotado com VMProtect

A REA (Ghidra) mostra que **o `srcds.exe` é apenas o lançador**: `___tmainCRTStartup`
(CRT VS2008) → `main` em `0x404150`, que faz **tail-call para `0x45d443`**, dentro
do segmento **`.vmp0` (0x44a000–0x48b800, ~262 KB)**. Ou seja, **toda a lógica do
lançador está virtualizada por VMProtect**: não há xrefs recuperáveis para as
strings-chave e a conduta só pode ser lida pelos artefatos de dados, não pelo fluxo.

Segmentos: `.text` 228 KB · `.rdata` 29 KB · `.data` 29 KB · **`.vmp0` 262 KB** ·
`.reloc` · `.rsrc` 273 KB. 1.722 procedimentos, 614 strings definidas.

**Papel:** é o `srcds` do **Source engine**. Strings `DedicatedMain` e
`bin\dedicated.dll` → ele carrega `bin/dedicated.dll` e chama o export
`DedicatedMain` (padrão do Source). A lógica de jogo/DS está em
`dedicated.dll` + `engine.dll` + `server.dll` (todos presentes no `en-EU/bin`).

### O que as strings revelam (conduta)

1. **Nexon Game Security embutido** (anticheat). RTTI
   `.?AVNGSecurity@HeroesNGSecurity@@` e `.?AVINGSecurity@HeroesNGSecurity@@`;
   console: `Nexon Game Security Start - OK` / `Nexon Game Security Start - Failed, %d`.
   O NGS é inicializado pelo próprio `srcds` no arranque.
2. **Telemetria para a Nexon:** `http://gamelog.heroes.nexon.com/common/ClientHostTimeOutLog.aspx?category=%d&message=`
   (e a variante `...TestServer.aspx`).
3. **Acoplamento com Steam** (padrão Source): `SteamExe`, `steam.exe`,
   `steam_dev.exe`, `Valve_SteamIPC_Class`, `Software\Valve\Steam`, `-steam`,
   erro `Error running game: could not find steam.exe to launch`.
4. **Região por `-lang`:** tabela `Vindictus-EU-GM`, `Vindictus-EU`, `Vindictus-GM`,
   `Vindictus`, `Heroes-TH/TW/JP/CN[-GM]`, `ko-KR-x-dev`; `.\cfg\language.cfg`.
   Define também as pastas de crash/captura em `Documentos\Vindictus EU[ GM]\Report
   Error | Screen Shot | Video Capture` (bate com o relatório de crash em
   `Documents\Vindictus EU GM\Report Error`).
5. **Console/pipe:** imports `SetConsoleCtrlHandler`, `WriteConsoleA/W`,
   `GetConsoleCP/Mode`, `PeekNamedPipe` → coerente com `-console` e com o DSService
   injetando comandos por pipe (`DSRedirectConsole`).

> Os flags `-connectip`, `-connectport`, `-port`, `+is_giant_raid`, `-nohltv`,
> `-nonmco`, `-allowdebug`, `-dev`, `-owner_pid`, `-owner_dsid` **não** são strings
> do `srcds.exe`: quem os lê é `dedicated.dll`/`engine.dll`/`server.dll`. O lançador
> repassa a linha de comando e só trata `-lang`/`-steam` ele mesmo.

### Por que não executei (I-P1)

O operador deixou a execução a meu critério. Decidi **não rodar** porque a REA
mostra três riscos concretos:

- **Nexon Game Security**: anticheat proprietário que pode instalar/carregar
  componente de sistema (driver), hooks e proteção de processo. Iniciar um anticheat
  num host de trabalho é uma ação de efeito de sistema e difícil de reverter.
- **Saída de rede para a Nexon** (`gamelog.heroes.nexon.com`).
- **VMProtect**: a conduta real está virtualizada e **não é auditável** antes de
  rodar.

Isso se soma à regra do `AGENTS.md` ("Never execute unreviewed legacy services
merely to inspect metadata") e à carga da máquina. Executar aqui não é uma inspeção
de metadados — é subir um serviço anticheat opaco com chamada externa.

### O que o console mostraria (predição a partir da evidência estática)

Com os argumentos do operador numa cópia de cliente (que tem `bin/dedicated.dll`,
`engine.dll`, `server.dll` e os mapas), o comportamento esperado é:

1. Abre janela de console (`-console`).
2. **Inicializa o Nexon Game Security** → imprime `Nexon Game Security Start - OK`
   ou `... - Failed, %d`. Sem o runtime/componente do NGS no host, pode **falhar e
   abortar** ou exibir erro de segurança.
3. Monta o filesystem do Source (`gameinfo`, HFS/VPK). `-dev -allowdebug` ligam o
   console de desenvolvimento.
4. **Sem `+map`** nos argumentos, o DS não autocarrega mapa — quem diz o mapa/modo é
   o DSService. Sozinho, ele fica ocioso no console (ou erra esperando mapa).
5. `-connectip 127.0.0.1 -connectport <porta>`: o DS tenta **conectar como cliente**
   ao canal de controle do DSService nesse ip:porta. **Sem nada ouvindo → recusa de
   conexão / retry / timeout no log.** É a "tentativa de conexão" que o operador
   perguntou: sim, ele tenta `127.0.0.1:<porta>` e falha.
6. `-port 27050`: socket de jogo/relay em 27050 (valor não-original).
7. `+is_giant_raid 0`: var de console (raide gigante PvE desligada).
8. `-nohltv`: sem SourceTV. `-nonmco`: módulos do jogo pulam o Nexon Messenger.
9. `-lang en-eu`: mapeia para a região `Vindictus-EU` (título/registro + pastas de
   crash).

Resultado provável: **inicia, tenta o NGS, monta conteúdo e trava tentando alcançar
o DSService ausente em `127.0.0.1:<porta>`, logando falha de conexão**; não hospeda
partida jogável sem o DSService. Erros de Steam/Nexon aparecem se o NGS não inicia
ou se o caminho de Steam for acionado.

### Receita segura, se o operador quiser a saída real

1. VM descartável (Windows) **sem rede** ou com rede isolada, com snapshot antes.
2. Copiar a pasta `en-EU` inteira (precisa de `bin/`, `cfg/`, `hfs/`, `maps/`).
3. Rodar o comando do operador; para forçar um mapa de teste, acrescentar `+map p05`
   (Arena). Capturar o console (`-condebug` grava em `console.log`).
4. Observar: linha do Nexon Game Security, erros de Steam/Nexon, e as tentativas de
   conexão em `-connectip`. Descartar o snapshot ao fim.

---

## Impacto no roteiro

- **I-P3 resolvida:** usar o template `zh-tw-x-gm` como referência das portas de DS;
  para en-EU, `DSAuxParams` seria `-allowdebug -dev -nonmco -lang en-eu[-x-gm]`.
  Relay de PvP = **27006** (TCP) / 27016+27006 (UDP); DSService = 27018; DSPort base
  = 26000. (Dá a porta do canal DSService ↔ DS, TCP em `DSServicePort`; o formato das mensagens continua no I-S4.)
- **I-P2 resolvida:** PvP ligado no EU com Arena/DeathMatch/GodMatch; mapas p03/p04/
  p05 presentes; UI é a `ArenaDialog`. PV1 (duelo livre) não depende de DS; PV2–PV4
  dependem da decisão **D-S2**.
- **I-P1 resolvida por predição; execução declinada.** Para a decisão **D-S2**
  (rodar o `srcds.exe` no host): há o custo do Nexon Game Security + VMProtect +
  dependência do DSService. Alternativas já listadas em D-S2: modos P2P (exigem
  mudar o perfil) ou ficar no duelo livre (PV1) por enquanto.

## Apêndice — reprodução

- REA (Ghidra via servidor REA): `open_binary` no `srcds.exe`, `binary_overview`,
  `list_strings`, `inspect_pe_resources`, `procedure_pseudo_code` de
  `___tmainCRTStartup` e `main` (`FUN_00404150`). Nenhuma execução.
- `.gfx` (scratchpad): varredura de strings ASCII em `ArenaDialog.gfx` e
  `ArenaPlayerCountDialog.gfx`.
- Disco (somente listagem dirigida): `ServiceCore.dll.config` em
  `ServidorVindictus/Server`; mapas `p0x` em `en-EU/hfs_extracted/maps`.
- Lidos (read-only): `srcds.exe`, `ArenaDialog.gfx`, `ArenaPlayerCountDialog.gfx`,
  `ServidorVindictus/Server/zh-tw-x-gm/Bin/ServiceCore.dll.config`, árvore de
  `maps`.
