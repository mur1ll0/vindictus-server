# Migração Vindictus .NET → Rust — roadmap vivo

Atualizado em 03/11/2026. Este arquivo é a memória de trabalho do projeto: o que já foi feito (checklist curto) e o que falta fazer. Os registros detalhados de cada sessão (relatos, causas, comandos, logs, hashes) ficam em `docs/history/session-log.md`, que só recebe acréscimos e não deve ser lido inteiro (ver `AGENTS.md`). A ordem de trabalho e as regras dos blocos estão em `docs/playable-game-roadmap.md`.

## Objetivo e critério de conclusão

Migrar os serviços antigos e as regras de jogo para Rust, executáveis em Docker, compatíveis com o cliente legado 1.69 EN. Conclusão exige login pelo jogo, personagem, inventário, lobby, batalha, recompensa, reconexão e persistência verificados no cliente. Infraestrutura saudável e C# descompilado não significam migração concluída.

Migração completa: **NÃO CONCLUÍDA**.

## Estado atual

- Blocos B0–B4 fechados no código (último gate completo: B4, 27/10/2026 — 524 Rust, 298 PostgreSQL em lotes, 61 Python, smoke Docker exit 0). Só o B0 tem aceite completo no cliente.
- Bloco atual: **B5** (multijogador e sistemas restantes). Pets (P1–P6) adiantados a pedido do operador, sem teste no cliente.
- Trilha paralela de classes novas: Delia (classe 10) e Miri (classe 11, conteúdo do NA) jogáveis e criáveis pela grade (`docs/future/miri-class-port.md`).
- Laboratório: `docker compose` (`game`, `server`, PostgreSQL) e cópia do cliente em `.cache/play-client` (`start-lab.bat`, conta `lab169_a`).

## Próxima ação retomável

1. Rebuild do Docker (`COMPOSE_PARALLEL_LIMIT=1 docker compose build`, depois `docker compose up -d --wait`) e sessão de cliente do operador pelo roteiro `docs/client-tests/B5-pets.md` (checkpoints P3 e P6).
2. Miri: aceita no cliente em 02/11/2026 (combate, grade e customização MG1–MG4, teclas da Delia regravadas MI45; `docs/future/miri-class-port.md`); branch `feat/miri-class` no `main`. Pendentes no cliente: B4 I3 (modelo substituto pelo painel, MG6), MI1–MI3 e MI30–MI36 sem relato isolado.
   - B2 RL1 (do ramo do `engine.dll`): rodar no cliente (barco sem trava; `engine.dll` do laboratório já com o patch).
3. Launcher: login pelo launcher aceito (LA0, LA2, LA4, LA5). Próximo: D-L6 (só ticket no jogo; modo por nome no perfil de desenvolvimento), depois L1 (`crates/client-dist`, serviço `updates`, importação da v1 = 1.69 + o necessário, zstd adaptativo + `hfs-xor`), receitas v1–v3 (L2) e instalar/atualizar no launcher (L3).
4. B5 multijogador implementado (MP1–MP7): sessão de cliente do operador pelo roteiro `docs/client-tests/B5-multijogador.md` (duas contas, dois clientes; Docker já reconstruído). Depois, os sistemas restantes do B5.
5. B5 correio implementado em 03/11 (ciclo completo e GiveItem ForceMail): sessão de cliente pelo roteiro `docs/client-tests/B5-correio.md` (duas personagens; Docker reconstruído).
6. B5 mercado implementado em 03/11: sessão de cliente pelo roteiro `docs/client-tests/B5-marketplace.md` (duas contas; Docker reconstruído). Depois, o próximo sistema restante do B5.
7. B5 eventos: drops de chefe, BoostEvent, histórias por período, lojas de cupom, metas coletivas, agenda, descrição no painel e collabs SAO/Fairy Tail completos para EU feitos em 03/11; sessão de cliente pelo roteiro `docs/client-tests/B5-eventos.md` (EV1–EV12).
8. B5 social/PvP (`docs/social-systems.md`): SO1, SO2, G1–G5, RK1–RK3 (ranking por período D-S3, títulos de rank, SelectTitle) feitos em 04/11, PV1 (duelo livre), G6a–G6b (armazém abre no cliente: I-S2 confirmada para o armazém), G6e (compra de slots; flags de slot na aba Ajustes de jogo → Guilda) e a carruagem em 05/11. Próximo: G6c (depositar/retirar itens e ouro), G6d, G6f; PV2–PV4 aguardam D-S2. Faltam I-S3–I-S5 e a I-S2 das mensagens de newbie/compra direta.
   - Antes do gate do B5: 10 testes PostgreSQL já falhavam antes desta trilha (guardas de dono/perfil com a classe 10 e upgrades de esquema parados na migração 51 do launcher) e 10 achados de clippy anteriores; a suíte PostgreSQL só roda em lotes (o tmpfs enche).

