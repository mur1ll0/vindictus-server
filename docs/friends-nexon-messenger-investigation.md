# Amigos, Nexon Messenger e servidor dedicado — investigação I-A1/I-A2/I-A3

Investigação de 03/11/2026 das perguntas I-A1, I-A2 e I-A3 de
[`social-systems.md`](social-systems.md) (bloco B5 social/PvP, decisão D-S1).
Tudo aqui é **evidência de leitura**: nenhum binário foi executado. Os arquivos do
cliente e do `ServidorVindictus` são evidência read-only (ver `AGENTS.md`).

## Método

- `.gfx` Scaleform: são SWF não comprimidos (assinatura `GFX\x09`). Um parser de
  tags SWF em Python extraiu os `ActionConstantPool` e as strings de `ActionPush`
  do bytecode AS2 (DoAction/DoInitAction/DefineButton2, inclusive em sprites
  aninhados). Isso recupera o vocabulário completo da tela: comandos emitidos,
  callbacks registrados e chaves de texto. (FFDec não estava instalado; o parser
  dá a mesma informação de identificadores sem baixar ferramenta.)
- Binários nativos (`.dll`/`.exe`): extração de strings ASCII+UTF-16 e, para o
  `NMServer.exe`, descompilação pontual de `main.StartServer` via REA (Ghidra).
  Nenhuma análise de binário inteiro do `client.dll` (limite de carga da máquina).

## Inventário dos artefatos

| Papel | Caminho | Tamanho | Observação |
|---|---|---|---|
| UI comunidade | `Vindictus1.69EN/Vindictus EU/en-EU/scaleform/CommunityDialog.gfx` | 104.520 B | abas amigos/guilda/bloqueio/sussurro |
| UI recomendar amigo | `.../hfs_extracted/scaleform/RecommendFriendDialog.gfx` | — | flag `RecommendFriend` (ligada) |
| UI Nexon/Facebook | `.../hfs_extracted/scaleform/NexonChatDialog.gfx` | — | compartilhar em NexonHome/Facebook (não é a lista de amigos) |
| Messenger cliente | `.../en-EU/nmconew.dll` | 2.197.848 B | PDB `20130611_SSO+MSG_CLI ... nmconew.pdb` (Nexon Europe) |
| Messenger cliente | `.../en-EU/nmcogame.dll` | 371.032 B | cola do jogo ↔ nmconew |
| Messenger cliente | `.../en-EU/NMService.exe` | 2.001.240 B | processo auxiliar do messenger no cliente |
| Config messenger | `.../en-EU/nmconew.ini` | — | `[Settings] ServerIp/ServerPort` |
| Messenger servidor | `ServidorVindictus/Server/NMServer.exe` | 6.721.536 B | sha256 `3333454583bc98c2…` — **reimplementação Go** (ver I-A3) |
| Ponte jogo→messenger | `ServidorVindictus/Server/zh-tw-x-gm/Bin/NMLinkW.dll` | 835.584 B | PDB `MessengerApplication\NMLink\20110113` (Nexon original) |
| Servidor dedicado PvP | `.../en-EU/srcds.exe` + `bin/server.dll`, `bin/dedicated.dll` | — | Source engine (contexto I-P) |

---

## I-A1 — `CommunityDialog.gfx` e telas de amigos

### Ponte com o jogo

A tela **não usa `ExternalInterface.call`**. A ponte AS2 → host é o mecanismo
Scaleform **`fscommand`**, com um despachante interno (`setCommand`/`_command`,
log de depuração `[FS] command : `, `gfxExtensions`). O host nativo (`client.dll`)
recebe `fscommand("CommunityDialog", "<ação>")` e decide para onde encaminhar.

Prefixos `FSCommand:` presentes: `CommunityDialog` (namespace da tela),
`ShowElement`, `Debug`, `Warning`, `LocalizedText` (genéricos de UI).

### Abas (ordem EN-EU)

Botões `MC_TabButton0/1/3/4/5` (o índice 2 não tem rótulo → oculto no EU):

| Aba | Botão | Chave de texto |
|---|---|---|
| Amigos | 0 | `..._Tab_Button_Friend` |
| Guilda | 1 | `MC_TabPanel1`/`MC_GuildInfo` (sem chave `Tab_Button` isolada) |
| Bloqueados | 3 | `..._Tab_Button_Blocked` |
| Sussurro | 4 | `..._Tab_Button_Talk` |
| Histórico de sussurro | 5 | `..._Tab_Button_TalkHistory` |

### Ações que a UI emite (AS → host, via `fscommand`)

- **Amigos:** `AddFriend`, `DeleteFriend`, `SortFriendList`, `onAddFriendButton`,
  `InviteTalk` (`onInviteTalk`), `InviteParty`.
