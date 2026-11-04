# Sistemas sociais e competitivos (B5): amigos, guildas, ranking e PvP

Plano levantado em 03/11/2026 a partir do C# descompilado (`legacy/decompiled`: FrontendServiceCore, PlayerService, TalkService, GuildService, PvpService, RankService, DSService) e das flags EN-EU do `heroes.db3` do 1.69 (`CurrentVer` 3.09.00; uma flag vale quando `CurrentVer` ≥ valor, `ServiceCore/FeatureMatrix.cs:192-195`). Nada aqui está implementado. Os ids de categoria citados são os de `src/protocol/server_profile.rs`.

## Flags do perfil EN-EU

- Ligadas: `UseTalkService`, `GroupTalk` (limites nulos), `ChatPanelBlockUser`, `CommunityDialogBlockedUserList`, `RecommendFriend`, `CharacterMenu_AddFriend`, `InGameGuild`, `GuildHeroesCore_v1`, `UseHeroesGuildChatServer`, `EnableChangeGuildMaster`, `GuildLevel`, `NewbieGuildRecommend`, `GuildStorage`, `GuildStoargeSlotAllCash`, `Arena`, `Pvp_Launch`, `UI_PvPButton`, `DeathMatch`, `GodMatch`, `CombatFormula_PVP`, `PVPPartyCountLimit`, `PvpBanPenalty`, `CharacterRank`, `RankTitle`.
- Desligadas: `FriendshipSystem` (e as variantes de ponto/recompensa), `friend_loyalty_system`, `PvP` (modos parciais P2P), `PvP_Tournament`, `DSDynamicLoad`, `GuildWebChat`, `GuildScore`, `Rank_GuildPointRank`, `GuildStorageSecondPassworded`, `WhisperFilterMaxCount`.

## Constatações

- **Amigos:** o servidor não tem lista de amigos com presença, pedido ou aceite. FriendshipInfo identifica o amigo pelo NexonSN e está desligado no EU; a lista visível no cliente provavelmente vem do Nexon Messenger (`nmconew.dll`, `nmcogame.dll`, `NMService.exe` no cliente), nativo. O servidor trata sussurro (TalkService), bloqueio (WhisperFilter/`WhisperBlock`) e chat em grupo (GroupTalk).
- **Guildas:** ramo `HeroesGuildAPI` (stored procedures do heroes, síncronas), sem dependência nativa.
- **Ranking:** consultas e `RankBasisUpdate` existem; nenhuma procedure preenche `RankResultCharacter` — o cálculo era um job externo e será reconstruído.
- **PvP:** Arena (p05), DeathMatch (p03) e GodMatch (p04), os modos ligados no EU, exigem o servidor dedicado `srcds.exe` (Source, `bin/server.dll`, `bin/dedicated.dll`), lançado pelo DSService com `-console -connectip <ip> -connectport <porta do DSService> -port <DSPort+i> <DSAuxParams> +is_giant_raid <0|1> -nohltv -owner_pid <pid> -owner_dsid <id>` (`DSService/DSEntity.cs:282-288,1347-1353`; `DSAuxParams` do zh-tw-x-gm: `-allowdebug -dev -nonmco -lang <região>`). Os modos P2P (CTF p01, Survivor p02, FruitMatch p06) reaproveitariam o relay, mas estão desligados no EU. O DS também atende `is_giant_raid` (raides gigantes do PvE).
- **Trade:** o 1.69 não tem troca direta entre jogadores; o `TradeService` é o mercado já migrado (`docs/marketplace.md`).

## Etapas

