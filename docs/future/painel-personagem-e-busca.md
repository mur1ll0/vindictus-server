# Progressão até um nível, editor de refino/scrolls e busca de itens por categoria (recursos futuros do painel)

Status: estudo de viabilidade (27/10/2026). Implementação em andamento por decisão do operador: busca por categoria (C1–C3) feita em 27/10/2026; ver `MIGRATION.md`. Este documento levanta, a partir da evidência do projeto, três recursos pedidos pelo operador para o painel admin. O que ainda não foi verificado aparece marcado como **hipótese**.

Encaixe no roteiro: depois do B4 (fechado) e sem mudar a ordem dos blocos B0–B5. Os três recursos usam o que o B4 já entrega: edição de personagem offline com auditoria e `operation_id` (`src/application/admin_character_edits.rs`), o catálogo do painel (`scripts/content/export_panel_catalog.py` → `EnglishNames` em `src/infrastructure/admin_characters.rs`) e o tooltip do jogo (`src/infrastructure/admin_tooltip.rs`). Ver "Recursos futuros do painel" em [playable-game-roadmap.md](../playable-game-roadmap.md).

Fontes desta análise, todas só para leitura: conteúdo do servidor `../ServidorVindictus/Server/zh-tw-x-gm/Bin/heroesContents.db3` e `Data/XML`, texto do cliente `.cache/play-client/hfs_extracted/resource/localized_text/english_eu/heroes_text_english_eu.txt` (UTF-16LE) e o código Rust/React do repositório. Consultas e contagens em `.cache/verification/feasibility-*.log`.

---

## 1. Progressão até um nível (personagens de teste)

Pedido: na aba Progressão (`web/admin/src/components/ProgressionEdits.tsx`), o operador digita um nível (ex.: 40) e o personagem recebe o nível, as quests e histórias concluídas até esse nível e as receitas/forjas que elas dariam.

### 1.1 Evidência: o que já existe

**Painel hoje.** `PanelGrowth::Level(target)` (`src/infrastructure/admin_progression.rs:19`) calcula a EXP que falta pela tabela de níveis (`exp_to`, linha 83) e chama `minimum_growth::give_reward`, a mesma cadeia de level-up do jogo, numa transação com a auditoria (`character.progression`, antes/depois com nível, EXP, AP, títulos e efeitos). Exige personagem offline (`PanelGrowthOutcome::Online`) e recusa repetição pelo `operation_id`. Subir de nível já dispara o que o jogo faz no level-up e no login: histórias com `AutoGiveLevel` ≤ nível (`StoryScript::auto_give_for_login`, `src/domain/npc_script.rs:1031`), quests com `AutoGiveLevel` (`QuestOwner::login`, `src/domain/quest.rs:713`) e títulos com `AutoGiveLevel` (`src/domain/progression.rs:228`). Ou seja: **abrir** já funciona; **concluir** é o que falta.

**Conteúdo (contagens do `heroesContents.db3`).**

| O quê | Tabela / arquivo | Linhas | Colunas que definem "até o nível N" |
|---|---|---|---|
| Quests (fases de missão) | `QuestInfo` | 269 | `Level` (1–100), `AutoGiveLevel` (190 com valor), `QuestSet` (999+ = teste/PvP/caça), `HuntingQuest`, `Feature` (EP3, EP4, S2EP1…) |
| Dificuldades | `QuestDifficultyInfo` | 904 | `MinLevel`, `MaxLevel` (ex.: `quest_gnoll_scout` 1/1, 1/1, 1/50) |
| Objetivos | `QuestGoalInfo` | 1.466 | `Weight` (225 objetivos com peso 100), `Gold`, `XP`, `Item`/`ItemNum` (ex.: `quest_gnoll_scout` dá `slayer_stamp`) |
| Pré-requisitos | `QuestConstraintInfo` | 82 | `QuestID` → `RequiredQuestID` com `RequiredStatus` 100 |
| Histórias | `StoryLineInfo` | 762 | `AutoGiveLevel` (193 com valor, máx. 80), `Category` (MainStream 112, MainStream_S2 64, Sub 203, Event 169, Skill 21, Equip 71…), recompensas `RewardGold/Exp/Ap/Title/Item/Skill`, `ClassRestriction`, `Feature` |
| Fases das histórias | `StoryGoalInfo` | 8.513 | `Phase` (A, B, C…), `Command`: `talk_to` 2.318, `has_item` 842, `QuestClear` 102, `quest_cleared` 62, `has_title` 38, `level_min` 6, `is_manufacture_reachexp` 14 |
| Roteiro das histórias | `Data/XML` (XML por `StoryLineInfo.XmlPath`) | — | comandos `give_item` 3.448, `complete_storyline` 1.569, `give_quest` 637, `acquire_title` 372, `give_skill` 250, `levelup_manufacture` 33, `acquire_manufacture` 17 |
| Títulos | `TitleInfo` / `TitleGoalInfo` / `TitleStatInfo` | 1.233 / 482 / 907 | `AutoGiveLevel` (452 com valor), `RequiredLevel`, `ClassRestriction`, `Feature` |
| Ofícios (forja do jogador) | `ManufactureInfo`, `ManufactureDependencyInfo`, `ManufactureRecipeInfo` | 37 / 8 / 1.730 | `Grade`/`ExperienceMin`; receitas por `ExperienceRequired`/`GradeRequired`; **1.717 de 1.730 com `AlwaysListed`**; `AutoGiveQuest` vazio em todas |
| Receitas de NPC | `RecipeInfo` | 381 | `AppearQuestID` (72 quests distintas) |

