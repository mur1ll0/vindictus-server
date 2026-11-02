# Migração Vindictus .NET → Rust — roadmap vivo

Atualizado em 31/10/2026. Este arquivo é a memória de trabalho do projeto: o que já foi feito (checklist curto) e o que falta fazer. Os registros detalhados de cada sessão (relatos, causas, comandos, logs, hashes) ficam em `docs/history/session-log.md`, que só recebe acréscimos e não deve ser lido inteiro (ver `AGENTS.md`). A ordem de trabalho e as regras dos blocos estão em `docs/playable-game-roadmap.md`.

## Objetivo e critério de conclusão

Migrar os serviços antigos e as regras de jogo para Rust, executáveis em Docker, compatíveis com o cliente legado 1.69 EN. Conclusão exige login pelo jogo, personagem, inventário, lobby, batalha, recompensa, reconexão e persistência verificados no cliente. Infraestrutura saudável e C# descompilado não significam migração concluída.

Migração completa: **NÃO CONCLUÍDA**.

## Estado atual

- Blocos B0–B4 fechados no código (último gate completo: B4, 27/10/2026 — 524 Rust, 298 PostgreSQL em lotes, 61 Python, smoke Docker exit 0). Só o B0 tem aceite completo no cliente.
- Bloco atual: **B5** (multijogador e sistemas restantes). Pets (P1–P6) adiantados a pedido do operador, sem teste no cliente.
- Trilha paralela de classes novas: Delia (classe 10) jogável e criável pela grade; Miri (classe 11) em porte na branch `feat/miri-class` (`docs/future/miri-class-port.md`).
- Laboratório: `docker compose` (`game`, `server`, PostgreSQL) e cópia do cliente em `.cache/play-client` (`start-lab.bat`, conta `lab169_a`).

## Próxima ação retomável

