# Editor de itens (recurso futuro do painel)

Status: planejamento (26/10/2026). Nada implementado. Objetivo: criar, clonar, editar e remover itens com campos por tipo (equipamento, roupa/fashion, consumível), incluindo ícone e modelo 3D com visualização no painel e ida e volta pelo Blender. **Primeiro grande objetivo: inner armors novos para todas as classes.** O que não foi verificado aparece marcado como **hipótese**.

Encaixe no roteiro: depois do **B4.5** (taxas do servidor: o editor de itens reaproveita os tipos de edição, a sobreposição e a aplicação no cliente que o B4.5 cria para `ItemClassInfo`). A etapa I1 (só conteúdo SQLite) pode começar logo depois do B4.5. Ícones e modelos (I3 em diante) dependem do [launcher](game-launcher.md) (L0 e L3) para chegar ao cliente. Ver "Recursos futuros do painel" em [playable-game-roadmap.md](../playable-game-roadmap.md).

## 1. Onde um item vive

### 1.1 Conteúdo SQLite

Levantado no conteúdo do servidor `ServidorVindictus/Server/zh-tw-x-gm/Bin/heroesContents.db3` e no `.cache/play-client/sql/heroes.db3`, ambos abertos só para leitura.

| Tabela | Linhas (servidor) | Cliente | Papel |
|---|---|---|---|
| `ItemClassInfo` | 19.702 | sim | identidade e regras: `ItemClass`, `Icon`, `IconBG`, `Category`, `TradeCategory`, `TradeCategorySub`, `InventoryFilterName`, `SortOrder`, `Storage`, `MaxStack`, `Price`, `SellPrice`, `ExpireIn`, `Rarity`, `Level`, `RequiredLevel`, `RequiredSkill`, `ClassRestriction`, `Unique`, `Bind`, `TradeOnBind`, `UseInTown`, `UseInQuest`, `Feature`, `ExtraData`… (chave `ItemClass` + `Feature`, `docs/content-tuning.md`) |
| `EquipItemInfo` | 12.377 | sim | equipamento: `EquipClass`, `CostumeType` (liga ao modelo, seção 1.2), `MaxDurability`, `ArmorHP`, `ColorOverride`, `Material1..3`, `ATK`, `ATK_Speed`, `Critical`, `Balance`, `MATK`, `DEF`, `Res_Critical`, `PVP_*`, `STR`, `DEX`, `INT`, `WILL`, `LUCK`, `HP`, `STAMINA`, `TOWN_SPEED`, `Weight`, `EnhanceType`, `QualityType`, `Synthesizable`, `Enchantable`, `Dyeable` |
| `EquipPartInfo` | 201 | sim | partes e slots; inner armor = `('Costume', 1, 'Inner Armor', 30, 'INNERARMOR', 0, 0)` |
| `InnerArmorTypeInfo` | 13 | sim | inner armors oferecidos na criação: `ID`, `Name` (chave de texto), `ClassConstraint`, `SN` (= `CostumeType`), `Weight`, `Icon`. Hoje só Lann (`lethita`), Fiona e Evie |
| `InnerArmorColorInfo` | 16 | sim | cores de inner armor (`R`, `G`, `B`, `Weight`) |
| `DyeableParts` | 109 | sim | canais de tingimento por item |
| `CustomizeItemInfo` / `CustomizeColorInfo` / `CustomizePriceInfo` / `CustomizeSetItemInfo` | 11.349 / … | sim (+ `…New` no cliente) | catálogo do salão de beleza e das roupas |
| `ItemStatInfo`, `StatInfo`, `VariableStatInfo` | — | parte | estatísticas por atributo |
| `SetInfo`, `SetItemInfo`, `SetEffectInfo`, `AvatarSetBonusInfo` | — / … / … / 10 | sim | conjuntos e bônus de avatar |
| `ItemEffectInfo` | 6 | sim | efeito de item (`Identifier`, `Target`, `Amount`, `ItemID`) |
| `FoodInfo` | 169 | sim | consumível → `StatusEffect`, `Level`, `DurationSec`, `Sharing` |
| `StatusEffectGroupInfo` | 2.099 | sim | o que o efeito faz: `Effect1..3` com argumentos, `ServerEffect1..3`, duração, cura, ícone, textos |
| `MicroPlayEffectInfo` | — | não | efeitos usados na missão (`minimum_mission::apply_effect`) |
| `ItemClassOverrideInfo`, `ItemTradeLimitInfo`, `ItemAttributeInfo` | — | sim | ajustes, limites de troca, atributos |
| `ItemClassExtraInfo` (6), `DefaultItemInfo` (80) | — | **não** | só servidor: dados extras e itens iniciais por classe |
| `ShopInfo`, `Cashshop_*`, `CraftShopInfo`, `ManufactureRecipeInfo`, `MonsterDropItemInfo`, `RandomItemV2*` | — | parte | onde o item é vendido, fabricado ou dropado (já editáveis ou planejados: B4.4, B4.5, [editor de fases](stage-editor.md)) |