## Aguardando o operador

| Roteiro | Situação |
|---|---|
| `docs/client-tests/B1-cidade.md` + `B2-fases.md` | Nenhuma entrada testada formalmente (missões já jogadas em sessões de diagnóstico de 22/10). RL1 (barco repetido sem trava, 31/10; patch `patch_engine_iocp_sign.py` já aplicado). |
| `docs/client-tests/B3-equipamento.md` | Nenhuma entrada testada (inclui DY1, ampola de tintura). |
| `docs/client-tests/B4-painel.md` | B4.2-L aceito; demais pendentes (inclui I2 editor de itens, I3 modelo substituto, P3 progressão, S1–S3 exclusão de personagem). |
| `docs/client-tests/B5-pets.md` | P1–P9 pendentes (P9: pets no Depot, 30/10). |
| `docs/client-tests/B5-multijogador.md` | MP1–MP7 pendentes (sessão única); requer duas contas e dois clientes (três para MP6). |
| `docs/client-tests/B5-correio.md` | CO1–CO11 pendentes; duas personagens (A nível ≥ 10). |
| `docs/client-tests/B5-marketplace.md` | MK1–MK11 pendentes; duas contas (o ouro é da conta). |
| `docs/client-tests/B5-eventos.md` | EV1–EV12 pendentes (chaves do painel, drops de chefe, BoostEvent, histórias por período, loja de cupom, descrição, meta coletiva, agenda, collabs SAO/Fairy Tail). |
| `docs/client-tests/B5-social.md` | SO1, SO2, G1–G5, RK1–RK3, PV1, G6b (resto), G6e e carruagem pendentes (sussurro, bloqueio, guilda, chat, GP/nível, bônus, ranking, títulos, duelo, armazém e compra de slots; duas ou três contas). |
| `docs/client-tests/C2-delia.md` | Aceito (D0–D23); D6 passou para o C3. |
| `docs/client-tests/C4-miri.md` | Aceito em 02/11 (combate, MG1–MG4, MI44, MI45); sem relato isolado MI1–MI3 e MI30–MI36. |
| `docs/client-tests/L-launcher.md` | LA0, LA2, LA4 e LA5 aceitos; LA3 coberto pelo LA4/LA5; LA1 dispensável. |
| `docs/client-tests/C3-criacao.md` | Grade, customização, Create → estalagem → cidade aceitos em 30/10; faltam volta à seleção com a personagem criada pela grade, G3 e G5. |

Testes de cliente adiados do B0: aprender skill nova com ouro (B1.2), empilhar pilhas da mesma classe (B1.4 ou B2.6), usar consumível pela barra (B2.5).

Decisões pendentes:
- Launcher (`docs/future/game-launcher.md` §5): D-L5 assinatura do exe (D-L2, D-L3, D-L4 e D-L6 decididos em 03/11).
- Social/PvP (`docs/social-systems.md`, `docs/nexon-services-replacement.md`): D-S1 amigos (Messenger próprio recomendado), D-S2 PvP (host jogador recomendado, ou lançador próprio do DS); I-A4, I-P4 e I-P5 opcionais do operador.
- Customização: poses animadas e rotação dos atores da grade (opcional, comando `cc_create_character_selection_play_sequence` do NA).