- **Menu de contexto do amigo** (`showFriendContextMenu`/`MC_FriendContextMenu`):
  Invite_Talk, Add_Friend, Invite_Party, Delete_Friend, Send_Chat, Send_Message,
  **Move_Group**, Show_Info, Assist, CharacterRank.
- **Bloqueio:** `BlockUser`, `unblockPlayer` (`Unblock.`), `addBlockedPlayerToList`,
  `clearBlockedPlayerList`, `numberOfBlockedPlayers`.
- **Guilda:** `InviteGuild`, `OpenGuildPage`, `SendChatGuild`, `showGuildContextMenu`,
  `showGuildInfo`, `removeAllGuildMembers`, `MC_GuildMemberListPanel`,
  `MC_GuildNoticePanel`, `MC_HasNotGuild`.
- **Sussurro/histórico:** `showTalkPanel`, `showTalkHistoryPanel`,
  `refreshTalkHistoryList`, `addReceviedTalkInvitation`, aceitar/rejeitar convite
  de sussurro, canais de chat (`openNewChattingChannel`, `closeChattingChannelByID/ByName`,
  `setChannelFocusByID/ByName`, `findEmptyChannel`, `initChannelTab`).

### Callbacks que a tela registra (host → AS)

`addFriend`, `addBlockedPlayerToList`, `clearBlockedPlayerList`,
`removeAllGroups`, `addGroup`, `removeAllGuildMembers`, `setTabByNum`,
`showFriendPanel`/`showBlockPanel`/`showGuildInfo`, `setCharacterInfo`
(RecommendFriend), `onlineMemberCount`, `getChannelIDbyCharacterName`,
`updateChannelsNum`, `getTalkHistoryMaxPageNum`, `showTalkHistoryPageAt`.

### Telas correlatas

- **`RecommendFriendDialog.gfx`**: `fscommand("RecommendFriendDialog", …)` com a
  ação `OnAddFriend`; callback `setCharacterInfo`. Corresponde à flag
  `RecommendFriend` (ligada no EU).
- **`NexonChatDialog.gfx`**: `fscommand("NexonChatDialog", …)` com `OnSend`,
  `OnFacebookSend`, `OnGuildAgitSend` e tipos `DIALOG_TYPE_FACEBOOK`/
  `DIALOG_TYPE_NEXON_HOME`. É **compartilhamento social (NexonHome/Facebook)**, não
  a lista de amigos do jogo.

### Para onde isso vai — servidor ou Messenger?

As ações se dividem em dois destinos (confirmado pela API dos binários, I-A3):

- **Amigos, presença e grupos → Nexon Messenger** (`nmconew.dll` no cliente,
  `NMLinkW.dll`/`NMServer` no servidor). `AddFriend`, `DeleteFriend`,
  `RequestNewFriend`/`ConfirmNewFriend`, `MoveFriend`, as categorias
  (`Move_Group` ↔ `Add/Delete/RenameCategory`), presença online e o estado
  "aguardando aceite" são do Messenger, não do protocolo TCP do jogo.
- **Sussurro, bloqueio e guilda → servidor do jogo.** Sussurro é o `TalkService`
  (`RequestWhisper`/`Whisper`/`WhisperFail`, SO1). Bloqueio é `WhisperBlock`/
  `WhisperFilter` (SO2) — flags `ChatPanelBlockUser`/`CommunityDialogBlockedUserList`
  ligadas. Guilda é o `HeroesGuildAPI` (procedures), com o chat de guilda podendo
  passar pela plataforma messenger (`NMLink_SendGuildChatMessage`,
  `CNMGuildOnlineInfoEvent`; flag `UseHeroesGuildChatServer` ligada,
  `GuildChatWcfService` vazio na config).

> As abas Amigos (aba 0) e a lista de online dependem do Messenger. Sem um
> Messenger ativo, a aba renderiza mas fica vazia/indisponível (string do cliente:
> *"Buddy List is temporarily unavailable…"* / *"Friend List tidak dapat diakses…"*).

---

## I-A2 — argumento de abertura do cliente e o que funciona

### `-nonmco` **não** está no cliente do jogo

`start.bat` do 1.69 EN (idêntico no `Client/` e na cópia de laboratório
`.cache/play-client`):

```
start vindictus.exe -stage -dev -lang en-eu-x-gm -noupdate -threads 4
```

Não há `-nonmco`. Ou seja, **o cliente do jogo sobe com o Messenger ligado**. O
`-nonmco` aparece só nos `DSAuxParams` do **servidor dedicado** (`srcds.exe`),
`-allowdebug -dev -nonmco -lang …` — correto: o processo de DS/PvP não deve abrir
messenger. (No servidor, `NGM_Disable=1` na `ServiceCore.dll.config` desliga o
Nexon Game Messenger do lado do serviço; isso é independente do cliente.)