Exemplo "até o nível 40": 81 quests com `Level` ≤ 40, `QuestSet` < 999 e sem caça (29 delas sem `Feature`); 176 histórias com `AutoGiveLevel` 1–40, das quais 128 são `Event`, 27 `Sub`, 9 `Contents`, 8 `Skill`, 3 `Repeat` e só **1** `MainStream`; 416 títulos com `AutoGiveLevel` 1–40. Ou seja, a história principal não é aberta por nível: ela encadeia por `open_storyline`/`give_quest` dos XMLs e por objetivos `QuestClear` (152 histórias dependem de quests concluídas).

**Estado no PostgreSQL** (todos já usados pelo jogo migrado):

| Estado | Tabela | Como o jogo grava "concluído" |
|---|---|---|
| Quest | `game_quests` (`migrations/0036_quests.sql`) | linha por quest (`row_id` = ID de entidade, `revealed`, `status`, `clear_count`, `play_count`…); `status` é recalculado no load como a soma dos `Weight` dos objetivos alcançados (`QuestOwner::init_status`, `src/domain/quest.rs:604`) |
| Objetivos | `game_quest_goal_achievements` | `status` é um campo de bits por dificuldade (`1 << difficulty`), `clear_count` |
| Tempo de conclusão | `game_quest_clear_times` | dificuldade, jogadores, tempo, rank (opcional) |
| História | `game_story_progress` (`0034`) | `phase_utf16` NULL = concluída (`npc_talk.rs:861`); `status` é a visibilidade (0 normal, 1 acompanhando, 255 oculta; `npc_script.rs:1227`) |
| Saves/variáveis de história | `game_story_saves`, `game_story_variables` (`0035`) | pontos de retomada e tokens (`token_is` usa) |
| Títulos | `game_titles`, `game_title_goal_progress` (`0020`) | `acquired`, `acquired_quest_utf16` |
| Ofícios | `game_manufacture` (grade, experiência), `game_manufacture_recipes`, `game_new_recipes` (`0031`) | `AcquireManufacture`/`LevelupManufacture`/`AcquireManufactureRecipe` (`minimum_manufacture.rs`, efeitos `Effect::Manufacture` em `npc_talk.rs:159`) |

Quem grava hoje: `minimum_mission_finish` (conclusão da missão, recompensas `GiveReward_*`, `update_play_count`), `minimum_quest::save` (linhas novas com IDs de entidade), `minimum_story` (`talk`, `set_story_line_status`, recompensas da história aplicadas na mesma transação, linhas 600–710) e `minimum_manufacture`.

### 1.2 O que "até o nível N" significa

Não há no conteúdo uma lista "progresso esperado no nível N". A proposta é derivar um **plano** determinístico do conteúdo, com regras explícitas:

1. **Nível**: o caminho atual (`PanelGrowth::Level`), primeiro. Isso já abre as histórias/quests/títulos de `AutoGiveLevel` ≤ N.
2. **Quests**: `QuestInfo` com `Level` ≤ N, `QuestSet` < 999, `HuntingQuest` falso, `Feature` habilitada pela política de features do servidor (a mesma usada no load do conteúdo), e pelo menos uma dificuldade com `QuestDifficultyInfo.MinLevel` ≤ N. Ordem: fecho transitivo de `QuestConstraintInfo` (uma quest só entra se a requerida entrar). Concluir = objetivos de peso que somam ≥ 100 marcados na dificuldade 1 (bit 1), `clear_count` = 1, `revealed` = 1. **Hipótese:** marcar só os objetivos positivos obrigatórios basta para o quadro mostrar "concluída"; conferir no cliente.
3. **Histórias**: as que o jogo abriria (`AutoGiveLevel` ≤ N e as abertas pela cadeia) e cujos requisitos podem ser satisfeitos pelo plano: `QuestClear`/`quest_cleared` de quests do plano, `level_min` ≤ N, sem `has_item`/`has_title`/`token_is` que o plano não produz. Concluir = `phase_utf16` NULL e `status` 0. Categorias `Event`, `Repeat` e `Contents` (SocialProduct) ficam **fora** por padrão (escolha do operador numa caixa).
4. **Títulos**: só os que as histórias/quests concluídas dariam (`acquire_title` dos XMLs, `StoryLineInfo.RewardTitle`) e os de `AutoGiveLevel` ≤ N que o level-up já aplica; títulos por `TitleGoalInfo` (matar X monstros etc.) ficam fora.
5. **Ofícios/receitas**: `acquire_manufacture` e `levelup_manufacture` das histórias concluídas (17 e 33 ocorrências nos XMLs). Receitas: quase todas são `AlwaysListed` e dependem só de `Grade`/`ExperienceRequired`, então "dar as receitas" é dar o ofício com o grau certo; `AcquireManufactureRecipe` só para as poucas sem `AlwaysListed`. Receitas de NPC (`RecipeInfo.AppearQuestID`) aparecem pela quest concluída, sem estado próprio (**hipótese**: o Rust ainda ignora `AppearQuestID` nas lojas, `src/infrastructure/npc_scripts.rs:242`; até isso ser migrado, nada muda na loja).