`ClassRestriction` é uma máscara de bits, conferida nos inner armors por prefixo: Lann (`lethita`) 1, Fiona 2, Evie (`evy`) 4, Karok (`kalok`) 8, Kai (`kay`) 16, Vella 32, Hurk 64, Lynn 128, Arisha 256, Hagie 512, Delia 1024. 2047 = as 11 classes. 909 classes têm `Category='INNERARMOR'` (arisha 93, delia 89, evy 111, fiona 113, hagie 48, hurk 37, kalok 68, kay 69, lethita 82, lynn 86, vella 93, `innerfree` 3, `cash` 17).

### 1.2 Cliente: scripts, ícones, textos e modelos

- **Item → modelo:** `scripts/player_costume.txt` (57.633 linhas, KeyValues, no HFS de `scripts`) tem as seções `base`, `inner_armor`, `face`, `hair`, `upper_body`, `lower_body`, `head`, `foot`, `hand`, `rear`, `tail`, `motion` e `common_motion`. Em `inner_armor` há um bloco por classe (`default` e `color1..3`) e as entradas numeradas, uma por `CostumeType`, cada uma com `category` (corpo: `female` 307, `male` 193, `giant` 70, `tall` 59), `label` e `normal` (o `.mdl`). Exemplo conferido: `fiona_inner_armor03` → `EquipItemInfo.CostumeType` 12 → `"12" { "category" "female" "label" "6" "normal" "models/player/fiona/fiona_inner_armor03.mdl" }`. O `CostumeType` dos inner armors vai de −1 a 90.375 (400 valores distintos). As entradas 10–14 servem a Fiona e a Evie (`InnerArmorTypeInfo` `fiona_innerarmor01…05`), então a numeração é global (**hipótese** de que o cliente filtra pelo `category` do corpo da classe).
- **Consumíveis no cliente:** `scripts/item_data.txt` (6.491 linhas) define o comportamento ao usar, por exemplo `life_potion_lvl1 { type health_potion; hp 500; pre_sequence potion_begin; can_double_up 1 … }`. Tipos mais comuns: `spskill_base_status_effect` 118, `status_effect_potion` 70, `town_visual_effect` 29, `artifact_status_effect` 20, `health_potion` 5. `scripts/useitem.txt` (51 linhas) liga itens usáveis a comandos de console (`use_item_extrastorage {0}`, `cc_enter_dyeshop_cash`…).
- **Ícones:** `scaleform/item_icon/<ItemClassInfo.Icon>.tga`. O `scripts/content/export_panel_catalog.py` já os converte para PNG em `.cache/private-content/icons/item` para o painel.
- **Textos:** `HEROES_ITEM_NAME_<CLASSE>` e `HEROES_ITEM_DESC_<CLASSE>` em `resource/localized_text/english_eu/heroes_text_english_eu.txt` (lido por `export_cash_shop.py` e `export_item_catalog.py`).
- **Modelos:** engine Source. Em `hfs_extracted/models/armor/kingdom/` estão `pc_male_kingdom_foot.mdl`, `.vvd`, `.dx80.vtx`, `.dx90.vtx` e `.phy`, mais a variante `_broken` (peça quebrada). O cabeçalho do `.mdl` é `IDST` versão **44**, com o nome interno `armor\kingdom\pc_male_kingdom_foot.mdl`; o `.vvd` é `IDSV` versão 4. Inner armors e corpos ficam em `models/player/<classe>/` (`arisha`, `darkknight`, `delia`, `evy`, `fiona`, `hagie`, `hurk`, `kalok`, `kay`, `lethita`, `lynn`, `paladin`, `vella`, mais `_anim`). Armaduras comuns são por corpo (`pc_male_`, `pc_female_`, `pc_giant_`, `pc_tall_`). Texturas em `materials/models/armor/<conjunto>/*.vtf` + `.vmt` (`_diffuse`, `_normal`).

### 1.3 Servidor Rust (o que já consome)