## Blocos do roteiro

- [x] **B0** — personagem na cidade: inventário, equipamento, mover/empilhar, quickslots, AP/cápsula, skills. Aceito no cliente em 21/10/2026.
- [x] **B1** — cidade: ouro/GiveItem, LearnNewSkill, NPCs/diálogos, lojas (compra, venda, recompra, reparo), histórias e quests de cidade, usáveis. Gate 21/10/2026. Cliente pendente.
- [x] **B2** — fases e combate: EXP/level up, entrada em missão, relay ProudNet (27005), dungeons/setores, monstros, combate, drops, conclusão/recompensas, desistência/desconexão. Gate 22/10/2026. Cliente pendente (primeira missão completa já jogada em 22/10).
- [x] **B3** — atributos de item, Craft, Enhance, Enchant, slots, reparo completo, armazenamento, SkillEnhance, caixas RandomItem/V2, cash shop (CS1–CS10), VIP/VVIP, Sort, fontes termais. Gate 24/10/2026. Item 5 adiado. Cliente pendente.
- [x] **B4** — painel de administração (contas, personagens, itens, progressão, lojas, cash shop, ajustes ao vivo, conteúdo versionado com manutenção, mapas/canais), login do jogo por login/senha, nome livre, criação das 10 classes, relatório de extensões. Gate 27/10/2026. Cliente parcial.
- [ ] **B5** — multijogador e sistemas restantes (em andamento).

### B2 — pendências conhecidas (recusadas ou ignoradas sem sucesso falso)

- Entrada e grupo: só missão solo. Recusadas: pesca, caça, torre, missões com item obrigatório ou limite de equipamento; moedas e pedras de bênção vazias; ReturnToShip (reiniciar) e CombatRejoinSystem inexistentes.
- Setores: sem AntiSectorSkip e sem diálogos de console de setor.
- Drops: baú do tesouro, mapas do tesouro, WeakPoint, HugeLucky (núcleos de história no B2 e de evento dos chefes no B5).
- Combate: MDRSystem só marcaria rankings (inexistentes); caixas alquímicas na barra recusadas.
- Conclusão: recompensas de troca de item (`slayer_stamp_limit>slayer_stamp`), CharacterRank, bônus de status/cafe/guilda/moedas/pedras, AddMicroPlayTime, contagem diária de quests grátis, OnEnterPhase das histórias; objetivo do sapo com regex não compilável.
- Progressão: AddGrowthLog, efeitos de guilda, títulos de rank; `take_item` de peça vestida recusado.
- Histórias: `select_vocation`, `complete_episode`/`give_title_by_episode`.
- Robustez: itens reservados de quem sai da missão vão para QueuedItem no original (StoreToQueuedItem não migrado; o QueuedItemController processa só linhas sem ItemID, B5 eventos).

### B3 — pendências

- [ ] Item 5: Combine Craft e habilidades de item (`PS_<n>`, `Gear.ToAbilityInfo`, `AbilityBonus` de prop e de prop nomeado, anúncio `GameUI_Heroes_ItemAbility_Enable_Announce`).
- [ ] Tingir peça com a ampola colorida (`DyeItemCashMessage`).

### B5 — o que falta