A extração do grafo história → comandos precisa ler os XMLs (`Data/XML`, por `XmlPath`), o que o `export_npc_scripts.py` já faz para o `npc-scripts.json`. O plano pode ser calculado no Rust a partir do `npc-scripts.json` carregado (`assets.npc`), sem exportação nova, se os comandos de recompensa já estiverem no export (**verificar** antes da etapa P1; senão, acrescentar ao exportador).

### 1.3 Recompensas: dar ou não

Recomendação: **não executar as recompensas de quest e história**, exceto as que definem progressão estrutural.

| Efeito | Dar? | Motivo |
|---|---|---|
| EXP/AP de quest e história | não | o nível alvo já vem do passo 1; somar EXP passaria do nível pedido |
| Ouro, itens (`give_item`, `QuestGoalInfo.Item`, `RewardItem`) | não (opção) | milhares de itens (3.448 `give_item`), troféus e itens de história que lotariam o inventário; o operador dá itens pelo painel |
| Títulos de história/quest | sim | definem requisitos (`has_title`) e o que o jogador vê; pequenos e idempotentes |
| Skills (`give_skill`, `RewardSkill`) | sim (opção, padrão ligado) | skills de classe vêm de histórias `Skill`; sem elas o personagem de teste não representa o nível |
| Ofícios (`acquire_manufacture`, `levelup_manufacture`) | sim | é o pedido "receitas/forjas" |
| Slots/armazéns (`OpenEquipmentSlot`, `OpenStorage`) | sim | estrutural, idempotente |
| `give_quest` | sim, só para as do plano | abrir sem concluir deixaria a quest pendente |

A tela mostra o plano antes de aplicar (contagens por tipo e a lista), e a resposta traz o antes/depois por tipo, como as outras edições.

### 1.4 Desenho proposto

**API**
- `GET /api/characters/{cid}/progression/plan?level=N&events=false&skills=true` → `{ level, quests: [...], stories: [...], titles: [...], manufactures: [...], skipped: [{id, reason}] }` (só leitura, `Permission::Read`).
- `POST /api/characters/{cid}/progression/reach` com `{ operation_id, level, events, skills }` → `ProgressionResult` estendido (contagens antes/depois de quests concluídas, histórias concluídas, títulos, ofícios, skills).

**Aplicação**: novo `PanelGrowth::Reach { level, options }` em `admin_progression::panel_change`, dentro da mesma transação e auditoria (`character.progression.reach`), com o personagem offline. Ordem: (1) level-up pelo caminho existente; (2) carregar `QuestOwner`, `StoryEntity` e ofícios pelos mesmos loaders do login; (3) aplicar o plano por métodos de domínio (novos, testáveis sem banco): `QuestOwner::complete_for_panel`, `StoryState::complete_for_panel`; (4) salvar pelos `save` existentes (IDs de entidade pelo `EntityAuthority` do painel, `PANEL_SERVICE_ID`); (5) efeitos estruturais pelos mesmos aplicadores das recompensas de história (`minimum_growth::acquire_title`, `skill_list::give`, `Effect::Manufacture`). Nada de SQL solto fora desses caminhos.

**Idempotência**: aplicar o mesmo nível de novo não muda nada ("sem_mudanca"); nível abaixo do atual recusa (`nivel`), como hoje. Não há "desfazer" automático; o antes/depois na auditoria registra o que mudou.

**UI**: modo novo "Progresso até o nível" na `ProgressionModal`, com o campo de nível, caixas (eventos, skills), botão "Ver plano" (lista por tipo, com nomes em inglês do catálogo: `quest_titles`, nomes de história) e confirmação.

### 1.5 Riscos e hipóteses

