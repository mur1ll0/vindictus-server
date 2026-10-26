# Editor de fases (recurso futuro do painel)

Status: planejamento (26/10/2026). Nada implementado. Este documento levanta os requisitos de um editor de fases no painel admin a partir da evidência do projeto. O que ainda não foi verificado aparece marcado como **hipótese**.

Encaixe no roteiro: depois do B4 (precisa do ciclo de edições/publicação do B4.4c e do relatório de viabilidade B4.7) e depois das primeiras etapas do [editor de itens](item-editor.md), porque drops e recompensas referenciam itens. Mudanças que exigem assets novos no cliente dependem do [launcher](game-launcher.md). Ver "Recursos futuros do painel" em [playable-game-roadmap.md](../playable-game-roadmap.md).

## 1. O que é uma "fase" no conteúdo

Uma fase é uma quest de missão (`QuestInfo.ID`, ex.: `quest_gnoll_scout`) jogada numa sequência de setores. Cada setor é um BSP do cliente. A tabela abaixo foi levantada no conteúdo do servidor `ServidorVindictus/Server/zh-tw-x-gm/Bin/heroesContents.db3` (251 tabelas) e no banco do cliente `.cache/play-client/sql/heroes.db3` (151 tabelas), ambos abertos só para leitura.

| Parte da fase | Tabela / arquivo | Linhas (servidor) | No banco do cliente? | Consumidor no Rust hoje |
|---|---|---|---|---|
| Identidade, NPC que oferece, nível, tempo, membros, navio | `QuestInfo` (`ID`, `Client` = NPC, `STAGE` = chave de texto do nome, `QuestSet`, `QuestGroupID`, `Level`, `AutoGiveLevel`, `TimeLimit`, `MaxMemberCount`, `MaxPlayCount`, `DisableUserShip`, `EnableUserDSMode`, `RequiredItem`, `RequiredStoryLineID`, `RequiredPhase`, `PropDropTableName`, `DropItemCountMin/Max`, `HuntingQuest`, `IsHardcore`, `EnablePracticeMode`, `Feature`…) | 269 | sim (mesmas colunas) | `npc-scripts.json` `mission.quests` e `quests.quests` (`scripts/content/export_npc_scripts.py`) → `domain/mission.rs`, `domain/quest.rs` |
| Dificuldades | `QuestDifficultyInfo` (`MinLevel`, `MaxLevel`, `GameMode`, `EvilCoreCount`, `ShowBossHp`, `BossHPRatio`, `BossATKRatio`, `BossDefRatio`, `MonsterAttackDamageRatio`, `EliteProbability`, `GoalExpRatio`, `GoalGoldRatio`…) | 904 | sim | `mission.quest_difficulties`, `mission.difficulty_rewards`, `quests.difficulties` |
| Objetivos e recompensas | `QuestGoalInfo` (`Target` regex, `TargetCount`, `Gold`, `XP`, `Item`, `ItemNum`, AP…) | — | sim | `mission.goals`, `mission.goal_infos` (`domain/mission_goal.rs`) |
| Sequência de setores e mapa de cada setor | `Sectors` (`QuestID`, `Sequence`, `SectorID`, `BSPName`, `Weight`, `MinPropItemNum`, `MaxPropItemNum`, `GoldDropTotal`, `GoldDropPerProp`) | 889 | **não** | `mission.sectors` → `SectorSequenceBuilder.GetSectorSequence` em `domain/mission.rs` |
| Ligações entre grupos de setores (portais) | `SectorGroupInfo` (`FromGroup`, `FromSector`, `FromTrigger`, `ToGroup`, `ToSpawnPoint`, `Feature`) | — | não | `mission.sector_groups` |
| Spawns, gatilhos, props, baús, rochas, boss | `Entity` (`ID`, `SectorID`) + `EntityAttribute` (`EntityID`, `Name`, `Value`): cópia, em banco, das entidades de cada BSP | 22.018 / 198.703 | não | `mission.entities` → `SectorBase.InitializeEntityString` (`domain/mission.rs:2597`) |
| Monstros (EXP, HP, ATK, DEF por dificuldade) | `MonsterInfo` (`Class`, `Variation`, `Difficulty`, `Level`, `Exp`, `HP1..5`, `ATK`, `DEF`, `WeakpointReward`…) | 1.381 | sim | `mission.monsters` |
| Drops de monstro | `MonsterDropItemInfo` (`MonsterID`, `QuestID`, `Difficulty`, `ItemClassEX`, `Weight`, `IsAbsolute`, `IsUniqueDrop`…) | — | não | `mission.monster_drops` → `MissionContents::assign_drops` |
| Drops de props e materiais | `PropItemWeightInfo`, `PropItemExpectationInfo`, `MaterialGroupInfo`, `PropItemInfo` | — | não (`MaterialGroupInfo` sim) | `mission.prop_items`, `prop_expectations`, `material_groups` |
| Props nomeados | `NamedPropItemInfo`, `NamedPropDropAmountInfo` | — | não | `mission.named_props`, `named_prop_items`, `named_prop_amounts` |
| Drops de história | `StoryLineDropItemInfo` | — | não | `mission.story_drops` |
| Regras por quest | `QuestSettingsInfo`, `QuestSettingsConditionInfo`, `QuestConstraintInfo`, `QuestPeriodInfo` | — | `QuestConstraintInfo` sim | `mission.settings`, `setting_conditions` |
| Efeitos de missão | `MicroPlayEffectInfo` | — | não | `mission.effects` (`minimum_mission::apply_effect`) |
| Ainda sem consumidor (pendências do B2) | `TreasureBoxDropItemInfo`, `TreasureMapInfo`, `WeakpointRewardInfo`, `TriggerDropInfo`, `TowerModuleSectorInfo`/`TowerModuleSettingsInfo` (torre, recusada), `Hunting*` (caça, recusada), `TodayMissionInfo`, `EqualQuest*` | — | parte | — |
| Mapas de "stage" antigos | `Stage` (`ID`, `Name`, `Map`: 4 linhas, `01a`–`04a`); `StageInfo` vazia | 4 / 0 | `Stage` sim | — |