1. Rebuild do Docker (`COMPOSE_PARALLEL_LIMIT=1 docker compose build`, depois `docker compose up -d --wait`) e sessão de cliente do operador pelo roteiro `docs/client-tests/B5-pets.md` (checkpoints P3 e P6).
2. Miri: M7e (pulos, descanso na mão) e M7f (corrente da Draker no cliente: cabo de batalha mão→arma, cabo de cidade na própria arma, corrente do braço em *bonemerge*, escondidos junto com a arma; teste de cidade pelo estado do jogo, corrente do braço com *precache*), pulo de cidade da Fiona, conjuntos e maestria da Draker; M7g: golpes próprios da Miri no `_ex`, SP, Tab, E nos núcleos, corrente oculta na fonte termal aplicados; MI12 do C4 pendente no cliente. M7h: Flamebreath no servidor (campo de rede da barra de febre) e o HUD `MiriSkillDialog.gfx`; M7i: pulos (`_ex` dos *includes*) e HUD declarado no `UIElements.xml`; M7j: corrente removida enquanto a arma não é desenhada, ativações de pulo e `jump_town` da Delia; M7k: ações da Miri no `action_table_player_ex.txt` (dano e SP ok), ataques no ar da Delia, HUD no `<PreloadList>`; M7l: visibilidade do dragão reenviada (cidade), aplicados; MI17 do C4 pendente. M7m: Dragon Knight, mecânica (efeito sem operação `dragonknight_transform`, estado em `m_gaugeType`, condições, Flamebreath ilimitado e recarga 5/4 s), aplicado; MI18 do C4 pendente. M7n: visual do Dragon Knight (veste o `.csf`, guarda e devolve a aparência); M7o: retrato da Miri na lista de seleção; M7p: correções da MI17–MI20; M7q: modelos e entradas de roupa dos itens importados, nomes dos conjuntos, efeitos `AE_CL_PLAY_CUSTOM_EFFECT` → `AE_CL_PLAY_EFFECT`, aplicados; conjuntos únicos (`dragon_rise_set`, `pythonshock_set`) já no servidor e no painel; M7s: nome e bônus dos conjuntos importados no painel e no jogo (gerados do `SetEffectInfo` onde o NA não tem texto); M7t: rastros de fogo da Draker (normais e de Dragon Knight nas sequências transformadas), gasto de SP da transformação (`AE_GAIN_SP_GAUGE -200`) e XE *move* da Delia como `skill_miri_xe_move`, aplicados; M7u: rastros dentro das 12 alças do 1.69 (crash `0x38828069`; sem rastros de Dragon Knight) e peça única do Dragon Rise (`onepiece`), aplicados; M7v: histórias e títulos "todas as classes" com o bit da Miri e textos do XE Move (da Delia), aplicados; M7w: livros de história (`storyscroll_`, `GiveStory`) no servidor e ramos `Delia`/`Miri` no `Switch` (XE Move da Ceara), aplicados; M7x: E no que cai (`SearchEvilCore`), sem *dash* ao parar a corrida, dragão do HUD escondido na troca de tela, destruir item (`DestroySlotItem`), aplicados; M7y: Parrying Attack (endure da Fiona/Delia para a classe 11, efeito dela) e o dragão do Call of the Dragon (modelo invocado), aplicados. MI23 do C4 pendente (M7r: arquivos `.vvd`/`.vtx`/`.phy` dos modelos, efeitos de Dragon Knight nas sequências transformadas); MI28 e MI29 do C4 pendentes. Itens adiados (sessão de 01/11): M7z ramos do `Switch` de classe para Delia/Miri (vão aonde a Fiona vai), aplicado, MI30 pendente; M8a `resound`/`resound_stamina` e efeito/som do *endure* pelo servidor (MI31); M8b olhos do dragão (MI32); M8c Flamebreath pelo *rank*, começando em 4, Hydra ao entrar (MI33); M8d (efeitos do Dragon Knight pelo estado) aguarda decisão (ver "Decisões pendentes"); M8e rastros escuros por efeitos de laboratório na arma, recusados no cliente e revertidos (M8h: sem rastros de Dragon Knight; MI37 cancelada); M8f `DestroyItemMessage` e item indestrutível vencido (MI35); M8g XE Move com textos e ícone genérico próprios (MI36); M8a–M8g aplicados (Docker, refresh, preparação, aplicador, DLLs); M8h: crash do Parrying Attack no cliente (`0x392204e4`, texto do efeito no meio do código) corrigido, aplicado (MI38: *parry* e E confirmados); M8i: efeitos pelo estado (`AE_CL_PLAY_CUSTOM_EFFECT` no `FireEvent` da Miri, decisão M8d do operador) e sonda do dragão do SP5, aplicados (MI39: o dragão aparece mas fica parado); M8j: asas negras do Dragon Knight (`rear` 500139 do `.csf` do menu; MI40: câmera foi para baixo do barco ao transformar, a peça de costas não tem os ossos de câmera; M8k: a entrada de costas aponta para as asas do NA no esqueleto inteiro, com os ossos de câmera, e o corpo mantém os ossos de cabelo e busto), dragão do SP5 preso à Miri (`SetParent` no servidor), HUD do Flamebreath escondido ao destruir a Miri (Home Screen) e sonda retirada, aplicados (MI40); MI30–MI36 e MI40 pendentes no cliente; M8k aplicado (MI41: câmera ok; membranas com a textura do corpo vistas de trás; sem chamas negras; dragão do SP5 parado no chão na vertical; HUD ainda na tela inicial). M8l: asas com o material de asa (pasta de material de laboratório, dois lados) e chamas negras do NA (`sp4_during_02_wing*`) no `miri_SP4`, aplicados (MI42). MI42: asas ok; dragão do SP5 aparece mas fica no chão. M8m: HUD do Flamebreath só fora da cidade **e** dentro do jogo (predicado `+0x40` do gerenciador de estados: 1–5 e 7; a seleção de personagens é 5 e cria uma Miri cujo *tick* mostrava o HUD, memória MI42), aplicado (MI43). Dragão do SP5: aparece no chão e não sobe (anda de lado com ela); no NA ele copia o ciclo da Miri a cada quadro; MI43: HUD some na seleção (ok). M8n: o dragão do SP5 segue o osso `ValveBiped.Anim_Attachment_GRAPPLING` da Miri a cada *think* do servidor (como o NA), aplicado com o jogo fechado (MI44). Tab (Fiery Evasion) parou: dados iguais aos do NA; suspeita no `config_miri_ex.cfg` de teclas (menor que o das outras classes), trocado pelo da Delia com cópia (MI44). Cópia `.cache/play-client` apagada por engano em 02/11 (junction da worktree) e recuperada do disco e dos originais, conferida por hash (histórico, 02/11); próximo: resultado da MI44. MI1–MI3 pendentes.
   - Grade da Miri (MG1): filme da grade, atriz `npc_miri` e vídeo aplicados na cópia; sessão MG1 do `C4-miri.md` (grade → customização → Create → estalagem → cidade → seleção).
   - B2 RL1 (do ramo do `engine.dll`): rodar no cliente (barco sem trava; `engine.dll` do laboratório já com o patch).
3. Sem bloqueio do operador: seguir o B5 pela próxima etapa acionável (ver "B5 — o que falta").

## Aguardando o operador