- `item-catalog.json` (`export_item_catalog.py`: classes, `max_stack`, nomes), `item-economy.json` (`ExpireIn`, `RequiredLevel`, preços), `item-enhance.json`, `equipment-profile` (`export_equipment_profile.py`), `item_stats`, `item_colors` e `random_item`: os exportadores leem `ItemClassInfo`, `EquipItemInfo` e afins do conteúdo do servidor. Documentos: `docs/item-catalog.md`, `docs/item-stats.md`, `docs/item-colors.md`, `docs/character-costumes.md`, `docs/initial-costume.md`.
- Uso de consumível na missão: `minimum_mission::use_consumable` e `apply_effect` (`src/infrastructure/minimum_mission.rs:1323`, `:1400`) com `MicroPlayEffectInfo`. Na cidade: os usáveis do B1.6.
- Lojas e cash shop já aceitam edições versionadas (B4.4). O painel já mostra o catálogo com ícones (B4.4b).

## 2. Campos do editor por tipo

| Tipo | Campos (tabela) | Exige cliente? |
|---|---|---|
| Comum a todos | ID (`ItemClass`), nome e descrição (texto), ícone, `Category`/`TradeCategory`/`Sub`, `Rarity`, `Level`, `RequiredLevel`, `ClassRestriction` (caixas por classe), `MaxStack`, `Price`/`SellPrice`, `ExpireIn`, `Bind`/`TradeOnBind`, `Unique`, `UseInTown`/`UseInQuest`, `Feature` (`ItemClassInfo`) | sim: o cliente tem `ItemClassInfo` |
| Equipamento | `EquipItemInfo` completo (atributos e estatísticas, `MaxDurability`, `EnhanceType`, `QualityType`, `Enchantable`, `Synthesizable`), cor padrão (`ColorOverride`, `DyeableParts`, `color1..3` do script), `CostumeType` → modelo por corpo (`player_costume.txt`), materiais de reparo, conjunto (`SetItemInfo`), venda em loja (ShopInfo, B4.4), receita (`ManufactureRecipeInfo`) e drop | sim (tabelas, script, modelo, ícone) |
| Roupa / fashion / inner armor | o mesmo do equipamento com `EquipClass='CUSTOMIZE'` e categoria `INNERARMOR` ou parte de avatar; `CustomizeItemInfo` (salão), `InnerArmorTypeInfo` (criação), `AvatarSetBonusInfo` | sim |
| Consumível | `FoodInfo` (efeito, nível, duração, compartilhar), `StatusEffectGroupInfo` (o que faz: `Effect1..3` e `ServerEffect1..3` com argumentos), `ItemEffectInfo`, `MicroPlayEffectInfo` (missão), bloco em `item_data.txt` (tipo, `hp`, animações, regras de dose dupla) e `useitem.txt` quando o uso abre uma tela ou comando | sim: o efeito visível e a animação são do cliente |

Regras gerais: um item novo só é publicado quando existir no servidor **e** no cliente na mesma versão (o item desconhecido no cliente é **hipótese** de quebra de inventário). Remover um item é recusado enquanto houver instâncias em inventários, lojas, receitas ou drops; o editor lista as referências e oferece "retirar de venda/drop" em vez de apagar.

## 3. Caminho completo: inner armors novos para todas as classes

### 3.1 Corte 1, sem modelo novo (só conteúdo, I1–I2)

Reaproveita modelos que já existem, com estatísticas, cores, nome e ícone novos.

1. **Servidor (`heroesContents`, sobreposição):** linha nova em `ItemClassInfo` (`Category='INNERARMOR'`, `TradeCategory='EQUIPMENT'`, `TradeCategorySub='CUSTOMIZE'`, `Icon`, `ClassRestriction` da classe-alvo, `Price`) e em `EquipItemInfo` (`EquipClass='CUSTOMIZE'`, `CostumeType` de um inner armor existente daquela classe, estatísticas, `Dyeable`, `ColorOverride`).
2. **Cliente (`heroes.db3`):** as mesmas linhas, pelo mesmo caminho do B4.5 (start-lab hoje, launcher depois).
3. **Texto:** `HEROES_ITEM_NAME_/DESC_<CLASSE>` novos. Exige distribuir o `heroes_text_english_eu.txt` alterado: arquivo de HFS, depende do teste L0-a/L0-d do [launcher](game-launcher.md). Antes disso, o item aparece sem nome (**hipótese**).
4. **Ícone:** reusar um `.tga` existente (só a coluna `Icon`) ou um novo (L0-a/L0-c).
5. **Entrega:** loja (B4.4), cash shop, painel "dar item" (B4.3), criação de personagem (`InnerArmorTypeInfo` + `DefaultItemInfo`, mexe na criação, decisão à parte).
6. **Teste:** dar o item pelo painel, vestir, conferir modelo, cor e estatísticas; relogar.