| Etapa | Conteúdo | Depende de |
|---|---|---|
| SO1 | Sussurro: `RequestWhisper` (638) → `Whisper` (639) / `WhisperFail` (640, OffLine, NoCharacter, LogicalFail), entrega pelo CID via `party_hub` (ponto único de envio). **Feito 04/11** (`protocol::whisper`, `minimum_whisper`; fixtures do oráculo, 63 casos); cliente pendente (`docs/client-tests/B5-social.md`) | — |
| SO2 | Bloqueio: tabela `WhisperBlock` (FromCID, ToCID, BlockLevel), `UpdateWhisperFilter` (637), `WhisperFilterList` (636) no login, recusa de bloquear a si mesmo. **Feito 04/11** (`game_whisper_block`, 0061; recusa qualquer personagem da própria conta; filtro não aplicado na entrega, como o original); cliente pendente | SO1 |
| SO3 | Chat em grupo (GroupTalk do TalkService): salas, convite, mensagens | SO1; limites (I-S3) |
| SO4 | Amigos pelo cliente: o que a UI emite em "adicionar amigo" e na aba de amigos; implementar só o lado do servidor que o cliente usar | I-A1, I-A2, D-S1 |
| G1 | Guilda: criar (`OpenGuild` 338, regras de nome, `GuildResult` 281), `GuildInfo` 278, `GuildMemberList` 336, reconectar no login (`GuildServiceProxy`), BriefGuildInfo no personagem **Feito 04/11** (`protocol::guild`, `domain::guild`, `infrastructure::guild`, migração 0062; história `Guild_create` por GuildStoryRequest e apagada no OpenGuildSuccess; `has_guild` real no diálogo; QueryGuildList por nome e por mestre; presença online pelo `GuildHub`); cliente pendente | I-S1 |
| G2 | Entrar/aprovar/rejeitar/sair/expulsar/cargos (`JoinGuild` 284, `LeaveGuild` 285, `GuildOperation` 337), trocar mestre (84), dissolver só com 1 membro (277), notícia (280), busca (339 → 279) **Feito 04/11** (modelo de `Sync` do GuildEntity no `GuildHub`: GuildInfo, linha parcial, lista completa e GuildKicked pelo `party_hub`; o fechamento da conexão do jogador sai do chat, desconecta e sincroniza na hora, como o `PlayerConn.Closed` do original); cliente pendente | G1 |
| G3 | Chat de guilda (85/86/88) e presença dos membros **Feito 04/11** (GuildChatRoom de membros do jogo: JoinGuildChatRoom 88 com o aviso GuildChatMemberInfo 85 aos que já estão na sala, GuildChat 86 só para membros regulares na sala, falhas Unknown/NotEnoughPermission; chat web desligado no EU); cliente pendente | G1 |
| G4 | GP (AP × 100/100, teto diário 2000 PvE / 600 PvP, reset 07:00 UTC), nível até 20 (`GuildLevelUpInfo`), cápsula de AP por correio/QueuedItem, `GuildGainGP` 89, `GuildLevelUp` 90 **Feito 04/11** (GiveAPWithGuild no AP de recompensa da missão; GiveGP com teto diário, GuildGainGP 89 a todos online, GuildLevelUp 90 e cápsula de AP pelo QueuedItem `GuildLevelUp` processado na hora para quem está online; reset diário preguiçoso na leitura dos dados da guilda; `GuildLevelUpInfo`/`GuildLevelBonusInfo` no `item-economy.json`, features avaliadas pela política); cliente pendente | G2 |
| G5 | Bônus de nível (`GuildLevelBonusInfo`) nos pontos hoje fixos em zero (missão, mercado, reparo, refino, tintura, fogueira), `FailInGuild` na exclusão, renomear personagem, títulos de rank **Feito 04/11** (GuildLevelUpBonus: taxa do mercado, refino, reparo, EXP e AP de batalha, GameJoinMemberInfo.GuildLevelBonus; restrição RequiredGuildLevel com o nível de guilda carregado; FailInGuild para qualquer cargo; GuildId/GuildName no resumo dentro do jogo). Fora: DyePrice (tintura com ouro não migrada), VocationExp (vocação não migrada), AdvancedFeather (item recusado), GuildBonus1 de party da mesma guilda, CharacterCommonInfo ao mudar a guilda e renomear (não migrado); cliente pendente | G4 |
| G6 | Armazém de guilda (≥ 20 membros, ouro 2,5M/dia e 10M, slots por cash) | G4; I-S2 |
| RK1 | Registro de pontuação (`RankBasis`, `RankBasisUpdate`) nos ganchos já existentes (conclusão de missão `CharacterRank`, MDRSystem) **Feito 04/11** (`game_rank_basis`/`_history`, 0064; `RankDescription` no `item-economy.json`, features pela política; regra do RankBasisUpdate: Count soma, Time guarda o melhor do dia UTC e o primeiro do dia novo vai ao histórico; gancho da missão: GamePlay_complete_battle e Solo/PartyTimeAttack_<quest> em ms). Fora: os outros RankIDs (GetItem_, UseItem_, nível, AP, títulos, refino/encanto/tintura, produção, pesca, mineração, partes, reanimar, carruagem) e GamePlay_help_beginner; sem efeito no cliente até o RK2 | — |
| RK2 | Cálculo diário/semanal/mensal de `RankResultCharacter` e consultas `QueryRank*` (382–388, 147, 333), favoritos (396) **Feito 04/11** (D-S3: publicação às 07:00 UTC em `game_rank_result`; consultas 382–388, alerta 333, favoritos 384/386/396, meta 385 sem rival como o stub original; banner aleatório 147 responde vazio, sem as colunas de banner exportadas); cliente pendente | RK1 |
| RK3 | Títulos de rank (`RankTitle`) **Feito 04/11** (lista semanal dentro de `RankTitleLimit` carregada na subida e 3 s após cada sábado 07:00 UTC; títulos em memória dados na entrada e nos processos diários 49/67 do EU; SelectTitle 565 migrado junto; os 12 títulos do EU são de PvP, sem efeito até PV2–PV4); cliente pendente | RK2 |
| PV1 | Duelo livre (`FreeMatchReport` 276): vitória/derrota no personagem | — |
| PV2 | Fila e canais: `PvpQueryChannelList` 130 → `PvpChannelList` 129, `PvpRegister` 442/`PvpUnregister` 611 → `PvpRegisterResult` 378, `PvpConfirmJoin` 143, trava da party | D-S2 |
| PV3 | Partida: relay PvP (27006 no original), `PvpGameHosted` 435/`PvpHostInfo` 436, `PvpMemberInfo` 437, `PvpTeamInfo` 380, `PvpReport` 379, `PvpCommand` 376, `PvpKicked` 377 | PV2; I-P1–I-P3 se for DS |
| PV4 | Resultado: selos (`honor_stamp`), `PvpResultInfo` (Win, Lose, MaxSuccessiveWin), `ArenaResultInfo` 647, GP de guilda, ranking `Pvp_<modo>`, penalidade (`PvpPenalty` 648) | PV3, G4, RK1 |