Não existe tabela `MapInfo` em nenhuma das duas bases. O único arquivo de "informação de mapa" do cliente é `scripts/base_map_info.txt` (7 linhas: um BSP destruído herda o BSP base, ex.: `t03_destroyed.bsp` → `t03.bsp`).

### 1.1 Entidades: o que um setor contém

Contagem de `EntityAttribute` com `Name='classname'` no conteúdo do servidor: `npc_template_maker` 12.456, `trigger_once` 3.784, `math_counter` 2.255, `trigger_multiple` 763, `random_box` 550, `trigger_respawn` 454, `prop_physics_multiplayer` 424, `mining_rock` 410, `npc_actor` 153, `gathering_rock` 145, `treasure_box` 136, `scriptable_monster_event` 90, `trigger_sector_move` 78, `event_of_trial` 76, `unique_npc_template_maker` 39, `named_boss_npc_template_maker` 18, entre outras. Os atributos mais comuns são `targetname`, `OnStartTouch`, `SpawnFrequency`, `MaxNPCCount`, `MaxLiveChildren`, `DestinationGroup`, `TemplateName`, `SpawnElite`, `OnAllSpawnedDead`, `GroupNumber`, `RespawnTime`, `origin`, `angles`, `model`, `DropItem`. São os pares chave/valor da *entity lump* de um BSP da engine Source, ligados por saídas (`On…`) no formato de I/O da Source.

O servidor monta o que o host recebe. `initialize_entity` (`src/domain/mission.rs:2597`) separa spawners (`npc_template_maker`, `unique_…`, `named_boss_…`), rochas, baús, `random_box`, `event_of_trial`, `revenger_event`, props nomeados e o resto. O que não vira objeto do servidor é concatenado na `entity_string` do setor. `QuestSector.entities` segue no `GameStartGranted` (`src/protocol/mission.rs:467`, `src/infrastructure/minimum_mission.rs:64`).

**Hipótese principal (verificar antes de prometer edição de spawns só no servidor):** o host (cliente) instancia os spawners e gatilhos a partir das strings de entidade que o servidor envia, e não da *entity lump* do próprio BSP. Se for verdade, mudar `TemplateName`, `MaxNPCCount`, `SpawnFrequency`, `origin` e o boss de um setor não exige mudar o cliente, desde que o modelo e o template do monstro já existam no cliente. Teste proposto: na cópia de laboratório, trocar o `TemplateName` de um `npc_template_maker` do primeiro setor de `quest_gnoll_scout` (BSP `01h`) por outro monstro existente e ver qual monstro aparece.