- **Estado inconsistente no cliente** (hipótese principal): o cliente monta o quadro de quests e o diário de histórias a partir de `QuestProgressMessage` e `SyncStoryProgress` no login; histórias concluídas sem os `game_story_saves`/variáveis que o roteiro teria criado podem deixar NPCs com falas de fases antigas. Teste de cliente obrigatório (seção 1.6).
- Histórias com `has_item`, `has_item_enhanced`, `token_is` e `is_manufacture_reachexp` não são satisfeitas pelo plano: ficam abertas e aparecem em `skipped`.
- `Feature`: o conteúdo mistura episódios desligados na região EU; usar a política de features já carregada, nunca a lista crua.
- Títulos com `ClassRestriction` e histórias com `ClassRestriction` (máscara de classes, 2047 = todas) devem ser filtrados pela classe do personagem.
- Volume: uma transação com ~80 quests, ~50 histórias e ~400 títulos é pequena para o PostgreSQL; limitar o nível a `MaxLevel` da política.
- Oráculo possível: personagens importadas do backup legado (há 51 no banco do laboratório) mostram como o jogo original guarda quests/histórias de um personagem de nível alto. Usá-las só para comparar formato, nunca como fonte de verdade de regras.

### 1.6 Testes

- Unidade (domínio): plano para níveis 1, 10, 40 e `MaxLevel` com um conteúdo de fixture (fecho de `QuestConstraintInfo`, filtro de `Feature`, `ClassRestriction`, histórias puladas com motivo); `complete_for_panel` produz o mesmo `status` que `init_status` calcula depois do load.
- PostgreSQL filtrado (`minimum_give::panel_*`/`admin_progression`): offline obrigatório; repetição pelo `operation_id`; idempotência; o load seguinte do login (`minimum_quest::login`, `minimum_story::login`) aceita o estado gravado sem erro.
- Cliente (roteiro novo no `docs/client-tests/`): personagem nova → painel "até o nível 20" → login → quadro de quests mostra as quests concluídas e libera as seguintes; diário de histórias sem pendências antigas; títulos na janela de títulos; ofício no NPC de forja; falar com Ceara/Marrec sem travar.

### 1.7 Etapas

| Etapa | Conteúdo | Tamanho |
|---|---|---|
| P0 | Conferir se `npc-scripts.json` já traz os comandos de recompensa por história; senão, ampliar `export_npc_scripts.py` (com testes Python) | pequena |
| P1 | Domínio: plano (`progression_plan.rs`) e conclusão de quest/história; testes de unidade | média |
| P2 | Infraestrutura/aplicação: `PanelGrowth::Reach`, rotas plan/reach, OpenAPI; testes PostgreSQL filtrados | média |
| P3 | UI na `ProgressionModal` (plano e confirmação); `npm run build` | pequena |
| P4 | Roteiro de cliente e sessão do operador; ajustar regras pelo resultado | — |

---

## 2. Editor de refino (Enhance) e scrolls (Enchant) de um item

Pedido: na ficha (inventário), ao editar um item (ex.: uma longsword), escolher o nível de Enhance e os scrolls de prefixo/sufixo válidos para aquele item, com prévia "antes → depois" das estatísticas.

### 2.1 Evidência

**Armazenamento.** Atributos em `game_item_attributes` (`migrations/0018_item_state.sql:14`: `item_id`, `attribute_bytes`, `value_bytes`, `arg`, `arg2`). Domínio em `src/domain/item_attribute.rs` (limites de texto, forma `ItemClassEx` `classe[ENHANCE:6]`, `parse_class_ex`). Escrita por `minimum_give::insert_attributes`/`delete_attributes` (`src/infrastructure/minimum_give.rs:479/507`).

**Enhance.** `EquipItemInfo.EnhanceType` (ex.: `longsword_swordwind` → `LONGSWORD_LVL1`; 8.704 classes sem tipo, ou seja, sem refino). Níveis por tipo em `EnhanceInfo` (3.167 linhas; máximo 15 nos tipos `_LVL1`–`_LVL4`, 3/5/8/10/11/12 nos sufixados como `ARMOR_LVL3_10`). Estatística por nível em `EnhanceStatInfo` (12.278 linhas; `LONGSWORD_LVL1` nível 1 ATK +50 … nível 12 ATK +2000). Regras do jogo em `src/domain/enhance.rs` (`load`, `bonus_stat`, `current_level`) e `src/domain/enhance_item.rs` (sucesso, materiais, falha), aplicadas por `minimum_enhance.rs`. Conteúdo exportado por `export_item_enhance.py` → `infrastructure::item_enhance::Contents` (`enhance_type`, `stat_rows`, `info`).