Ordem: SO1 → SO2 → G1–G5 → RK1–RK3 → PV1 → (decisão D-S2) PV2–PV4 → G6, SO3, SO4 conforme as investigações.

## Desvios deliberados (guildas)

- ChangeMaster exige que quem pede seja o mestre: o original passa o próprio nome como mestre antigo sem conferir o cargo, e a procedure rebaixa o dono real; a recusa responde `ChangeMaster_NotMatchOldMaster`.
- JoinGuild recusa guilda fechada com `GuildNotInitialize`: no original a entidade carrega a guilda fechada (GuildGetInfoBySn não filtra `dateClosed`) e o personagem entraria numa guilda dissolvida.

## Decisões do operador

- D-S1: amigos — sem amigos (só SO1–SO3), ou Messenger próprio: `nmcogame.dll` substituto em Rust + módulo `messenger` no servidor (recomendado, `docs/nexon-services-replacement.md`), ou servidor compatível com o `nmconew.dll`.
- D-S2: PvP — partida hospedada por um jogador via relay (experimento recomendado, sem DS), lançador próprio do DS (`DedicatedMain` do `bin/dedicated.dll`, sem o `srcds.exe`) numa VM/host Windows, ou ficar no duelo livre por enquanto.
- D-S3: **decidido em 04/11 — por período.** Todo dia às 07:00 UTC (e na subida do servidor, se a última publicação for anterior à fronteira mais recente) o servidor publica três resultados por EventID ligado: diário = o dia que terminou (07:00 → 07:00), semanal = a semana até agora desde o último sábado 07:00 UTC (`RankPeriodInfo` 2011-11-19 07:00, 10080 min), mensal = o mês até agora desde o dia 1 às 07:00 UTC. Tempo (ScoreType Time, ABSOLUTE): o melhor valor de RankBasis ∪ RankBasis_History com LastUpdate na janela (SortType decide menor ou maior). Contagem (RELATIVE): o ganho na janela, pelo acumulado atual menos o instantâneo do acumulado guardado na fronteira de início (sem instantâneo, 0). Ordem pelo SortType; `n4Order` = ROW_NUMBER, `n4Rank` = RANK; `n4Rank_Prev` = posição na publicação anterior do mesmo período (0 sem posição); só personagens com Status 0; as consultas mostram os 100 primeiros.

## Investigações

### Com o operador (no original)