### O que funcionava

Sem um servidor original em execução aqui, a resposta vem da arquitetura + de um
servidor privado comprovadamente jogável (Windictus 2023):

- **Lista de amigos / "Add Friend" do menu / aba de bloqueados funcionam quando há
  um NMServer configurado.** O `nmconew.dll` lê `nmconew.ini` e conecta em
  `ServerIp:ServerPort`. Valores encontrados:
  - 1.69 EN / `Client/` / laboratório: `192.168.50.227:47611`
  - **Windictus 2023 (servidor privado que o operador jogava): `windictus.ddns.net:47611`**
- O Windictus 2023 traz `NMService.exe`, `nmconew.dll`/`nmconew.ini` e `srcds.exe`
  — isto é, um servidor privado **real e jogável** serve amigos por um NMServer
  próprio em `:47611`, não pelo protocolo do jogo.
- Bloqueio e sussurro funcionam pelo próprio servidor do jogo (TalkService),
  independentemente do Messenger.

**Conclusão para D-S1:** a lista de amigos com presença/pedido/aceite é um
subsistema Messenger. Para tê-la, ou se roda um NMServer (há uma reimplementação
Go pronta no pacote, I-A3) e aponta-se `nmconew.ini` para ele, ou implementa-se só
o que não depende do Messenger (sussurro SO1, bloqueio SO2, chat de grupo SO3) e a
aba de amigos fica inerte. O caminho "Windictus" (NMServer em `:47611`) é o único
com prova de funcionar no cliente.

---

## I-A3 — inventário do `NMServer`

### Localização e instalação

- Executável: `ServidorVindictus/Server/NMServer.exe` (raiz da pasta `Server`).
- Ponte do lado do jogo: `ServidorVindictus/Server/zh-tw-x-gm/Bin/NMLinkW.dll`.
- **Não** é instalado como serviço pelo `InstallServices.bat` (esse `.bat` só
  instala `RCClientService`/`RCServerService` — "Remote Control", outra coisa). O
  `NMServer.exe` é um processo avulso, iniciado à parte.
- Não há `.ini`/`.config`/`.xml` próprio do NMServer ao lado dele; a configuração
  de endereço/porta fica **no cliente** (`nmconew.ini`).

### Natureza — é uma reimplementação, não o binário da Nexon

Strings do `NMServer.exe` revelam um **programa Go de terceiros**, não o servidor
original da Nexon:

- Caminhos de fonte: `F:\Willi Network\NMServer for MSAuth\` com
  `NMServer.go`, `NMCrypto.go`, `PacketHandlers.go`, `PacketReader.go`,
  `PacketWriter.go`, `PacketWrapper.go`, `BytesStringifier.go`.
- Runtime Go (`net.Listen`, `net/http`, `crypto/tls`, `ListenAndServe`).
- Símbolos `main.*`: `StartServer`, `connectionHandler`, `pserver`,
  `PacketHandler`/`PacketReader`/`PacketWriter`, `ReadPacket`/`SendPacket`/
  `ReadPackets`/`StartReadPacket`, `Encrypt`/`Decrypt`/`xor_table` (cifra XOR
  própria), e handlers `HandleNothing`, `HandleLogin`, `HandleLogin2`,
  `HandleLogin3` (login em 3 etapas), `mb2sb`.
- Autenticação externa: chama `https://www.nexon.net/api/v001/account/login`.

Ou seja: servidor TCP com protocolo de pacotes próprio (cifra XOR) e login em três
mensagens, validando a conta contra a API da nexon.net. "MSAuth" = o modo de auth
do messenger. Em contraste, o `NMLinkW.dll` é o **componente original da Nexon**
(PDB `MessengerApplication\NMLink\20110113`), usado pelo `ServiceCore` do jogo para
empurrar atualizações ao NMServer.

### Portas e protocolo

- **Porta de escuta do NMServer: `47611`** (TCP). É o `ServerPort` que o
  `nmconew.dll` lê de `nmconew.ini` em todos os clientes examinados. Em
  `main.StartServer` o endereço é montado em runtime
  (`strconv.Itoa(porta)` + `net.Listen`), então a porta não é uma string fixa no
  binário — vem do chamador/constante; o console imprime `Listening on <addr>`.
- Autenticação HTTPS de saída para `www.nexon.net` (API de login de conta).
- `NMLinkW.dll` (servidor do jogo → NMServer) registra `m_uServerPort`,
  `CNMLinkManager::MakeServerConfigConnection`, log em `C:\LOG\NMLink`.

### API exposta (o que o messenger faz)

`NMLinkW.dll` (servidor) exporta a superfície de amigos/guilda/presença:

