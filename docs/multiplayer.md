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
| MP2 | Relay com vários membros: MemberJoin para os membros antigos, NMInfos, quadros 20–22 → 23–25, saída do grupo | Pendente |
| MP3 | Navio: JoinShip do membro (AddPlayer, MemberInfo ao host, HostInfo + ConnectionRequest ao membro), Ready, StartGame com todos prontos | Pendente |
| MP4 | Combate e loot: EXP, drops, conclusão e recompensas para cada membro | Pendente (depois do checkpoint de cliente do MP3) |
| MP5 | Saída e reconexão de membros na missão, reeleição do host | Pendente |

## Decisões e limites (MP1)

- Mensagens novas no fio de missão, com fixtures do oráculo C#: PartyInvite (517), PartyInvited (516), PartyInvitationAccept (513), PartyInvitationReject (515), PartyInvitationRejected (514), Kick (470), JoinShip (469), HostInfo (467), Ready (553).
- QueryCIDByName compara os nomes decodificados sem diferenciar maiúsculas (colação do SQL Server), entre personagens com `status<>1`.
- O Kick vindo da party envia dois KickedMessage, como `KickProcessor` (LeaveParty + o próprio).
- Não migrados: assistência (ASSIST/FORCE_ASSIST), CombatRejoinSystem, captcha/punição antes do aceite, chat de party, TransferMaster pelo cliente, lista de navios de outras parties, diário de party (`AddPartyLedger`).
