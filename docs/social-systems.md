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
| SO1 | Sussurro: `RequestWhisper` (638) → `Whisper` (639) / `WhisperFail` (640, OffLine, NoCharacter, LogicalFail), entrega pelo CID via `party_hub` (ponto único de envio) | — |
| SO2 | Bloqueio: tabela `WhisperBlock` (FromCID, ToCID, BlockLevel), `UpdateWhisperFilter` (637), `WhisperFilterList` (636) no login, recusa de bloquear a si mesmo | SO1 |
| SO3 | Chat em grupo (GroupTalk do TalkService): salas, convite, mensagens | SO1; limites (I-S3) |
| SO4 | Amigos pelo cliente: o que a UI emite em "adicionar amigo" e na aba de amigos; implementar só o lado do servidor que o cliente usar | I-A1, I-A2, D-S1 |
| G1 | Guilda: criar (`OpenGuild` 338, regras de nome, `GuildResult` 281), `GuildInfo` 278, `GuildMemberList` 336, reconectar no login (`GuildServiceProxy`), BriefGuildInfo no personagem | I-S1 |
| G2 | Entrar/aprovar/rejeitar/sair/expulsar/cargos (`JoinGuild` 284, `LeaveGuild` 285, `GuildOperation` 337), trocar mestre (84), dissolver só com 1 membro (277), notícia (280), busca (339 → 279) | G1 |
| G3 | Chat de guilda (85/86/88) e presença dos membros | G1 |
| G4 | GP (AP × 100/100, teto diário 2000 PvE / 600 PvP, reset 07:00 UTC), nível até 20 (`GuildLevelUpInfo`), cápsula de AP por correio/QueuedItem, `GuildGainGP` 89, `GuildLevelUp` 90 | G2 |
| G5 | Bônus de nível (`GuildLevelBonusInfo`) nos pontos hoje fixos em zero (missão, mercado, reparo, refino, tintura, fogueira), `FailInGuild` na exclusão, renomear personagem, títulos de rank | G4 |
| G6 | Armazém de guilda (≥ 20 membros, ouro 2,5M/dia e 10M, slots por cash) | G4; I-S2 |
| RK1 | Registro de pontuação (`RankBasis`, `RankBasisUpdate`) nos ganchos já existentes (conclusão de missão `CharacterRank`, MDRSystem) | — |
| RK2 | Cálculo diário/semanal/mensal de `RankResultCharacter` e consultas `QueryRank*` (382–388, 147, 333), favoritos (396) | RK1; D-S3 |
| RK3 | Títulos de rank (`RankTitle`) | RK2 |
| PV1 | Duelo livre (`FreeMatchReport` 276): vitória/derrota no personagem | — |
| PV2 | Fila e canais: `PvpQueryChannelList` 130 → `PvpChannelList` 129, `PvpRegister` 442/`PvpUnregister` 611 → `PvpRegisterResult` 378, `PvpConfirmJoin` 143, trava da party | D-S2 |
| PV3 | Partida: relay PvP (27006 no original), `PvpGameHosted` 435/`PvpHostInfo` 436, `PvpMemberInfo` 437, `PvpTeamInfo` 380, `PvpReport` 379, `PvpCommand` 376, `PvpKicked` 377 | PV2; I-P1–I-P3 se for DS |
| PV4 | Resultado: selos (`honor_stamp`), `PvpResultInfo` (Win, Lose, MaxSuccessiveWin), `ArenaResultInfo` 647, GP de guilda, ranking `Pvp_<modo>`, penalidade (`PvpPenalty` 648) | PV3, G4, RK1 |

Ordem: SO1 → SO2 → G1–G5 → RK1–RK3 → PV1 → (decisão D-S2) PV2–PV4 → G6, SO3, SO4 conforme as investigações.

## Decisões do operador

- D-S1: amigos — só o lado do servidor que o cliente usar (sem emular o Nexon Messenger) ou outra abordagem depois de I-A1/I-A2.
- D-S2: PvP — rodar o `srcds.exe` como processo no host Windows (fora do Docker Linux), mudar o perfil para os modos P2P, ou ficar no duelo livre por enquanto.
- D-S3: ranking — fórmula e periodicidade do cálculo (a regra original não está nos binários).

## Investigações

### Com o operador (no original)

- I-A1: `en-EU/scaleform/CommunityDialog.gfx` (e outros `.gfx` com "Friend"/"Community"): decompilar com o JPEXS FFDec e trazer as abas, e os nomes chamados em `ExternalInterface.call`/`fscommand` e os callbacks registrados (o que a UI pede ao jogo em amigos, bloqueio, grupo, guilda).
- I-A2: no servidor original em execução (se houver acesso, ou relato de quem jogou): a lista de amigos, o "Add Friend" do menu do personagem e a aba de bloqueados funcionavam? Com que argumento o cliente era aberto (`-nonmco`?).
- I-A3: o executável e a configuração do `NMServer` no pacote `ServidorVindictus` (onde está, que portas e serviços expõe); só o inventário, sem executar.
- I-P1: `srcds.exe` numa cópia isolada do cliente, com os argumentos acima e `-nonmco`, sem servidor ouvindo: inicia? o que o console mostra (mapa, erros de Steam/Nexon, tentativa de conexão em `-connectip`)? Executar um binário legado é decisão do operador.
- I-P2: os mapas p01–p06 no HFS do 1.69 EN (nomes de `.bsp` de PvP/arena) e se a UI do 1.69 EN mostra o botão de PvP e quais modos.
- I-P3: `ServidorVindictus` en-EU/en-US: `DSExecPath`, `DSPort`, `DSAuxParams`, portas do PvP relay e do DSService no `ServiceCore.dll.config`.

### Do lado do Claude (sem o operador)

- I-S1: corpos das procedures de guilda, amizade, `WhisperBlock`, `PvpResultInfo` e ranking no SQL Server legado restaurado (`docker compose --profile legacy up -d --wait legacy-sqlserver`, só leitura; nada de catálogo bruto no git).
- I-S2: categoria das mensagens de armazém/newbie/compra direta de guilda (`candidate_category: 0` em `docs/evidence/reverse/messages.json`) e do `PvpRegisterGameWaitingQueue`.
- I-S3: limites do GroupTalk (flags nulas no EU) e se o cliente 1.69 abre salas de grupo.
- I-S4: protocolo TCP DSService ↔ `srcds` (`DSHostConnectionEstablish`, `LaunchPvpDS`, ping, encaminhamento de Report/Penalty/Kick) a partir do `DSEntity.cs`, com fixtures do oráculo C#.
- I-S5: emissores de `PvpStartGame`/`PvpEndGame`/`PvpInfo` (438/434/142), não achados no C#.
