# Multijogador (B5)

Party, convites, navio, instância compartilhada, combate e loot sincronizados, saída e reconexão. Evidência: `PartyServiceCore`/`MicroPlayServiceCore._Party_` (Party, PartyMember, processadores), `FrontendServiceCore` (`BattleServiceProxy` e seus estados, `SendInvitation`, `UpdateParty`, `Kick`), `RelayServer` (`P2PGroup`) e o ProudNet do `engine.dll` (Ghidra headless).

## Arquitetura

- `domain::party`: o `Party.cs` original — slots (mestre no slot 0, 4 → 6 → 8 sem navio), convites, entrada NORMAL, remoção com `ProcessAfterRemoveMember` (reeleição, volta à cidade antes de `Starting`, dissolução), kick, transferência, `SetMemberState` e `IsAllReady`.
- `infrastructure::party_hub`: a autoridade única do processo. Cada personagem selecionado tem um assento com caixa de correio (`mpsc`, 256); toda mudança visível a outros jogadores passa por `broadcast` (PartyInfo e, com navio, UpdateShip para cada membro). Quem chama recebe os próprios pacotes em ordem; os demais membros recebem pela caixa de correio, que o laço da conexão (`lab_frontend`) despeja no socket.
- `infrastructure::minimum_party`: os estados do `BattleServiceProxy` fora da MicroPlay (Initial, PartyMaster, PartyMember): convite (abre a party no Initial), aceite, recusa, saída e kick.
- `minimum_mission::Mission` difunde pelo hub; `accept_quest` recebe a party do mestre (aberta pelo Initial ou já existente).

## Etapas

| Etapa | Conteúdo | Situação |
|---|---|---|
| MP1 | Party na cidade: PartyInvite/Invited, aceite, recusa, LeavePartyMessage, Kick, desconexão; AcceptQuest pelo mestre de uma party | Feito; cliente pendente (`docs/client-tests/B5-multijogador.md`) |
| MP2 | Relay com vários membros: MemberJoin para os membros antigos, NMInfos, quadros 20–22 → 23–25, MemberLeave (`docs/battle-transport.md`) | Feito; cliente no checkpoint do MP3 |
| MP3 | Navio: JoinShip do membro (AddPlayer, MemberInfo ao host, HostInfo + ConnectionRequest ao membro), Ready, StartGame com todos prontos | Feito; **checkpoint de cliente** (MP2+MP3) |
| MP4 | Combate e loot: EXP, drops, conclusão e recompensas para cada membro | Feito; cliente pendente |
| MP5 | Saída e reconexão de membros na missão, migração de host | Feito; cliente pendente |

## Decisões e limites (MP1)

- Mensagens novas no fio de missão, com fixtures do oráculo C#: PartyInvite (517), PartyInvited (516), PartyInvitationAccept (513), PartyInvitationReject (515), PartyInvitationRejected (514), Kick (470), JoinShip (469), HostInfo (467), Ready (553).
- QueryCIDByName compara os nomes decodificados sem diferenciar maiúsculas (colação do SQL Server), entre personagens com `status<>1`.
- O Kick vindo da party envia dois KickedMessage, como `KickProcessor` (LeaveParty + o próprio).
- Não migrados: assistência (ASSIST/FORCE_ASSIST), CombatRejoinSystem, captcha/punição antes do aceite, chat de party, TransferMaster pelo cliente, lista de navios de outras parties, diário de party (`AddPartyLedger`).

## Decisões e limites (MP3)

- O cliente do host simula a dungeon; o servidor não manda `GameStartGranted` aos membros. O membro recebe `HostInfoMessage` (GameInfo do RegisterServer do host, slot, chave) e o host recebe `MemberInfoMessage` do membro; o resto passa pelo P2P (relay, MP2).
- A MicroPlay compartilhada fica no hub (`party_hub::Play`: quest, opção, grupo do relay, host, HostInfo, jogadores por slot). Cada membro tem sua `Mission` (MicroPlayer), com `host = false`.
- As mensagens de MicroPlayHost (RegisterServer, StartGame, setores, mortes, drops, consumíveis, eventos) vindas de um membro são ignoradas, como no `MicroPlayMember` original.
- Saída do relay por membro (`Relay::leave`); o grupo fecha quando fica vazio, inclusive na missão solo.
- O membro cuja party volta à cidade (o host saiu do navio antes de começar) larga o navio local.
- Não migrados no MP3: JoinShip do Initial pela lista de navios (a lista continua vazia), assistência, QuestTimerInfo de reentrada, `IsRequiredItemForAll`, fadiga, HostRestarted (migração de host) e recompensas dos membros (MP4).