### 3.2 Corte 2, modelo novo por classe (I3–I5)

Para cada uma das 11 classes (corpo e esqueleto próprios em `models/player/<classe>/`):

1. Modelar no Blender sobre o esqueleto da classe, importado do `.mdl` do corpo (ex.: `models/player/fiona/fiona.mdl`) e do inner armor existente, para herdar ossos, pesos e proporções.
2. Exportar `.mdl`/`.vvd`/`.vtx`/`.phy` v44 + a variante `_broken`, se o jogo a exigir para inner armor (**hipótese**: o `_broken` aparece em armaduras comuns), e as texturas `.vtf`/`.vmt` com os nomes que o `.mdl` referencia.
3. Entrada nova em `player_costume.txt` → `inner_armor` → `"<CostumeType novo>" { category <corpo>; label …; normal "models/player/<classe>/<novo>.mdl" }`, com um número livre (o editor reserva uma faixa própria, por exemplo ≥ 100.000, acima do maior atual, 90.375).
4. Linhas de `ItemClassInfo`/`EquipItemInfo` com esse `CostumeType`, no servidor e no cliente.
5. Distribuição: modelo, texturas, `player_costume.txt` e texto pelo launcher (soltos, se o L0 provar a sobreposição; senão, HFS refeito).
6. Opcional: oferecer na criação (`InnerArmorTypeInfo`), que hoje só cobre Lann, Fiona e Evie.

Custo estimado: conteúdo baixo; asset alto (11 modelos com rig de corpo diferente); servidor baixo (o catálogo já é dirigido por conteúdo); binário do cliente nenhum, se o `CostumeType` alto for aceito (**hipótese**: limite de número ou tamanho da tabela no `client.dll`, ver seção 5).

## 4. Modelos 3D: visualização no painel e Blender 5.1

### 4.1 Formato

- Família Source: `.mdl` (cabeçalho, ossos, bodygroups, materiais, sequências e referência aos outros arquivos), `.vvd` (vértices, normais, UV, até 3 pesos de osso por vértice e tangentes), `.vtx` (malha otimizada por hardware: `dx80` e `dx90`), `.phy` (colisão e ragdoll); texturas `.vtf` + materiais `.vmt` (texto). Os quatro arquivos levam o mesmo *checksum* no cabeçalho e precisam bater (regra da Source; **hipótese** de que o Vindictus mantém).
- Versão **44** conferida (`IDST`, 44). É a família da Source 2006/Episode One; ferramentas atuais geram 48/49. Pode haver campos próprios do Vindictus (**hipótese**).
- Dentro do HFS os arquivos ficam com o caminho normal (`hfs_extracted/models/...`), e a extração já existe (`Tools/HFSExtract`, `vZipFlip_HFS`, `vindictusmdlsorter`).

### 4.2 Ferramentas conhecidas (nenhuma executada nesta etapa)

- **Crowbar** (decompilar `.mdl` → `.smd`/`.qc` e compilar via `studiomdl`); tem perfil para Vindictus segundo a comunidade (**hipótese**, conhecimento externo não verificado aqui).
- **Blender Source Tools** (importa e exporta SMD/DMX e chama o `studiomdl`), **SourceIO** (add-on que importa `.mdl`/`.vtf`/`.bsp` direto; suporte a v44 e Vindictus é **hipótese**), **VTFEdit/VTFLib** (texturas).
- `studiomdl` com saída v44: o do Source SDK 2006/Episode One (**hipótese**; precisa de teste de compilação e carga no cliente).

### 4.3 Recomendação de arquitetura (modelo do pw-universal)

O pw-universal-server do operador resolveu o mesmo problema assim (lido em `docs/admin/RECURSOS_VERSIONAMENTO_E_ATUALIZADOR.md` §8.3–§8.4 e `tools/blender/pw_estudio/`). A conversão formato do jogo ↔ glTF fica no servidor (`crates/pw-recursos/src/estudio.rs`, `importar.rs`). O painel mostra o GLB num visualizador web. A extensão do Blender (`blender_manifest.toml`, `blender_version_min = "4.2.0"`) **não escreve o formato do jogo**: ela valida o modelo contra as regras do formato (limites de vértices e ossos, pesos por vértice, nomes, ganchos, FPS) e exporta o GLB que o servidor espera.