- [ ] Multijogador (`docs/multiplayer.md`, roteiro `docs/client-tests/B5-multijogador.md`): MP1 party na cidade, MP2 relay, MP3 navio/JoinShip e MP1–MP8 feitos (party, relay, navio, combate/loot por jogador com avisos para todos, saída/migração de host/reentrada inclusive do host, sair com o host, lista e navio de cidade, chat e pedido de professor); sessão de cliente única pendente.
- [ ] Sussurro e bloqueio (SO1/SO2 feitos 04/11), chat em grupo e amigos (SO3–SO4, `docs/social-systems.md`); amigos pelo Messenger próprio (D-S1).
- [ ] Guildas (G1–G5 feitos 04/11; G6 armazém, `docs/social-systems.md`).
- [ ] Ranking (RK1–RK3 feitos 04/11) e PvP/arena (PV1 feito 05/11; PV2–PV4, `docs/social-systems.md`); modos do EU exigem o `srcds.exe` (D-S2).
- Trade direto não existe no 1.69 (o `TradeService` é o mercado, já feito).
- [x] Correio (03/11/2026): listar, ler, enviar, retirar, apagar, devolver, avisos, chegada/expiração e GiveItem ForceMail nas origens migradas (`docs/mail-reference.md`); cliente pendente. Correio de eventos e amizade depende desses sistemas.
- [ ] Eventos/UserCare: chaves de evento ligáveis no painel (extensão, 03/11; `docs/content-tuning.md`); drops de evento dos chefes (EventDropItemInfo, EV5), recompensas periódicas do BoostEvent (PeriodicEventInfo, EV6), histórias por período (StoryLinePeriodInfo, EV7) e lojas de cupom com limite de compra (CouponShopItemInfo, ShopBuyRestrict, EV8), 03/11; descrição de cada evento no painel (EV9); metas coletivas AllUserGoal com as janelas deslocadas para a publicação da chave (decisão de 03/11), entrega pelo QueuedItem (EV10); agenda de eventos com os 370 modelos EventTemplate, chaves `Chave[valor]` e `droptable mod/reset` (decisão de 03/11, EV11); collabs SAO e Fairy Tail completos para EU (filtros e pacotes no painel, chaves só de NPC, textos em inglês, roupas para todas as classes; EV12; cartela de bingo não migrada). Sem efeito: `freetoken` e `droptable` de pesca.
- [x] Pets P1–P6 (30/10/2026; cliente pendente; detalhes em `docs/future/pet-system.md`).
- [ ] Pets P7 (pets do NA, junto com a importação das classes) e pet de outro jogador (multijogador).
- [ ] Títulos/achievements restantes (títulos adquiridos e por ação de combate já feitos no B2).
- [x] Leilão/mercado (03/11/2026): registrar com taxa, buscar (categoria, nome, filtros, ordem, páginas), comprar parcial, cancelar, expirar e ouro/itens pelo correio; listagens automáticas do servidor no painel (extensão, Ajustes de jogo) (`docs/marketplace.md`); cliente pendente.
- [ ] Cash shop e cosméticos restantes (Depot já funciona).
- [ ] Paridade e aceite final (M23/M24).

## Trilhas paralelas

- Classes novas (`docs/future/new-classes.md`, `docs/future/delia-class-port.md`, skill `new-class-port`):
  - [x] Delia C2: classe 10 jogável (corpo, armas, guarda, condições, efeitos, Rose Crush, SP), aceita em 29/10.
  - [ ] Delia C3: criação pela grade do NA — falta volta à seleção, G3 e G5.
  - [ ] Miri: M0–M5 feitos; M7a–M7d no cliente (arma, estados, `_ex`, combate básico); M7e (pulos, descanso) e M7f (corrente) aplicados, MI9/MI10 pendentes; mecânicas aceitas no cliente em 02/11; grade MG1/MG2 jogadas, MG3 e MG4 aplicados (`docs/future/miri-creation-grid.md`), MG3/MG4 do C4 pendentes; falta o aceite; decisões de 30/10 (assets do NA, todas as mecânicas, `weapon_greatsword`, modelos v44).