**Enchant.** `EnchantInfo` (248 linhas): `EnchantClass`, `IsPrefix`, `EnchantLevel` 1–9, `MinArgValue`/`MaxArgValue` (o `Arg` entra no texto da estatística como `{0}`), `ItemConstraint` (ex.: `WEAPON`; `CLOTH,LIGHT_ARMOR,…`; `(HEAVY_ARMOR,PLATE_ARMOR)&(HELM,PANTS)`; listas de classes como os anéis `s2_ring_*`), `Duration` (timed), `IgnoreMaxArmor`. Estatísticas e condições em `EnchantStatInfo` (618 linhas; ex.: `test` → `Balance+10` com `level>=(3)`). Regras em `src/domain/enchant.rs`: `is_available_to` (linha 216: cada grupo `&` precisa de um token contendo a classe, a categoria ou a `EquipClass`), `load` (condições pelo dono: nível, classe, ranks de skill, títulos), `do_enchant`. A sessão do jogo (`minimum_enchant.rs:180–245`) exige `EquipItemInfo.Enchantable` ≠ 0, scroll com exatamente um `PREFIX` ou `SUFFIX`, item na bolsa e não quebrado (salvo `IgnoreMaxArmor`). O scroll é `enchant_scroll[PREFIX:<EnchantClass>]`.

**Rank.** O texto do cliente usa `GAMEUI_HEROES_ITEM_TOOLTIP_ENCHANT_RANK` "Rank" e `HEROES_ITEM_CONSTRAINT_ENCHANT_LEVEL` "For Enchant rank %s and above"; os rótulos de rank são os de skill (`GAMEUI_HEROES_SKILL_RANK_1` "F" … `_6` "A", `_7` "9", `_8` "8", `_9` "7"). **Hipótese:** `EnchantLevel` 1–9 = rank F…7 (6 = A). O "rank A" do pedido é, então, o rank do scroll, não um atributo do item; o item tem `Rarity`, `Level` e `RequiredLevel` (`longsword_spiked`: raridade 4, nível 23). `ItemClassInfo.RequiredEnchantLevel` existe; conferir se alguma classe o usa antes de exibi-lo como restrição.

**Estatísticas e tooltip.** `minimum_item_stat::equipment_with` (`src/infrastructure/minimum_item_stat.rs:117`) calcula `Item.Stat` = estatística da classe + ENHANCE + QUALITY + PREFIX/SUFFIX, com o contexto do dono (`context`, linha 76), avaliando as duas aritméticas medidas e exigindo que concordem. `admin_tooltip::build` (`admin_tooltip.rs:145`) já monta o tooltip do cliente (estatísticas, `enhance_level`, enchants com rank e linhas de condição ok/não ok) a partir de uma classe e de atributos quaisquer. Portanto a prévia "depois" é só chamar `build` com os atributos propostos; o "antes" é o tooltip atual do item.

**Painel hoje.** "Dar item" já aceita `classe[ENHANCE:6]` e `[PREFIX:…]` (`GiveItemModal`, `web/admin/src/components/CharacterEdits.tsx:116`). A validação (`minimum_give.rs:196–212`) só exige que a estatística seja calculável. **Lacunas encontradas:** não confere `EnchantInfo.ItemConstraint` (`is_available_to`), `Enchantable`, se o `EnhanceType` existe nem o nível máximo do tipo. Um `ENHANCE:20` numa longsword `LONGSWORD_LVL1` (máx. 15) grava nível 20 sem linhas de estatística (bônus zero), e o cliente mostraria +20. O editor novo corrige isso para os itens editados; o "dar item" deveria passar pelas mesmas regras (etapa E1).

### 2.2 Desenho proposto

**Regras (domínio, novo `src/domain/item_upgrade.rs`)** para um item de classe C:
- Enhance permitido se `enhance_type(C)` existe; níveis válidos = os níveis presentes em `EnhanceInfo` daquele tipo (0 = remover).
- Prefixo/sufixo permitidos = `EnchantInfo` com `IsPrefix` correspondente, `Feature` habilitada, `is_available_to(info, C, categoria, EquipClass)` verdadeiro e `Enchantable(C)` ≠ 0; `Arg` dentro de `[MinArgValue, MaxArgValue]` (padrão: máximo); enchants com `Duration` > 0 (temporários) ficam fora no primeiro corte.
- Item combinado (atributos `COMBINE*`), `ENHANCEOTHER` e atributos desconhecidos: recusa (o cálculo atual já recusa).
- Sem alterar `ANTIBIND`, `QUALITY`, `LOOK`, `COSTUMELOC`: são preservados.