### 1.2 Do lado do cliente

- **Mapas:** um BSP por pacote `.hfs`. `.cache/play-client/hfs_extracted/summary.txt` liga cada um dos 2.681 pacotes a uma pasta, por exemplo `0050CB1A…hfs -> maps/16l`. São 284 pastas em `hfs_extracted/maps`, uma por BSP (`maps/01h/01h.bsp`). A cidade é `maps/t01/t01.bsp`.
- **Grafo de navegação:** `maps/graphs/<bsp>.ain` fica solto na pasta do cliente. `DownloadLists/<bsp>.lst` (31 listas) contém só essa linha, por exemplo `"maps\graphs\01a.ain"`.
- **Monstros:** `scripts/npc_variations.txt` (86.417 linhas, KeyValues): modelo, tamanho, regras de queda e invocações por `npc_<classe>` e variação. Modelos em `models/monster/<nome>`, sons em `scripts/game_sounds_heroes_monster.txt`.
- **Textos:** nome da fase pela chave `QuestInfo.STAGE` (`heroes_quest_stage_name_<quest>`) e textos de NPC/quest em `resource/localized_text/english_eu/heroes_text_english_eu.txt` (no HFS `resource`).
- **Banco do cliente:** `QuestInfo`, `QuestDifficultyInfo`, `QuestGoalInfo`, `QuestConstraintInfo`, `MonsterInfo`, `Stage` e `MaterialGroupInfo` existem no `heroes.db3` do cliente. **Hipótese:** o quadro de quests, os níveis exibidos e a barra de HP do boss leem esse banco. Por isso uma quest nova ou um nível mudado precisa da mesma linha no cliente, como já acontece com `QuestDifficultyInfo.MinLevel` em `docs/content-tuning.md`.

### 1.3 Barco, party e quadro

- Fluxo recuperado (`docs/battle-transport.md:7`): `AcceptQuestMessage` → `OpenNewParty` → `RegisterShipInfo` → `StartMicroPlay2`. O servidor recomenda navios em `minimum_mission::recommend_ships` e guarda o estado da party em `minimum_mission::party_state`. Hoje só existe missão solo (pendências do B2 no roadmap).
- O que a quest define: `MaxMemberCount`, `DisableUserShip` (navio próprio permitido), `EnableUserDSMode`, `RequiredItem` e `MaxPlayCount`. Modelos de navio: `ShipYardItemClassInfo` (2 linhas, `ModelName`, `GimicName`).
- **Onde se embarca** (cais, ponto de criar a party): **hipótese:** entidades do BSP da cidade (`t01.bsp`), sem tabela no conteúdo. Mudar o local exige mudar o BSP do cliente.
- **Posição no quadro:** nenhuma coluna de coordenada foi encontrada. O agrupamento vem de `QuestSet`, `QuestGroupID`, `Client` (NPC) e `Level`. **Hipótese:** a ordem e as abas do quadro saem desses campos e do layout Scaleform (`scaleform/*.gfx`). Confirmar no cliente mudando o `QuestSet` de uma quest.

## 2. Três classes de mudança

| Classe | O que muda | Onde aplicar | Custo / risco |
|---|---|---|---|
| A — só servidor | drops (`MonsterDropItemInfo`, `PropItem*`, `NamedProp*`, `DropItemCountMin/Max`), pesos de setor (`Sectors.Weight`), ouro por setor, recompensas de objetivo, EXP/HP no cálculo do servidor e, pela hipótese do 1.1, spawns e boss | sobreposição carregada na partida, como as lojas do B4.4c | baixo; publicação com manutenção |
| B — servidor + `heroes.db3` do cliente | `QuestInfo` (nome, nível, NPC, quadro, membros), `QuestDifficultyInfo`, `QuestGoalInfo`, `MonsterInfo` exibido, quest nova que reusa BSPs existentes | sobreposição + a mesma célula na cópia do cliente (hoje `start-lab.bat`; depois o launcher) | médio; cliente e servidor precisam estar na mesma versão |
| C — assets novos | BSP novo ou alterado (geometria, cais, gatilhos fixos), `.ain` novo, monstro novo (modelo, `npc_variations.txt`), texto novo | pacote `.hfs` refeito ou arquivo solto distribuído pelo launcher | alto: compilar BSP compatível com a engine do cliente, sem SDK oficial; ver [game-launcher.md](game-launcher.md) sobre arquivos soltos |