- I-A1/I-A2/I-A3: **resolvidas** por REA + parse dos `.gfx` (03/11/2026, [`friends-nexon-messenger-investigation.md`](friends-nexon-messenger-investigation.md)). Em resumo: a UI usa `fscommand("CommunityDialog", …)` (abas Amigos/Guilda/Bloqueados/Sussurro/Histórico); amigos/presença/grupos são do **Nexon Messenger** (`nmconew.dll` ↔ `NMServer` em `:47611`, config em `nmconew.ini`), enquanto sussurro/bloqueio/guilda são do servidor do jogo; o cliente sobe sem `-nonmco` (só o `srcds.exe` o usa); o `NMServer.exe` do pacote é uma reimplementação Go (Willi Network) que autentica em `nexon.net`. D-S1 passa a ter 3 opções (ver doc).
- I-P1/I-P2/I-P3: **resolvidas** por REA + leitura (03/11/2026, [`pvp-srcds-investigation.md`](pvp-srcds-investigation.md)). Em resumo: I-P3 — não há `ServiceCore.dll.config` de en-EU/en-US (só o `zh-tw-x-gm`): `DSPort 26000`, `DSServicePort 27018`, `ClientDSServicePort 28018`, `DSAuxParams -allowdebug -dev -nonmco -lang <região>`, relay de PvP TCP `27006` (UDP 27016/27006). I-P2 — UI mostra o botão de PvP (`UI_PvPButton`) com Arena (p05), DeathMatch (p03) e GodMatch (p04); mapas no HFS p02–p07 (p01 ausente); UI = `ArenaDialog.gfx`. I-P1 — `srcds.exe` é lançador do Source empacotado com VMProtect, que embute o Nexon Game Security e chama `gamelog.heroes.nexon.com`; **execução declinada** (risco de anticheat/sistema), comportamento previsto no doc; D-S2 segue do operador.

### Avaliação das investigações (03/11/2026)

- O `NMServer.exe` (Go) só tem os handlers `HandleLogin`, `HandleLogin2`, `HandleLogin3` e `HandleNothing` (27 símbolos `main.*`, nenhum de amigo, categoria ou presença). Ele aceita o login do messenger e mais nada: a opção 2 do D-S1 não entrega lista de amigos. Para amigos, só a opção 3 (Messenger em Rust), que exige o protocolo do `nmconew.dll` além do login.
- O `srcds.exe` do Windictus 2023 é outro build (1.034.312 bytes contra 843.536 do 1.69, ainda com `.vmp0`) **sem** a string do Nexon Game Security, e mantém `gamelog.heroes`. É indício de que o servidor privado usou um DS sem o NGS.
- O botão e os modos de PvP (I-P2) vêm da flag `UI_PvPButton` e dos `.gfx`, não de observação no cliente.
- I-P3 deu só as portas; o formato das mensagens DSService ↔ DS continua no I-S4.

### Respostas do operador (04/11/2026)

- I-A4: no Windictus 2023 a lista de amigos funcionava (o Messenger deles ia além do login do `NMServer.exe` Go do pacote).
- I-P4: provável que a Arena abrisse partida (havia quem farmasse PvP), sem certeza.
- I-P5: recusado. Nenhum binário legado será executado; o DS só avança por engenharia reversa estática primeiro.

### Do lado do Claude (sem o operador)

- I-S1: **guilda resolvida em 04/11** sem subir o SQL Server: `docs/evidence/database/study_heroes.schema.json` (`sql_modules.definition`) tem as 13 tabelas e as 53 procedures de guilda (GuildCreate, GuildUserJoin/JoinApply/JoinReject/Secede/TypeModify, GuildGroupChangeMaster, GuildRemove, GuildAllMemberGetList, GuildUserGroupGetList, GuildCheckGroupName/ID, Get/UpdateGuildInfo…). Ranking e `PvpResultInfo` seguem pelo mesmo caminho (`study_heroesRank`, `study_heroes`); amizade não é necessária (D-S1).
- I-S2: categoria das mensagens de armazém/newbie/compra direta de guilda (`candidate_category: 0` em `docs/evidence/reverse/messages.json`) e do `PvpRegisterGameWaitingQueue`.
- I-S3: limites do GroupTalk (flags nulas no EU) e se o cliente 1.69 abre salas de grupo.
- I-S4: protocolo TCP DSService ↔ `srcds` (`DSHostConnectionEstablish`, `LaunchPvpDS`, ping, encaminhamento de Report/Penalty/Kick) a partir do `DSEntity.cs`, com fixtures do oráculo C#.
- I-S5: emissores de `PvpStartGame`/`PvpEndGame`/`PvpInfo` (438/434/142), não achados no C#.