```
NMLink_AddFriend            NMLink_RequestNewFriend     NMLink_ConfirmNewFriend
NMLink_DeleteFriend         NMLink_MoveFriend           NMLink_CheckFriend[_Block]
NMLink_AddCategory          NMLink_DeleteCategory       NMLink_RenameCategory
NMLink_CheckLogin[_Block]   NMLink_CheckUserLogin[_Block]  NMLink_IsSessionON
NMLink_RefreshUserInfo      NMLink_RefreshUserGuildList NMLink_NotifyNoteReceived
NMLink_ChangeLevel/NickName/NexonNickName/MetaData      NMLink_SendGuildChatMessage
NMLink_SendSystemMessage    NMLink_SetDefaultServer     NMLink_Init/Terminate
NMLink_RemoveVirtualFriend/RemoveVirtualUser            NMLink_IsSessionON
```

`nmconew.dll` (cliente) tem o lado espelhado, com eventos/funções:
`CNMGetFriendListFunc`, `CNMGetFriendInfoFunc`, `CNMRequestNewFriendFunc`,
`CNMConfirmNewFriendFunc`, `CNMBlockFriendFunc`, `CNMChangeFriendMemo[Ex]Func`,
`CNMAddFriendToCategoryFunc`, e eventos `CNM{Real,Virtual,}FriendInfoChangedEvent`,
`CNMRequestNewFriendEvent`, `CNMGuildOnlineInfoEvent`/`…ExEvent`. Strings de erro
("another game"/"different server"/"Banned List") mostram que é a plataforma
messenger compartilhada da Nexon (o PDB e a string "Counter-Strike Online"
confirmam o reaproveitamento entre jogos).

---

## Cadeia de arquitetura (amigos)

```
CommunityDialog.gfx  --fscommand("CommunityDialog", AddFriend/…)-->  client.dll
client.dll  ──>  nmconew.dll (+ nmcogame.dll, NMService.exe)        [cliente]
nmconew.dll  --TCP 47611 (nmconew.ini: ServerIp:ServerPort)-->  NMServer.exe
NMServer.exe  --HTTPS-->  www.nexon.net/api/v001/account/login     [auth]
                                   ▲
ServiceCore do jogo  ──NMLinkW.dll──┘  (AddFriend/presença/guilda/system msg)
```

Sussurro, bloqueio e guilda seguem o caminho normal do jogo (TalkService /
WhisperBlock / HeroesGuildAPI), **não** o Messenger.

## Impacto no roteiro

- **D-S1 (amigos):** três opções concretas —
  1. Só servidor do jogo: SO1 sussurro, SO2 bloqueio, SO3 chat de grupo; aba de
     amigos fica vazia (sem Messenger).
  2. Rodar o `NMServer.exe` (reimplementação Go, Willi Network) no host e apontar
     `nmconew.ini` para ele em `:47611`, como no Windictus 2023. Atenção: o binário
     só tem handlers de login (`HandleLogin`–`HandleLogin3`, `HandleNothing`),
     então aceita o login do messenger, mas não serve lista de amigos.
     Requer validar o protocolo de pacotes/handlers e a dependência de
     `nexon.net/account/login` (pode não servir para contas locais).
  3. Reimplementar o lado servidor do NMLink/NMServer em Rust (protocolo XOR +
     3 logins + API de amigos/presença/categorias) para integrar com as contas
     locais, sem depender da nexon.net.
- **SO4 (amigos pelo cliente):** implementar só o lado servidor que a UI usa —
  e a UI de amigos usa o Messenger, não o protocolo do jogo. Portanto SO4 só
  avança junto com a decisão D-S1 (opção 2 ou 3).
- `srcds.exe`/PvP: fora do escopo desta nota (I-P1–I-P3); o relay PvP é `27006`,
  `DSPort 26000`, `DSServicePort 27018` na `ServiceCore.dll.config`.

## Apêndice — reprodução

Scripts de leitura (scratchpad da sessão), rodados com `py -3.13 -I`:
- `gfx_as2.py <arquivo.gfx>` — extrai `ActionConstantPool`+`ActionPush` do AS2.
- `strings_scan.py <binário> [palavras… | ALL]` — strings ASCII+UTF-16 com filtro.
- REA: `open_binary` + `procedure_pseudo_code main.StartServer` no `NMServer.exe`.

Arquivos lidos (read-only): `CommunityDialog.gfx`, `RecommendFriendDialog.gfx`,
`NexonChatDialog.gfx`, `nmconew.dll`, `NMLinkW.dll`, `NMServer.exe`,
`nmconew.ini` (1.69EN/Client/play-client/Windictus 2023), `start.bat`,
`ServiceCore.dll.config`, `InstallServices.bat`.