**API**
- `GET /api/characters/{cid}/items/{item_id}/upgrade-options` → `{ enhance: { type, levels: [..], current }, prefixes: [{class, name, rank, stats: [...], arg: {min,max}}], suffixes: [...], current: {prefix, suffix} }` com nomes do cliente (`GAMEUI_HEROES_ATTRIBUTE_PREFIX_<CLASS>`, linhas `GAMEUI_HEROES_ENCHANTSTAT_*`), filtráveis por rank e texto.
- `POST /api/characters/{cid}/items/{item_id}/upgrade/preview` com `{ enhance, prefix, suffix }` → `{ before: ItemTooltip, after: ItemTooltip }` (sem escrita; mesmo `build` com o contexto do dono).
- `POST /api/characters/{cid}/items/{item_id}/upgrade` com `{ operation_id, enhance, prefix, suffix }` → antes/depois.

**Gravação**: `admin_character_edits::upgrade_item` → infraestrutura nova, transação única: personagem offline; o item pertence ao personagem e está numa coleção do inventário; valida pelas regras acima e por `equipment_with` (se falhar, recusa `NotEditable` com o rótulo da regra no log, como `panel character change refused`); troca só as linhas ENHANCE/PREFIX/SUFFIX em `game_item_attributes`; se o item estiver equipado, refaz a aparência/estatística do gear pelo caminho de `minimum_equip` (`refresh_gear`); auditoria `character.item.upgrade` com os atributos antes/depois. Sem sucesso falso: combinação inválida responde 409 com o motivo, nunca grava parcial.

**UI**: no `ItemModal` da ficha (`web/admin/src/pages/Character.tsx:87`), botão "Refino e scrolls" (só para equipamento com `EnhanceType` ou `Enchantable`). Painel com seletor de nível (0…máx.), dois seletores de scroll (busca por nome, filtro por rank F…7, estatísticas e condições visíveis), campo de `Arg` quando há faixa, e a prévia lado a lado com `ItemTooltipBox` antes → depois (diferenças destacadas; condições não atendidas pelo dono em cinza, como o jogo).

### 2.3 Riscos e hipóteses

- **O que o cliente aceita**: o cliente recebe os atributos no inventário e calcula o próprio tooltip; um nível de Enhance acima do máximo do tipo ou um enchant fora da restrição pode ser exibido ou travar o tooltip (**hipótese**; a regra nova evita gravar esses casos).
- Efeitos visuais do refino (`EnhanceInfo.Effect`, brilho a partir de certo nível) vêm do cliente; conferir no teste de cliente que o brilho aparece.
- Condições de enchant dependem do dono (nível, classe, skills, títulos): a prévia usa o personagem dono; mover o item para outra classe muda as linhas ativas, como no jogo.
- Itens equipados: a sessão de enchant do jogo não migrou alvos equipados; o editor do painel pode tratá-los porque o personagem está offline, mas precisa do `refresh_gear` (teste PostgreSQL dedicado).
- Durabilidade: o refino/enchant real pode reduzir durabilidade; o editor não mexe nela.

### 2.4 Testes

- Unidade: opções para `longsword_swordwind` (níveis 1–15 de `LONGSWORD_LVL1`; prefixos `WEAPON` sim, `TUNIC` não; grupo `&` de armadura); recusa de nível 16, enchant temporário, item sem `Enchantable`; `Arg` fora da faixa.
- PostgreSQL filtrado: offline, repetição, item de outro personagem, item equipado (gear refeito), troca de prefixo preserva `ANTIBIND`/`QUALITY`; o login seguinte carrega o item sem erro.
- HTTP: OpenAPI regenerado (`admin_http`).
- Cliente: longsword +10 com prefixo e sufixo de rank A editados offline → login → tooltip igual à prévia do painel (ATK, rank, linhas), brilho de refino, equipar e desequipar; repetir com uma peça de armadura com restrição `&`.

### 2.5 Etapas

| Etapa | Conteúdo | Tamanho |
|---|---|---|
| E1 | Domínio `item_upgrade` (opções e validação) + aplicar a mesma validação no "dar item" com atributos | pequena |
| E2 | Rotas options/preview (só leitura) reaproveitando `admin_tooltip::build` | pequena |
| E3 | Gravação offline com auditoria e `refresh_gear`; testes PostgreSQL | média |
| E4 | UI no `ItemModal` com prévia; `npm run build`; roteiro de cliente | média |

---

## 3. Busca de itens em modal paginado com categorias do marketplace

Pedido: em todo o painel onde hoje é preciso digitar para achar um item, um modal paginado com filtros por categoria como no marketplace do jogo (Armas → Longsword, Pillar…; Armaduras → Plate, Plate Helm…).

### 3.1 Evidência: categorias do jogo

**Conteúdo.** Cada linha de `ItemClassInfo` (19.702) tem `TradeCategory` e `TradeCategorySub`; nenhuma linha está sem categoria. As tabelas `MainCategoryInfo` (15 linhas, na ordem: CLOTH, LIGHTARMOR, HEAVYARMOR, PLATEARMOR, AVATAR, WEAPON, ACCESSORY, COMBINE_PART, GOODS, MATERIAL, QUEST, TIRCOIN, EVENT, REAR, ETC) e `SubCategoryInfo` (82 linhas) definem a árvore e a ordem; ambas têm `Feature` (ex.: `STAFF`/`SCYTHE` → `Evy`, `PHANTOMDAGGER` → `Hagie`, `AVATAR` → `AvatarTradeCategory`, `REAR` → `zhCN`). `MainCategoryDetailOptionsInfo` (27) liga categorias a grupos de opções de detalhe (**hipótese:** filtros extras da tela, como nível e raridade).