## Decisões e limites (MP4)

- Réplica por jogador: o frontend do host repassa às réplicas dos membros (aviso `Play` do hub) as mensagens de MicroPlayHost que todo MicroPlayer aplica — MoveToNextSector, MonsterKilled, RagdollKicked, CombatRecord, UseConsumable, PlayerKilled, PlayerRevived, MicroPlayEvent. No StartGame o host publica o instantâneo (setores e sequência sem núcleos; aviso `Started`), e cada réplica parte dele. Cada membro calcula assim a própria EXP, núcleos (sorte, classe e drops de história próprios), metas, morte, consumíveis e a recompensa final, que chega só ao próprio cliente.
- EXP de monstro: `MonsterKillExpCalculator` — peso `(nível/2+10)²` sobre a soma dos pesos da play e bônus de party 140/240/330/420 (1/2/3/4+ jogadores); a solo continua igual.
- Ergs (props) ficam no host: o vencedor é sorteado entre os jogadores (`PickRandomRewardingPlayer`) e o `PickErg` entrega ao vencedor (aviso `Give`); o host recebe o `PutErg` com o slot vencedor.
- Efeitos (reviver e afins) valem para qualquer slot da play; `ApplyMicroPlayEffects` vai só ao host, como no original. Cada réplica acompanha quem está morto.
- Limites: o rastreador de metas de cada réplica conta as metas individuais só do próprio jogador (o original mostrava também as dos outros); `GetItem` de outro jogador não aparece; destruição de props por um membro não conta para as metas de props desse membro.

## Decisões e limites (MP5)

- Saída de membro no meio da missão: desistência (FinishMicroPlay GiveUp) dá a recompensa de desistência na réplica do membro e o tira da party, da play e do grupo do relay; os demais recebem PartyInfo e MemberLeave.
- Migração de host (`MicroPlay.CheckHostDisconnected` → `StartHostElection` → `ChangeHost`): quando o host sai (desconexão ou desistência) de uma play iniciada e aberta, o hub elege o jogador de menor slot, que vira mestre no slot 0 (TransferMaster); todos recebem `HostRestarting`; a réplica do novo host vira a autoridade e recebe o próprio `MemberInfo` e `GameStartGranted` (setores atuais, `CurGameTime`, `NewSlot`, `NewKey`). O `RegisterServer` do novo host é o `HostRestarted`: cada outro jogador recebe `HostInfoMessage` e manda o próprio `MemberInfo` ao novo host. Sem `RegisterServer` em 120 s (`HOST_RESTART_TIMEOUT`), o navio afunda (`ReturnFromQuest(ToTown)` e saída para todos). Antes do início, o host saindo do navio devolve a party à cidade (MP3).
- Reentrada (CombatRejoinSystem, ligado pela versão do perfil): o membro desconectado de uma play aberta fica registrado (MicroPlayEntityInfo/DisconnectedMemberCID em memória, que equivale às linhas mais novas que o início do serviço); a lista de personagens mostra `LoginPartyState = 1`; `RequestJoinParty` > 0 faz JoinParty(REJOIN) no slot guardado, com `RejoinCombatSuccess`; o `JoinShip` seguinte entra na play com `QuestTimerInfo`, e a réplica parte do instantâneo atual do host (rastreador e pets iniciados). `RequestJoinParty` = 0 apaga o registro.
- Limites: o host que reentra volta como membro; a recompensa de quem desconectou e não voltou não é enviada por correio; `LeaveWithHost` (decisão de sair junto com o host) não migrada.