| Roteiro | Situação |
|---|---|
| `docs/client-tests/B1-cidade.md` + `B2-fases.md` | Nenhuma entrada testada formalmente (missões já jogadas em sessões de diagnóstico de 22/10). RL1 (barco repetido sem trava, 31/10; patch `patch_engine_iocp_sign.py` já aplicado). |
| `docs/client-tests/B3-equipamento.md` | Nenhuma entrada testada (inclui DY1, ampola de tintura). |
| `docs/client-tests/B4-painel.md` | B4.2-L aceito; demais pendentes (inclui I2 editor de itens, P3 progressão, S1–S3 exclusão de personagem). |
| `docs/client-tests/B5-pets.md` | P1–P9 pendentes (P9: pets no Depot, 30/10). |
| `docs/client-tests/C2-delia.md` | Aceito (D0–D23); D6 passou para o C3. |
| `docs/client-tests/C4-miri.md` | MI4–MI8 jogados (30–31/10): cidade, arma Draker, combate básico no barco ok; retrato → M8. MI11 jogada (31/10); MI16 jogada (31/10: dano e SP ok; dragão ok em batalha, aparecia na cidade com NPC; Dragon Knight sem efeito); MI17, MI18 e MI19 pendentes. MI1–MI3 pendentes. MG1 (criação pela grade) pendente. |
| `docs/client-tests/C3-criacao.md` | Grade, customização, Create → estalagem → cidade aceitos em 30/10; faltam volta à seleção com a personagem criada pela grade, G3 e G5. |

Testes de cliente adiados do B0: aprender skill nova com ouro (B1.2), empilhar pilhas da mesma classe (B1.4 ou B2.6), usar consumível pela barra (B2.5).

Decisões pendentes:
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
- Drops: baú do tesouro, mapas do tesouro, núcleos de evento/história, WeakPoint, HugeLucky; sobra de inventário cheio por correio.
- Combate: MDRSystem só marcaria rankings (inexistentes); caixas alquímicas na barra recusadas.
- Conclusão: recompensas de troca de item (`slayer_stamp_limit>slayer_stamp`), CharacterRank, bônus de status/cafe/guilda/moedas/pedras, AddMicroPlayTime, contagem diária de quests grátis, OnEnterPhase das histórias; objetivo do sapo com regex não compilável.
- Progressão: AddGrowthLog, efeitos de guilda, títulos de rank; `take_item` de peça vestida recusado.
- Histórias: `select_vocation`, `complete_episode`/`give_title_by_episode`.
- Robustez: correio dos itens reservados de quem sai da missão.

### B3 — pendências

- [ ] Item 5: Combine Craft e habilidades de item (`PS_<n>`, `Gear.ToAbilityInfo`, `AbilityBonus` de prop e de prop nomeado, anúncio `GameUI_Heroes_ItemAbility_Enable_Announce`).
- [ ] Tingir peça com a ampola colorida (`DyeItemCashMessage`).

### B5 — o que falta

- [ ] Multijogador: party, convites, instância/combate/loot sincronizados, host/servidor, saída e reconexão de membros (primeiro teste com vários clientes).
- [ ] PvP/arena/ranking.
- [ ] Guildas.
- [ ] Trade.
- [ ] Correio completo.
- [ ] Eventos/UserCare.
- [x] Pets P1–P6 (30/10/2026; cliente pendente; detalhes em `docs/future/pet-system.md`).
- [ ] Pets P7 (pets do NA, junto com a importação das classes) e pet de outro jogador (multijogador).
- [ ] Títulos/achievements restantes (títulos adquiridos e por ação de combate já feitos no B2).
- [ ] Leilão/mercado.
- [ ] Cash shop e cosméticos restantes (Depot já funciona).
- [ ] Paridade e aceite final (M23/M24).

## Trilhas paralelas

- Classes novas (`docs/future/new-classes.md`, `docs/future/delia-class-port.md`, skill `new-class-port`):
  - [x] Delia C2: classe 10 jogável (corpo, armas, guarda, condições, efeitos, Rose Crush, SP), aceita em 29/10.
  - [ ] Delia C3: criação pela grade do NA — falta volta à seleção, G3 e G5.
  - [ ] Miri: M0–M5 feitos; M7a–M7d no cliente (arma, estados, `_ex`, combate básico); M7e (pulos, descanso) e M7f (corrente) aplicados, MI9/MI10 pendentes; mecânicas aceitas no cliente em 02/11; grade MG1 aplicada (`docs/future/miri-creation-grid.md`), MG1 do C4 pendente; falta o aceite; decisões de 30/10 (assets do NA, todas as mecânicas, `weapon_greatsword`, modelos v44).
- Recursos futuros do painel (`docs/playable-game-roadmap.md`, seção "Recursos futuros do painel"):
  - [x] Editor de itens (corte 1 aceito; I2 no cliente pendente).
  - [x] Busca por categoria, editor de refino/scrolls, progressão até um nível (inclui história principal; P3 no cliente pendente).
  - [x] Seleção de personagem: exclusão, cancelamento e ordem dos slots (S1–S3 no cliente pendentes).
  - [ ] Launcher do jogo: teste L0 (arquivos soltos × `.hfs`) a qualquer momento; L1–L3 antes de mudar assets do cliente.
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
- [ ] M21 serviços adicionais — parcial: cash shop, armazenamento; postal, trade, guild, rank não.
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