Quest nova com BSPs existentes (classe B) é o caminho realista para "criar fases". BSP novo (classe C) vira uma investigação separada: formato do BSP do Vindictus, versão do `vbsp`, `.ain` e lightmaps. Fica fora do primeiro corte.

## 3. Requisitos do editor

### 3.1 Funcionais

1. **Lista de fases:** filtro por NPC, `QuestSet`, nível e mapa, com o nome em inglês (`heroes_text_english_eu.txt`).
2. **Ficha da fase (QuestInfo):** NPC, nível, nível de entrega automática, tempo, membros, limite diário, navio próprio, item exigido, história/fase exigida, tabela de drop de props, contagem de drops, modo prática/hardcore. Cada campo mostra o consumidor (servidor, cliente ou ambos), como a tabela de `docs/content-tuning.md`.
3. **Dificuldades:** uma aba por linha de `QuestDifficultyInfo`, com as taxas do boss e dos monstros, nível mínimo/máximo e proporções de EXP/ouro.
4. **Objetivos:** `QuestGoalInfo`, com teste da regex `Target` contra os nomes de monstro e de entidade da fase. A regex do sapo que o motor não compila (pendência do B2) deve ser recusada na validação.
5. **Setores:** sequência com peso por alternativa e BSP de cada setor (só BSPs existentes no cliente), ouro e props por setor, e as ligações de `SectorGroupInfo` como grafo (de → para, gatilho, ponto de spawn).
6. **Entidades do setor:** tabela de spawners (`TemplateName`, `MaxNPCCount`, `MaxLiveChildren`, `SpawnFrequency`, `SpawnElite`, `RespawnTime`, `GroupNumber`, `origin`), boss (`named_boss_npc_template_maker`), baús, `random_box`, rochas e gatilhos. Edição de atributo com tipo. Remover ou acrescentar entidades só depois de confirmada a hipótese do 1.1.
7. **Monstros:** ficha de `MonsterInfo` por classe, variação e dificuldade, com EXP, HP e ataque, mostrando em quais fases o monstro aparece.
8. **Drops do mapa:** `MonsterDropItemInfo` por monstro, quest e dificuldade; props (`PropItemWeightInfo`/`Expectation`, `MaterialGroupInfo`); props nomeados; drops de história. Itens escolhidos pela busca do catálogo (a mesma das lojas do B4.4).
9. **Quest nova por clonagem:** copia todas as linhas ligadas (`QuestInfo`, `QuestDifficultyInfo`, `QuestGoalInfo`, `Sectors`, `SectorGroupInfo`, drops, settings) com um ID novo, mais as chaves de texto novas (`HEROES_QUEST_…`, `heroes_quest_stage_name_<id>`).
10. **Publicação:** pelo ciclo existente (`content_edits` → versão → manutenção agendada do B4.4c). As partes de classe B geram a célula do cliente na mesma versão.

### 3.2 Não funcionais e regras do projeto

- Novos tipos de edição em `content_edits`, como `quest_row`, `quest_difficulty_row`, `sector_row`, `sector_group_row`, `entity_attribute`, `monster_drop_row`, `prop_drop_row`, `monster_row` e `text_entry`. Cada tipo tem validação no `application/content_admin.rs`, sobreposição em `domain/content_overlay.rs` (idempotente, a edição mais nova de um alvo vence) e auditoria na mesma transação.
- O conteúdo SQLite continua versionado separadamente do estado dos personagens (AGENTS.md). Os bancos originais continuam somente leitura.
- Limites: o exportador já recusa mais de 100.000 entidades ou 1.000.000 atributos (`export_npc_scripts.py`). O editor aplica limites por fase (setores, entidades por setor, linhas de drop).
- Toda mudança visível a outros jogadores segue pelo ponto único de autoridade da missão (regra do B5).
- Uma quest com missão em andamento não muda no meio. A versão nova vale para missões criadas depois do reinício.

