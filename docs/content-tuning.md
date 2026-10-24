# Valores de jogo ajustáveis (base para o painel B4)

Onde ficam, no conteúdo, os números que definem drop, EXP, títulos, níveis e pilhas, e por onde eles chegam ao servidor e ao cliente. O painel de administrador (bloco B4) deve editar estes pontos. Hoje eles são aplicados pela camada `scripts/content/content_overrides.py` (servidor) e por `scripts/client/apply_original_content.py` (cópia do cliente).

## Fontes

| Fonte | Caminho | SHA-256 | Papel |
|---|---|---|---|
| Conteúdo do servidor (modificado) | `ServidorVindictus/Server/zh-tw-x-gm/Bin/heroesContents.db3` | `fe0cf8bcef27…` | fonte amarrada de todos os perfis (o hash dele fica nos manifestos) |
| Conteúdo original do servidor | `TutorialRagezone/DB/server/heroesContents.db3` | `5d367666fd006ffd670e1fc85cfeb087006c3fa89522d108fb60a95925055781` | valores originais das colunas abaixo |
| Banco original do cliente | `TutorialRagezone/DB/client/sql/heroes.db3` | `c22ec7fe22f948250fc0e7dacf82acfdfe547141b03a0a6f10a1bda3cc436188` | valores originais aplicados na cópia `.cache/play-client/sql/heroes.db3` |

Os caminhos são relativos a `D:\PROJETOS\VindictusPrivate`. As três bases são somente leitura; a cópia do cliente guarda o estado anterior em `heroes.db3.pre-original-content`. Mais bancos de referência: `docs/client-compatibility.md`, seção "Bancos de conteúdo de referência".

## Pontos ajustáveis aplicados

| Tabela | Coluna | Chave da linha | O que controla | Modificado → original | Servidor (perfil → uso no Rust) | Cliente |
|---|---|---|---|---|---|---|
| `QuestInfo` | `DropItemCountMin`, `DropItemCountMax` | `ID` | quantos monstros comuns de uma fase recebem drop (sorteio Min..Max no `GetSectorSequence`) | 20/20 → 1/2 (269 quests) | `npc-scripts.json` `mission.quests` [3], [4] → `QuestDigest.item_count_min/max` → `MissionContents::assign_drops` | exibe e compara com o próprio banco |
| `QuestInfo` | `MaxPlayCount` | `ID` | limite de jogadas por dia | sem limite → ex.: 1 (55 quests) | `npc-scripts.json` `quests.quests` `MaxPlayCount` → contagem de jogadas do quadro | quadro de quests |
| `QuestInfo` | `QuestSet` | `ID` | conjunto de quests (agrupamento/quadro) | ex.: 1 → 2000 (5 quests) | `npc-scripts.json` `quests.quests` `QuestSet` | quadro de quests |
| `QuestInfo` | `DisableUserShip`, `EnableUserDSMode` | `ID` | se a quest aceita navio próprio / servidor dedicado de usuário | False → True (5 quests de dragão) | `npc-scripts.json` `mission.quests` | criação de navio |
| `QuestDifficultyInfo` | `MinLevel` | `QuestID`, `Difficulty` | nível mínimo por dificuldade | 1 → ex.: 50 (867 linhas) | `npc-scripts.json` `quests.difficulties` [2] → disponibilidade da quest por nível (`QuestOwner`) | o banco do cliente já tem o original |
| `StoryLineInfo` | `RewardExp` | `StoryLineID` | EXP da conclusão da história (vezes `ExpRate_Story`) | ex.: 2400 → 800 (10 histórias) | `npc-scripts.json` `storylines[].info.RewardExp` → `complete_storyline` | diário de histórias |
| `TitleGoalInfo` | `TargetCount` | `ID` | quantas ações a meta de título exige | 1 → 10…1000 (440 metas; aranha com chute 50, chute 25, agarrão 10/100/300/1000) | `npc-scripts.json` `progression.title_goal_targets` [2] → `Progression::proceed` | contador "x/N" da janela de títulos |
| `ItemClassInfo` | `MaxStack` | `ItemClass`, `Feature` | pilha máxima | 1000 → 100 (5.148 classes no servidor, 10 no cliente) | `item-catalog.json` `classes[].max_stack` → planejador de entrega e mover/empilhar | empilhar no inventário |
| `ItemClassInfo` | `ExpireIn` | `ItemClass`, `Feature` | validade em segundos ao criar o item | ex.: sem validade → 43200 (2 classes) | `item-economy.json` `classes[].expire_in` → `Item.Initialize` (validade) | tooltip |
| `ItemClassInfo` | `RequiredLevel` | `ItemClass`, `Feature` | nível para usar/equipar | 1 → 8 (`test_luck_150`) | `item-economy.json` `classes[].required_level` | — |
| `EnhanceServerInfo` | `SuccessRatio` | `EnhanceType` (o nível como texto), `EnhanceLevel`, `Feature` | chance de sucesso da melhoria por nível; é a que o `EnhanceItemProcessor` usa (sobrescreve a do `EnhanceInfo`) | 1.0 → 1.0 nos níveis 1–3, 0,75 (4–5), 0,5 (6–8), 0,4 (9–12), 0,33 (13–15) nas linhas `!zhCN` (12 linhas; o original guarda o valor como texto `'.33'` e o SQLite.NET do servidor lê `REAL` como `Single`) | `item-enhance.json` `server[].success_ratio` → `item_enhance::Contents::server_ratio` | o banco do cliente não tem a tabela |
| `EnhanceInfo` | `SuccessRatio` | `EnhanceType`, `EnhanceLevel`, `Feature` | chance exibida/da tabela por tipo de equipamento; no servidor o valor é trocado pelo do `EnhanceServerInfo` | 1.0 → ex.: 0,75/0,33 (2.210 linhas) | `item-enhance.json` `enhance[].success_ratio` (lido, não decide o sorteio) | `scripts/client/apply_original_content.py` aplica as 2.210 células na cópia |
| `EnchantInfo` | `MinSuccessRatio`, `MaxSuccessRatio` | `EnchantClass` | chance de sucesso do encantamento no início e no fim do minijogo (em %; `EnchantSuccessBonus` soma) | 100/100 → os valores originais, ex.: 25/60, 25/40, 35/74 (163 linhas) | `item-enhance.json` `enchant[].min_ratio`/`max_ratio` → `enchant::success_ratio` | `scripts/client/apply_original_content.py` aplica as células na cópia |