Diferença de nomes: em `SubCategoryInfo` as armaduras usam `HELM`, `TUNIC`, `PANTS`, `GLOVES`, `BOOTS`, enquanto os itens trazem `CLOTH_HELM`, `PLATEARMOR_BOOTS` etc. A subcategoria do item é então `<MAIN>_<SUB>` para as armaduras. Há pares fora da árvore: `EQUIPMENT/CUSTOMIZE` (2.023 itens de aparência), `TAIL/TAIL` (6), `ACCESSORY/ARTIFACT` (51), `GOODS/SHAREENABLE`, `AVATAR/PLATEARMOR_TUNIC` etc. O painel precisa de um grupo "Outros" por categoria para não esconder itens.

**Nomes em inglês.** 97 chaves `HEROES_ITEM_TRADECATEGORY_NAME_<X>` no texto EU do cliente (log `.cache/verification/feasibility-tradecat-text.log`). Exemplos: `WEAPON` "Weapon", `LONGSWORD` "Longsword", `PILLAR` "Pillar", `HAMMER` "Long Hammer", `DUALSWORD` "Twin Swords", `BATTLEGLAIVE` "Glaive", `BLASTER` "Cestus", `CASTLET` "Focus", `LONGBLADE` "Spellsword", `LARGESHIELD` "Giant Shield", `PLATEARMOR` "Plate Armor", `HELM` "Helm", `TUNIC` "Tunic", `BOOTS` "Boots", `MATERIAL_ORE` "Ore", `TROPHY` "Loot", `KEYITEM` "Story", `CUSTOMIZE` "Customization". `AVATAR*`, `REAR*` e `TAIL` só têm texto coreano no arquivo EU: o painel usa um nome português fixo para eles ("Avatar: Helm" etc.).

**Tela do cliente.** `scaleform/TradePurchase.gfx` existe no cliente (**hipótese:** é a tela do marketplace; não foi desmontada). A árvore e os nomes acima bastam para o painel; não é preciso ler o `.gfx`.

### 3.2 Evidência: busca no painel hoje

| Endpoint | Onde | Comportamento |
|---|---|---|
| `GET /api/items?search=` | `src/http/admin/characters.rs:159` → `EnglishNames::search_items` (`src/infrastructure/admin_characters.rs:635`) | 2–64 caracteres obrigatórios, filtro por substring na classe ou no nome, sem itens ocultos, ordena por nome, corta em 50; sem paginação nem categoria |
| `GET /api/item-editor/items?search=&filter=` | `src/http/admin/items.rs:72` → `application::item_editor::list` | até 200, filtros adicionados/editados/ocultos/pendentes |

O catálogo em memória (`EnglishNames`) vem do `panel-catalog.json` (18.884 itens em `item_edit` na última reexportação, segundo o `MIGRATION.md`). O `item_info` exportado (`export_panel_catalog.py:40–45`, chaves curtas `r`, `c`, `rl`, `lv`, `cr`, `ec`…) **não** inclui `TradeCategory`/`TradeCategorySub`.

Pontos do frontend que exigem digitar (todos a trocar pelo modal):

| Arquivo | Ponto | Hoje |
|---|---|---|
| `web/admin/src/components/CharacterEdits.tsx:76/91` | Dar item (`GiveItemModal`) | busca por texto + lista de 50; campo livre `ItemClassEx` (linha 116) |
| `web/admin/src/pages/Shops.tsx:57/182` | Lojas de NPC: acrescentar item | busca por texto |
| `web/admin/src/pages/Models.tsx:57/81` | Modelos 3D | busca por texto |
| `web/admin/src/pages/ItemEditor.tsx:438/468` | Lista do editor de itens | busca + filtros próprios (até 200) |
| `web/admin/src/pages/ItemEditor.tsx:195` | Modelo do clone (`template`) | classe digitada sem busca |
| `web/admin/src/pages/ItemEditor.tsx:251` | Conjunto (`set_id`) | texto livre (fora do escopo de itens, manter) |
| `web/admin/src/pages/Character.tsx:313` | Filtro do inventário da ficha | filtro local das abas (manter; não é catálogo) |
| `web/admin/src/pages/CashShop.tsx:62` | Cash shop | filtra categorias de produto; não acrescenta item novo hoje. Quando acrescentar produto existir, usar o modal |
| Recurso 2 (seção 2.2) | escolha de scroll | o mesmo modal genérico de lista paginada serve |