Para o Vindictus:

1. **Servidor (Rust):** leitor `.mdl`/`.vvd`/`.vtx` v44 → glTF (malha, ossos, pesos, UV, materiais convertidos de `.vtf` para PNG), limitado (tamanho e contagens) e com testes contra modelos reais extraídos. Depois, o escritor glTF → `.mdl`/`.vvd`/`.vtx` v44 (com o *checksum* coerente e as duas variantes de `.vtx`) ou, como alternativa, a geração de `.smd` + `.qc` e uma chamada ao `studiomdl` v44 numa ferramenta separada.
2. **Painel:** visualizador three.js do GLB (girar, trocar corpo/classe, cores `color1..3`, comparar com o original), importação de GLB com relatório de validação e exportação do GLB do item para editar.
3. **Extensão Blender 5.1** (formato de extensão com `blender_manifest.toml`, como o `pw_estudio`): importar o GLB do painel (com o esqueleto da classe), conferir as regras da Source (≤ 3 pesos por vértice, ossos com o nome do esqueleto da classe, ≤ 128 ossos por malha em `.vtx` (**hipótese** de limite), um material por grupo com nome de `.vmt`, escala e eixo), corrigir o que não exige decisão do artista e exportar o GLB. Opcional: um modo "Source direto" que lê `.mdl` v44 (reaproveitando o SourceIO, se suportar) para quem tem o arquivo extraído.
4. **O que o leitor ou escritor precisa cobrir:** cabeçalho `studiohdr_t` v44, ossos (nome, pai, posição e rotação, matriz de pose), *bodyparts*/modelos/malhas, `texture` e `cdtexture` (caminhos de material), *skin families*, *attachments*, *hitboxes* (para dano: **hipótese** de que a armadura não as usa), `.vvd` (fixups de LOD, vértices e tangentes), `.vtx` (strips, strip groups, LODs). Sequências e animações não são necessárias para inner armor, que usa as animações do corpo (`models/player/_anim`).

## 5. Engenharia reversa com REA (pedido do operador, 26/10/2026)

Pontual, dentro da regra de 24/10 contra análise completa das DLLs do cliente:

- **`client.dll` (14,7 MB):** só busca de strings e descompilação das funções que leem `player_costume.txt` (`inner_armor`, `category`, `normal`, `label`) e `item_data.txt`. Responderia ao maior `CostumeType` aceito, ao filtro por corpo e ao que acontece com um `ItemClass` desconhecido. Precisa de autorização do operador para abrir a DLL no REA sem análise automática completa.
- **Nada disso substitui o teste no cliente:** para o corte 1 basta o L0 do launcher e um item clonado. O REA entra se um teste falhar sem explicação.
- O REA não lê o launcher do Windictus (proteção de metadados, ver [game-launcher.md](game-launcher.md)).

## 6. Riscos

- Itens novos sem a linha correspondente no cliente: tooltip vazio ou recusa (**hipótese**). Mitigação: publicação conjunta pelo launcher.
- Texto e ícone vivem em HFS; sem a sobreposição de soltos (L0-a), cada mudança exige refazer HFS.
- Escrever `.mdl` v44 corretamente é o maior risco técnico. O corte 1 (sem modelo novo) entrega valor sem ele.
- Remoção de item com instâncias vivas: sempre recusada.
- Assets novos são do operador. Os originais do jogo nunca entram no Git (AGENTS.md).

## 7. Etapas propostas

| Etapa | Entrega | Cliente |
|---|---|---|
| I1 | depois do B4.5: painel de itens só leitura com todos os campos da seção 2, referências (lojas, drops, receitas, inventários) e ícone | não necessário |
| I2 | criar, clonar e editar itens no conteúdo (servidor + `heroes.db3` do cliente), com modelo existente: corte 1 dos inner armors | inner armor clonado com estatísticas e cor novas, vestido e relogado |
| I3 | textos e ícones novos distribuídos (depende de L0 e L3 do launcher) | nome e ícone novos no inventário |
| I4 | leitor `.mdl` v44 → glTF no servidor e visualizador no painel | não necessário |
| I5 | importar GLB, gerar `.mdl` v44 e entrada no `player_costume.txt`; extensão Blender 5.1 | inner armor com modelo novo em uma classe, depois nas 11 |
| I6 | consumíveis: efeito (`FoodInfo`/`StatusEffectGroupInfo`) e `item_data.txt` | poção nova usada na cidade e na missão |