Notas:
- `EnhanceInfo` e `EnhanceServerInfo`: as mesmas chaves (`EnhanceType`, `EnhanceLevel`, `Feature`) nas duas bases; só `SuccessRatio` difere. `EnhanceStatInfo`, `EnhanceBonusServerInfo`, `EnhanceInsteadMaterialInfo` e `RepairEnhanceInfo` são iguais.
- `QuestInfo`, `QuestDifficultyInfo`, `StoryLineInfo` e `TitleGoalInfo` têm as mesmas chaves, na mesma ordem, nas duas bases. Em `ItemClassInfo` a chave é o par (`ItemClass`, `Feature`), porque `ItemClass` se repete; uma linha sem par (`hagie_noluck_inner_free`/`hagie_noluck_inner_armor`) mantém o valor do servidor.
- Os perfis `fiona-inventory` (entrega inicial) e `normal-equipment-profile` ainda leem `ExpireIn`/`RequiredLevel` do conteúdo do servidor: as únicas diferenças que veriam são dois `ExpireIn` e o `RequiredLevel` de um item de teste, e esses perfis estão amarrados à criação de personagem.

## Iguais nas duas bases (já originais)

- `MonsterInfo`: EXP (`Exp`), HP e dificuldade dos monstros.
- `MonsterDropItemInfo`: itens, pesos (`Weight`), absolutos (`IsAbsolute`) e drops únicos dos monstros.
- `PropItemWeightInfo`, `PropItemExpectationInfo`, `NamedPropItemInfo`, `NamedPropDropAmountInfo`: drops de props.
- `QuestGoalInfo`, `TitleInfo`, `TitleStatInfo`, `LevelUpExpInfo`, `CharacterClassInfo`.
- `FeatureMatrix`: taxas globais `ExpRate_Combat`, `GoldRate_Combat`, `APRate_Combat`, `ExpRate_Story`, `GoldRate_Story`, `APRate_Story`, `ExpRate_Monster` (só as URLs de `EndPoints` diferem).

Esses também são candidatos a ajuste no painel (por exemplo, as taxas do `FeatureMatrix` multiplicam EXP, ouro e AP), mas hoje já correspondem ao original.

## Diferenças ainda não usadas pelo servidor

Tabelas alteradas no conteúdo do servidor que pertencem a sistemas do B3 ou de cosméticos, sem consumidor no Rust: `ManufactureRecipeInfo` (1.695), `CustomizeItemInfo`/`CustomizePriceInfo` (salão de beleza), `PetInfo` (36), `EquipItemInfo` (4), `EntityAttribute` (5). Quando esses sistemas forem migrados, decidir a fonte de cada uma e acrescentar aqui.

## Como reaplicar

```powershell
py -3.13 scripts/content/export_item_catalog.py --output .cache/private-content/item-catalog.json
py -3.13 scripts/content/export_item_economy.py --output .cache/private-content/item-economy.json
py -3.13 scripts/content/export_npc_scripts.py --output .cache/private-content/npc-scripts.json
py -3.13 scripts/content/export_item_enhance.py --output .cache/private-content/item-enhance.json
py -3.13 scripts/client/apply_original_content.py --client-copy .cache/play-client --apply
docker compose up -d --build --wait
```