## 4. Sugestões para tornar a criação prática e autônoma

1. **Clonar fase + assistente:** "nova fase a partir de…" pede ID, nome, NPC, nível, dificuldades e o BSP de cada setor, já com a validação da seção 5.
2. **Simulador:** roda `GetSectorSequence` e a distribuição de drops (`assign_drops`) com semente fixa no servidor e mostra a sequência sorteada, os spawners por setor e a expectativa de drop por item e dificuldade. É o "pré-jogo" sem cliente e usa o mesmo código do domínio, sem reimplementar no navegador.
3. **Pré-visualização do setor:** planta 2D (vista de cima) com os `origin` e `angles` das entidades e com as ligações de setor. Etapa posterior: ler a geometria do BSP extraído (`hfs_extracted/maps/<bsp>/<bsp>.bsp`) e desenhá-la em three.js, como o visualizador do [editor de itens](item-editor.md). Só leitura do asset, nunca alteração.
4. **Validação forte:** BSP existente no cliente, `TemplateName` existente em `npc_variations.txt`, `MonsterInfo` presente para cada classe e dificuldade, item do drop no catálogo, regex compilável, grafo de setores sem setor inalcançável e com saída para a conclusão, `Feature` conhecida.
5. **Diferença lado a lado:** original, versão publicada e rascunho, como a tabela de `content-tuning.md`.
6. **Etiqueta "precisa do cliente":** cada edição mostra se exige atualizar o cliente (classe B/C). A publicação só termina quando o launcher tiver a versão correspondente.
7. **Roteiro de teste gerado:** ao publicar uma fase, o painel gera a entrada do roteiro do bloco (`docs/client-tests/…`) com a pré-condição e os passos.

## 5. Riscos

- A hipótese do 1.1 pode ser falsa (o host pode usar a *entity lump* do BSP): spawns passariam à classe C.
- Cliente e servidor em versões diferentes de `QuestInfo` podem gerar recusas ou diálogos errados. A mitigação é o launcher com versão mínima.
- O GameStartGranted tem limite de corpo (`MAX_MISSION_BODY` = 4 MiB em `minimum_mission.rs:414`); fases com muitas entidades podem estourar.
- BSP novo sem ferramenta comprovada: fora do primeiro corte.
- Sistemas ainda recusados (torre, caça, baús do tesouro, pontos fracos) aparecem no editor só como leitura até serem migrados.

## 6. Engenharia reversa com REA (pedido do operador, 26/10/2026)

O REA serve para confirmar hipóteses pontuais, não para analisar o binário inteiro: a regra do AGENTS.md de 24/10/2026 proíbe a análise completa das DLLs do cliente por carga da máquina. Pontos úteis para este editor:

- **`bin/server.dll` do cliente (16,3 MB):** é a parte "servidor" da engine Source que roda no host. Uma busca de strings (`npc_template_maker`, `TemplateName`, `EntityString`) e a descompilação só das funções que as referenciam diriam se o host usa as entidades enviadas pelo servidor (hipótese do 1.1). Isso exige a autorização do operador para abrir a DLL no REA de forma limitada, sem a análise automática completa.
- **Formato do `.bsp`:** a leitura da *entity lump* e do cabeçalho pode ser feita por script sobre o arquivo extraído, sem REA.

Nada disso foi executado nesta etapa.

## 7. Etapas propostas

| Etapa | Entrega | Classe | Cliente |
|---|---|---|---|
| F1 | painel só leitura: fases, setores, entidades, monstros e drops (a partir de `npc-scripts.json`) | — | não necessário |
| F2 | edição de drops e taxas de fase (classe A) pelo ciclo do B4.4c; parte disso já é o B4.5 (`DropItemCountMin/Max`) | A | conferir o drop |
| F3 | teste da hipótese do 1.1 (troca de `TemplateName`) | — | checkpoint de cliente |
| F4 | edição de spawns e boss (se o F3 confirmar) e simulador | A | trocar o boss de uma fase |
| F5 | clonar fase e editar `QuestInfo`/`QuestDifficultyInfo`/`QuestGoalInfo`, com o cliente atualizado pelo launcher | B | quest nova no quadro, jogada até o fim |
| F6 | estudo do BSP novo (classe C) | C | — |