- Recursos futuros do painel (`docs/playable-game-roadmap.md`, seção "Recursos futuros do painel"):
  - [x] Editor de itens (corte 1 aceito; I2 no cliente pendente; modelo substituto de item existente, I3 pendente).
  - [x] Busca por categoria, editor de refino/scrolls, progressão até um nível (inclui história principal; P3 no cliente pendente).
  - [x] Seleção de personagem: exclusão, cancelamento e ordem dos slots (S1–S3 no cliente pendentes).
  - [ ] Launcher do jogo: L0 feito; plano em `docs/future/game-launcher.md` §4–7 (versões 1 base/2 Delia/3 Miri, Windows 7, status do Docker). L-A: login direto para a seleção por `patch_client_launcher_login.py` (`-launcher_ticket`), LA0/LA2 aceitos. L4 adiantado: sessão do launcher (login e senha até fechar o launcher) e ticket de uso único, `scripts/dev/play.ps1`; LA3 pendente. Armazenamento decidido em 02/11 (`docs/future/client-distribution-storage.md`: v3 ≈ 10,96 GB de download, v2→v3 ≈ 1,82 GB, servidor ≈ 11 GB). Launcher Rust + Win32 (`tools/launcher`, exe win7 32 bits) feito em 03/11 com login, Jogar, Painel e Criar conta; instalar/atualizar pendentes. LA4: login e Jogar aceitos; Go Back e administrador corrigidos em 03/11 (ticket reutilizável pelo mesmo jogo, `RunAsInvoker`), LA5 aceito em 03/11 (sem administrador).
  - [ ] Editor de fases (depois do editor de itens).

## Etapas da migração integral (M01–M24)

Marcar [x] só com evidência fresca da etapa inteira.

- [x] M01 inventário dos binários · M02 descompilação 95/95 · M03 skills e arquitetura · M04 seis backups restaurados · M05 decisão SQL Server→PostgreSQL · M06 gate local · M07 Docker e smoke · M08 framing Devcat · M09 ISAAC · M10 inventário do cliente.
- [ ] M11 protocolo — parcial: framing/cifra, login, mensagens usadas por B0–B4 com fixtures do C#.
- [ ] M12 bootstrap do cliente — parcial: FeatureMatrix/EndPoint e cópia de laboratório funcionam.
- [ ] M13 autenticação — parcial: login por login/senha (B4.2b) e segunda senha; passport legado não.
- [ ] M14 dados legados — parcial: 29 usuários copiados; demais dados SQL Server não migrados.
- [ ] M15 personagens — parcial: criar (10 classes + Delia), listar, selecionar, excluir; transferência não.
- [ ] M16 inventário — parcial: B0/B1/B3; GetItemStat/conversões/rates incompletos.
- [ ] M17 conteúdo — parcial: perfis SQLite exportados para o que B0–B4 usam.
- [ ] M18 lobby/canais — parcial: cidade/movimento solo; grupos e chat não.
- [ ] M19 transporte de batalha — parcial: relay ProudNet solo (B2); trava do barco com memória alta corrigida no `engine.dll` do laboratório (RL1 no cliente pendente).
- [ ] M20 MicroPlay — parcial: missão solo (B2), pendências acima.
- [ ] M21 serviços adicionais — parcial: cash shop, armazenamento, correio e mercado (cliente pendente); trade direto, guild, rank não.
- [ ] M22 ferramentas operacionais — parcial: painel B4.
- [ ] M23 paridade completa.
- [ ] M24 aceite final com cliente.

## Dependências não resolvidas

- Código nativo não recuperável pelo ILSpy: NMLinkW, NMServer e componentes de rede/proteção.
- Regras SQL Server (procedures, tipos, collation) ainda não convertidas além do que os blocos exigiram.
- Classes 11+ exigem código/assets do cliente NA (`F:\Games\vindictus`, só leitura).

## Regras de atualização

- Ao fechar um passo verificado: atualizar a linha do checklist correspondente e a "Próxima ação retomável" aqui (curto, sem log), e acrescentar o registro completo em `docs/history/session-log.md`.
- Remover daqui o que deixar de ser pendente; não acumular histórico neste arquivo.
- Estados intermediários: implementado, em verificação, pendente ou dependência não resolvida. Não inferir paridade de quantidade de código portado.