### 3.3 Desenho proposto

**Exportação**: `export_panel_catalog.py` passa a gravar no `item_info` `tc` (TradeCategory) e `ts` (TradeCategorySub) e, no topo, `trade_categories`: a árvore de `MainCategoryInfo`/`SubCategoryInfo` na ordem do conteúdo, com `Feature`, nomes do cliente (`HEROES_ITEM_TRADECATEGORY_NAME_*`) e o mapeamento das subcategorias de armadura (`PLATEARMOR` + `HELM` → `PLATEARMOR_HELM`). Itens adicionados pelo editor herdam a categoria do modelo (ou a escolhida no formulário). Teste Python: toda linha de `ItemClassInfo` cai em um nó (inclusive "Outros").

**Índice em memória** (Rust, ao carregar o catálogo): `Vec` de itens ordenado por (categoria, subcategoria, nome) e um mapa nó → faixa; a busca filtra a faixa (no máximo ~19 mil comparações de substring, sem banco). Filtros adicionais baratos: classe permitida (`cr`, máscara), faixa de `RequiredLevel`, raridade, "equipável", e a origem (base/adicionado/editado/oculto) para o editor.

**API**
- `GET /api/items/categories` → árvore com contagens por nó (respeita `Feature` da política e itens ocultos).
- `GET /api/items?search=&category=&sub=&class=&min_level=&max_level=&rarity=&page=&per_page=` → `{ items: [CatalogItem + tc/ts/lv/r], total, page, per_page }`. `search` passa a ser opcional quando há categoria; `per_page` ≤ 100. Compatível: com só `search`, o resultado da primeira página é o de hoje.
- `GET /api/item-editor/items` ganha os mesmos parâmetros (reaproveita o índice), mantendo `filter`.

**UI**: componente `ItemPickerModal` (em `web/admin/src/components/`): árvore de categorias à esquerda (nomes do cliente, contagem), campo de busca opcional, filtros (classe, nível, raridade), grade/lista paginada com `ItemIcon`, nome, classe e `WithItemTooltip`; seleção simples ou múltipla (lojas). TanStack Query com `queryKey` por filtro e página, `placeholderData` para manter a página anterior. Os pontos da tabela 3.2 trocam o campo de texto por um botão "Escolher item" (o campo de texto continua para quem já sabe a classe).

### 3.4 Riscos e hipóteses

- Árvore do cliente × árvore do conteúdo: a ordem e as abas do marketplace do cliente vêm do `.gfx` (**hipótese**); o painel usa a ordem das tabelas. Diferença visual, sem efeito no jogo.
- Itens sem nome em inglês continuam pela classe; não somem da categoria (hoje `search_items` só percorre os itens com nome; o índice novo deve partir do `item_info`).
- O índice precisa ser refeito quando o catálogo é reexportado (`refresh_item_content.py` já reinicia o `game`, que recarrega).

### 3.5 Testes

- Python: árvore e categoria de cada linha (`test_export_panel_catalog.py`).
- Rust (unidade): paginação (total, última página, `per_page` limitado), filtros combinados, itens ocultos fora, compatibilidade da busca só por texto.
- HTTP: OpenAPI regenerado; parâmetros inválidos → 400.
- Frontend: `npm run build`; conferência manual nas cinco telas.
- Cliente: não necessário (só painel).

### 3.6 Etapas

| Etapa | Conteúdo | Tamanho |
|---|---|---|
| C1 | Exportador: `tc`/`ts` e `trade_categories`; reexportação pelo `refresh_item_content.py` | pequena |
| C2 | Índice e API paginada (`/api/items`, `/api/items/categories`, editor de itens) | pequena |
| C3 | `ItemPickerModal` e troca nos pontos da tabela 3.2 | média |

---

## 4. Recomendação de ordem

1. **Busca por categoria (C1–C3)** primeiro: só painel, sem cliente, risco baixo, e o modal é reaproveitado pelos outros dois (escolha de scroll no recurso 2, de itens na seleção do plano do recurso 1).
2. **Editor de refino/scrolls (E1–E4)**: E1 fecha uma lacuna real do "dar item" (nível de Enhance acima do máximo e enchant fora da restrição gravam sem recusa). Todo o cálculo já existe (`equipment_with`, `admin_tooltip::build`); o teste de cliente é curto.
3. **Progressão até o nível (P0–P4)** por último: maior incerteza (histórias encadeadas por XML, estado do diário no cliente) e precisa de sessão de cliente para calibrar as regras. Até lá, o modo "Nível" atual já cria personagens de teste com o nível e o que o level-up abre.

Os três respeitam as regras do painel: personagem offline, transação única com auditoria e `operation_id`, recusa com motivo em vez de sucesso falso, e nenhuma escrita nos bancos de conteúdo originais.
