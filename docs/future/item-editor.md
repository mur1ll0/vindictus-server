# Editor de itens (recurso futuro do painel)

Status: corte 1 e editor no painel (corte 2, seção 3.3) implementados em 27/10/2026; teste do editor no cliente pendente. Objetivo: criar, clonar, editar e remover itens com campos por tipo (equipamento, roupa/fashion, consumível), incluindo ícone e modelo 3D com visualização no painel e ida e volta pelo Blender. **Primeiro grande objetivo: inner armors novos para todas as classes.** O que não foi verificado aparece marcado como **hipótese**.

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

`ClassRestriction` é uma máscara de bits, conferida nos inner armors por prefixo: Lann (`lethita`) 1, Fiona 2, Evie (`evy`) 4, Karok (`kalok`) 8, Kai (`kay`) 16, Vella 32, Hurk 64, Lynn 128, Arisha 256, Hagie 512, Delia 1024. 2047 = as 11 classes do conteúdo. O cliente 1.69, porém, só conhece **10** classes: a tabela do `client.dll` (`0x38cafd50`, com o limite `cmp eax, 0xa` em `0x3879ec68`) traz Lethita, Fiona, Evy, Kalok, Kay, Vella, Hurk, Lynn, Arisha e Hagie. Itens de Delia existem no banco, mas a classe não existe neste cliente (coerente com `CHARACTER_COUNT = 10` em `docs/b4-painel-plano.md`). Inner armors novos "para todas as classes" significam as 10. 909 classes têm `Category='INNERARMOR'` (arisha 93, delia 89, evy 111, fiona 113, hagie 48, hurk 37, kalok 68, kay 69, lethita 82, lynn 86, vella 93, `innerfree` 3, `cash` 17).

### 1.2 Cliente: scripts, ícones, textos e modelos

- **Item → modelo:** `scripts/player_costume.txt` (57.633 linhas, KeyValues, no HFS de `scripts`) tem as seções `base`, `inner_armor`, `face`, `hair`, `upper_body`, `lower_body`, `head`, `foot`, `hand`, `rear`, `tail`, `motion` e `common_motion`. Em `inner_armor` há um bloco por classe (`default` e `color1..3`) e as entradas numeradas, uma por `CostumeType`, cada uma com `category` (corpo: `female` 307, `male` 193, `giant` 70, `tall` 59), `label` e `normal` (o `.mdl`). Exemplo conferido: `fiona_inner_armor03` → `EquipItemInfo.CostumeType` 12 → `"12" { "category" "female" "label" "6" "normal" "models/player/fiona/fiona_inner_armor03.mdl" }`. O `CostumeType` dos inner armors vai de −1 a 90.375 (400 valores distintos). As entradas 10–14 servem a Fiona e a Evie (`InnerArmorTypeInfo` `fiona_innerarmor01…05`): a numeração é global por seção. O parser no `client.dll` foi lido (seção 5): 12 seções reconhecidas (`base`, `inner_armor`, `face`, `hair`, `upper_body`, `lower_body`, `head`, `foot`, `hand`, `rear`, `motion`, `common_motion`; a seção `tail` do arquivo não está na lista do 1.69), blocos de classe só para 10 nomes e categorias `None`/`Male`/`Female`/`Giant`/`Tall`.
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
| Consumível | `FoodInfo` (efeito, nível, duração, compartilhar), `StatusEffectGroupInfo` (o que faz: `Effect1..3` e `ServerEffect1..3` com argumentos), `ItemEffectInfo`, `MicroPlayEffectInfo` (missão), bloco em `item_data.txt` (tipo, `hp`, animações, regras de dose dupla) e `useitem.txt` quando o uso abre uma tela ou comando | sim: o efeito visível e a animação são do cliente. Os 26 valores de `type` usados no `item_data.txt` (`health_potion`, `status_effect_potion`, `repair_kit`…) estão como texto no `bin/server.dll` do cliente (a parte host da engine) e não no `client.dll`: são classes C++ fixas. Um consumível novo escolhe um `type` existente e ajusta os parâmetros; um comportamento novo exige mudar o binário |

Regras gerais: um item novo só é publicado quando existir no servidor **e** no cliente na mesma versão (o item desconhecido no cliente é **hipótese** de quebra de inventário). Remover um item é recusado enquanto houver instâncias em inventários, lojas, receitas ou drops; o editor lista as referências e oferece "retirar de venda/drop" em vez de apagar.

## 3. Caminho completo: inner armors novos para todas as classes

### 3.1 Corte 1, sem modelo novo (só conteúdo, I1–I2)

Reaproveita modelos que já existem, com estatísticas, cores, nome e ícone novos.

1. **Servidor (`heroesContents`, sobreposição):** linha nova em `ItemClassInfo` (`Category='INNERARMOR'`, `TradeCategory='EQUIPMENT'`, `TradeCategorySub='CUSTOMIZE'`, `Icon`, `ClassRestriction` da classe-alvo, `Price`) e em `EquipItemInfo` (`EquipClass='CUSTOMIZE'`, `CostumeType` de um inner armor existente daquela classe, estatísticas, `Dyeable`, `ColorOverride`).
2. **Cliente (`heroes.db3`):** as mesmas linhas, pelo mesmo caminho do B4.5 (start-lab hoje, launcher depois).
3. **Texto:** `HEROES_ITEM_NAME_/DESC_<CLASSE>` novos. Exige distribuir o `heroes_text_english_eu.txt` alterado. Ele está no índice HFS, mas `resource/` não é prefixo bloqueado: desativar o HFS de `resource` e soltar a pasta inteira funciona pelo código (o mesmo truque já usado no `scaleform/`, ver [launcher](game-launcher.md) seção 2.2); o teste L0-b confirma. Antes disso, o item aparece sem nome (**hipótese**).
4. **Ícone:** reusar um `.tga` existente (só a coluna `Icon`) ou um novo com nome novo em `scaleform/item_icon/`, solto (fora do índice e fora dos prefixos bloqueados; teste L0-c).
5. **Entrega:** loja (B4.4), cash shop, painel "dar item" (B4.3), criação de personagem (`InnerArmorTypeInfo` + `DefaultItemInfo`, mexe na criação, decisão à parte).
6. **Teste:** dar o item pelo painel, vestir, conferir modelo, cor e estatísticas; relogar.

**Levantamento de 26/10/2026 para executar o corte 1:**

- **Item candidato:** o `player_costume.txt` tem a entrada 16 da Fiona (`fiona_inner_fringe.mdl`, modelo presente no índice), mas nenhum item a usa. Só existe `fiona_inner_fringe_socks` (CostumeType 24). Um item novo `fiona_inner_fringe` com `CostumeType` 16 usa esse modelo **sem mudar o `player_costume.txt`**. As tabelas que um inner ocupa no servidor são `ItemClassInfo` (1 linha), `EquipItemInfo` (2 linhas) e, para customização, `CustomizeDefaultInfo` e `CustomizePriceInfo`.
- **Restrição do servidor:** ele não lê o `heroesContents.db3`, e sim os arquivos exportados por `scripts/content/export_*.py`. Esses exportadores fazem `SELECT *` no banco original, conferem que a ordem das linhas bate entre eles (por exemplo, `export_item_stats` contra `export_item_catalog`) e gravam o hash do banco. O servidor recusa exportações de hashes diferentes, e isso inclui a prova de nomes (`character_name_proof`, gerada por oráculo). Logo, trocar o banco por uma cópia aumentada obriga a reexportar tudo e refazer a prova.
- **Desenho proposto:** uma camada de linhas adicionadas, como a `lab_content_additions.json` já usada nas lojas. As mesmas linhas de `ItemClassInfo`/`EquipItemInfo` são acrescentadas no fim da projeção de cada exportador de item (catálogo, estatísticas, cores, economia, perfil de equipamento, catálogo do painel e nomes), mantendo o hash do banco de origem. No cliente, `apply_lab_content_additions.py` grava as linhas no `heroes.db3`, o texto entra pelo pacote refeito (`scripts/client/hfs.py`) e o ícone vai solto.
- **Pendente:** decidir o momento. A camada toca exportadores compartilhados, e em 26/10/2026 outra sessão corrigia a criação de personagem e o Check de nome (com mudanças em `export_fiona_initializer.py` e em `src/`).

**Implementação de laboratório (26/10/2026):**

- **Servidor:** `scripts/content/content_additions.py` abre o `heroesContents.db3` do servidor só para leitura e põe na frente de `ItemClassInfo`, `EquipItemInfo`, `CustomizeDefaultInfo` e `CustomizePriceInfo` uma `TEMP VIEW` com as linhas originais seguidas das adicionadas. As adicionadas copiam todas as linhas do item-modelo, com `ItemClass` trocado e as colunas de `set` alteradas. Os 18 exportadores usam `content_additions.connect`: o arquivo e o hash do banco não mudam, e todas as projeções ganham as mesmas linhas no fim. Reexportados 13 arquivos (`item-catalog`, `item-stats`, `item-colors`, `item-economy`, `item-enhance`, `npc-scripts`, `random-item`, `cash-shop`, `item-names`, `panel-catalog`, `normal-equipment-profile-v4`, `fiona-appearance-inventory` e `fiona-inventory`). A comparação com os anteriores mostrou só acréscimos. Os dois últimos são obrigatórios: o servidor confere que a aparência e o inventário têm o tamanho do catálogo (sem eles, `invalid private minimum creation assets`). Backup dos anteriores em `.cache/private-content-items-backup`.
- **Itens (`scripts/content/item_additions.json`):**
  - `fiona_inner_fringe` (corte 1): `CostumeType` 16 (`fiona_inner_fringe.mdl`, modelo e ícone já existentes) e texto novo.
  - `fiona_inner_zero` (teste do corte 2): `CostumeType` 9100 e entrada nova no `inner_armor` do `player_costume.txt`. O modelo e as texturas foram fornecidos pelo operador e ficam em `.cache/private-assets/inner_zero`, fora do Git. O modelo vai num pacote novo, parte 14 de `models/player/fiona`, e as texturas na pasta nova `materials/models/player/fiona/fiona_inner_zero`. **Atenção:** o `.mdl` é da versão 48, e os 16.802 modelos do 1.69 são da 44. VVD v4, VTX v7 e VTF 7.2 são iguais aos do 1.69. Se o motor recusar a v48, o modelo não aparece; nesse caso, a próxima etapa é converter o cabeçalho ou recompilar para v44.
- **Cliente:** `scripts/client/apply_lab_content_additions.py --apply` (também rodado pelo `start-lab.bat`) grava as linhas no `sql/heroes.db3`. Ele refaz, a partir de `lab-backup/`, os pacotes de texto em inglês (nome e descrição) e o de scripts (`player_costume.txt`), cria os pacotes dos arquivos do item e sincroniza o índice HFS (`scripts/client/hfs.py`, cujo XOR passou a ser feito por inteiro grande: 2,5 s para tudo). Sem itens, ele volta aos originais e apaga os pacotes criados. A tabela de 4.096 bytes fica em `<cópia>/hfs-keystream.bin`, fora do Git.
- **Testes:** `scripts/content/test_content_additions.py` (5) e `scripts/client/test_apply_lab_hfs.py` (3), com bancos e pacotes sintéticos. As suítes `scripts/content` (65) e `scripts/client` passam.
- **Teste no cliente: passou (26/10/2026).** O operador deu `fiona_inner_fringe` e `fiona_inner_zero` à Fiona pelo painel. Os dois aparecem na bolsa com nome e ícone, vestem com o modelo certo e continuam depois de relogar. Consequências:
  - **o corte 1 está comprovado de ponta a ponta:** item novo no servidor e no cliente, texto novo em pacote refeito, ícone existente, criado pelo painel e persistido;
  - **o motor 1.69 aceita MDL v48**, junto com texturas em pasta nova e entrada nova no `player_costume.txt`. Modelos de clientes mais novos (por exemplo, o NA em `F:\Games\vindictus`) podem entrar sem conversão para v44, o que reduz o risco do corte 2 e dos monstros novos. Falta testar um modelo **compilado por nós** (Blender → `studiomdl`).

**Add-on do Blender e primeiro item editado (27/10/2026):**

- **Add-on versionado:** `tools/blender/vindictus` (extensão do Blender 5.1; instalação e uso em `tools/blender/README.md`).
  - Lê e grava MDL v44/v48, VVD v4, VTX v7 (pele por hardware, paletas de até 53 ossos) e VTF 7.2 (DXT1/3/5 e formatos sem compressão na leitura; BGRA8888 com mipmaps na gravação).
  - A importação cria o esqueleto, a malha com pesos, as normais e os materiais com textura, e oferece escolher malha, texturas e esqueleto, além de criar objetos novos, substituir ou somar à malha ativa.
  - A exportação troca a malha sobre o modelo-base do kit, porque não há `studiomdl` na máquina.
- **Kits:** `scripts/client/model_kit.py` gera um kit (modelo e materiais) direto dos pacotes do cliente, pelo índice HFS. É a rotina do futuro botão "exportar" do visualizador 3D do painel.
- **Item editado:** `fiona_inner_fringe` foi importado no Blender 5.1 (sem interface). O babado (malhas 1 e 2, 1.030 vértices) foi inflado 1,2 unidade ao longo das normais, e a máscara de tingimento recebeu listras diagonais com R e G trocados. O resultado foi exportado como `models/player/fiona/fiona_inner_lab.mdl`, com os materiais em `materials/models/player/fiona/fiona_inner_lab/`. Kit em `.cache/private-assets/inner_lab` e cena em `.cache/private-assets/fiona_inner_lab.blend`, os dois fora do Git.
- **Item novo:** `fiona_inner_lab` ("Lab Frilled Inner Armor"), com `CostumeType` 9101 e entrada 9101 no `player_costume.txt`. O modelo entra na parte 14 de `models/player/fiona`, junto com o `fiona_inner_zero`.
- **Conferido fora do cliente:**
  - a releitura do kit exportado mostra os mesmos triângulos, ossos válidos e até 3 pesos por vértice;
  - a contagem de vértices caiu porque o VVD do jogo guarda vértices usados só pelos níveis de detalhe menores;
  - a prévia renderizada no Blender mostra o modelo em postura de referência, com texturas;
  - o servidor reexportou os 13 arquivos (12.383 linhas de equipamento) e reiniciou saudável.
- **Testes:** `tools/blender/vindictus/tests/test_formats.py` (5 testes, com o Python do Blender). As suítes `scripts/client` (33) e `scripts/content` (65) passam. O teste de integração do `hfs.py` passou a usar o índice original de `lab-backup/` quando os itens de laboratório estão aplicados.
- **Teste no cliente: passou (27/10/2026).** O operador deu `fiona_inner_lab` à Fiona pelo painel. O inner aparece com o babado inflado e as listras, e continua depois de relogar. Ficou comprovado o ciclo completo: kit tirado dos pacotes, edição no Blender 5.1 com o add-on, exportação por troca de malha (VVD/VTX gerados por nós, VTF sem compressão), item novo e pacotes novos no cliente.
- **Ainda não feito:**
  - animações: ficam nos modelos de animação do personagem;
  - visualizador 3D e botão exportar no painel;
  - ossos novos;
  - mudar o número de materiais em relação ao modelo-base.

**Visualizador 3D e exportação de kit no painel (27/10/2026):**

- **Servidor:**
  - `src/infrastructure/client_files.rs` lê arquivos do cliente pelo índice HFS (a mesma lógica do `hfs.py`, em Rust, com leitura limitada).
  - `src/infrastructure/model_kit.rs` monta o kit (modelo, VVD, VTX, materiais e texturas, `kit.json`) como zip sem compressão, e resolve item → modelo pelo `player_costume.txt`. As seções são `inner_armor`, `upper_body`, `lower_body`, `hand`, `foot` e `head`, inclusive das categorias `AVATAR_*`.
  - Endpoints: `GET /api/models/kit?model=` e `GET /api/items/{item_class}/model`, os dois com permissão `Manage`.
  - O contêiner `game` monta `.cache/play-client` só para leitura em `/client` (`--admin-client-dir`, `--admin-client-keystream`). O `export_panel_catalog.py` passou a exportar o `CostumeType` (`ct`).
- **Painel:** a página **Modelos 3D** (`/modelos`) busca um item ou aceita um caminho de modelo.
  - O navegador lê o kit (`web/admin/src/lib/vindictusModel.ts`: zip, MDL v44/v48, VVD, VTX, VTF DXT1/3/5) e o desenha com three.js, com órbita, ossos e wireframe.
  - **Exportar kit (.zip)** salva o arquivo para o add-on do Blender.
- **Limites:** armas e cabelos ainda não são resolvidos pelo item (use o caminho); o visualizador mostra o nível de detalhe 0 sem animação.
- **Verificado:**
  - `tests/model_kit.rs` (com `--ignored`, contra a cópia de laboratório): o kit do `fiona_inner_fringe` gerado em Rust é idêntico byte a byte ao do `model_kit.py`, e o `CostumeType` 9101 resolve para `fiona_inner_lab.mdl`;
  - o leitor TypeScript do visualizador, rodado no Node 24 sobre o zip do `fiona_inner_lab`, leu as 3 malhas (índices válidos), as 3 texturas e os 75 ossos;
  - `cargo test --test admin_http` passa, com o `openapi.json` regenerado;
  - o painel compila (`npm run build`), e o Docker foi reconstruído e subiu saudável.
- **Teste no painel (pendente, do operador):** a criação de uma conta de teste com papel `gm` foi bloqueada pela permissão automática da sessão, então a tela logada não foi aberta nesta sessão.

### 3.2 Corte 2, modelo novo por classe (I3–I5)

Para cada uma das 10 classes do cliente (corpo e esqueleto próprios em `models/player/<classe>/`):

1. Modelar no Blender sobre o esqueleto da classe, importado do `.mdl` do corpo (ex.: `models/player/fiona/fiona.mdl`) e do inner armor existente, para herdar ossos, pesos e proporções.
2. Exportar `.mdl`/`.vvd`/`.vtx`/`.phy` v44 + a variante `_broken`, se o jogo a exigir para inner armor (**hipótese**: o `_broken` aparece em armaduras comuns), e as texturas `.vtf`/`.vmt` com os nomes que o `.mdl` referencia.
3. Entrada nova em `player_costume.txt` → `inner_armor` → `"<CostumeType novo>" { category <corpo>; label …; normal "models/player/<classe>/<novo>.mdl" }`, com um número livre (o editor reserva uma faixa própria, por exemplo ≥ 100.000, acima do maior atual, 90.375). O parser guarda o número (`atoi`) num vetor dinâmico por seção, procurado por igualdade; não há teto fixo no código lido, e um número repetido é tratado à parte (seção 5).
4. Linhas de `ItemClassInfo`/`EquipItemInfo` com esse `CostumeType`, no servidor e no cliente.
5. Distribuição: modelo, texturas e `player_costume.txt` ficam em `models/`, `materials/` e `scripts/`, prefixos que o cliente **só** lê de pacote. Precisam de `.hfs` refeito ou novo **com o índice HFS regravado** (formato recuperado em [game-launcher.md](game-launcher.md) seção 2.2; viabilidade decidida pelo teste L0-d). O texto pode ir solto (item 3 do corte 1). **Atualização de 26/10/2026 (L0-d, L0-e e L0-f do launcher):** o cliente aceita índice regravado e pacotes refeitos pelo codec, inclusive o de scripts com o `player_costume.txt` alterado. Restrição: cada arquivo tem de estar no pacote da sua pasta, e o nome do pacote de uma pasta vem de `SHA-1(alternar_caixa(pasta@….N))`, regra recuperada no mesmo dia (`game-launcher.md`). Modelos novos podem ir em pastas existentes (refazendo o pacote delas) ou em pastas novas (pacote `.0` novo), esta última comprovada pelo L0-g (Fiona com um inner lido de `models/player/fiona_l0`).
6. Opcional: oferecer na criação (`InnerArmorTypeInfo`), que hoje só cobre Lann, Fiona e Evie.

Custo estimado: conteúdo baixo; asset alto (10 modelos com rig de corpo diferente); servidor baixo (o catálogo já é dirigido por conteúdo); binário do cliente nenhum, se o L0-d aceitar o índice regravado. Se recusar, o corte 2 depende de mudar o binário ou o `GameInfo.txt`.

### 3.3 Corte 2: editor de itens no painel (27/10/2026)

Implementa criar, clonar, editar, ocultar e remover itens pelo painel (`/itens`, "Editor de itens"), sobre o caminho do corte 1.

**Armazenamento: edições de conteúdo no PostgreSQL, não o `item_additions.json`.** O painel roda no contêiner `game`, que não tem Python e monta `.cache/private-content` só para leitura; o `item_additions.json` é um arquivo versionado no Git. As mudanças do painel viram edições de conteúdo (migração `0049`, tipos `item`, `item_override`, `item_hide` e `item_remove`), com autor, auditoria, estado pendente e versão publicada, no mesmo ciclo das lojas e taxas (B4.4/B4.5). O `item_additions.json` continua como a camada versionada de laboratório; um item do painel com a mesma classe o substitui, e `item_remove` o tira.

**Sobreposição publicada** (`content_edits::overlay`, gravada pelo `game` em `.cache/content-state/content-edits.json`):

| Parte | Origem | Efeito no servidor e no cliente |
|---|---|---|
| `items` | `item` (último por classe) | linhas do item-modelo com `ItemClass` trocado e as colunas do formulário, `set_id` (`SetItemInfo`), texto, entrada do `player_costume.txt` e pacote dos assets do kit |
| `item_overrides` | `item_override` de item base | só as colunas de `EDITABLE` e o texto; uma sobreposição vazia ("Restaurar original") a tira |
| `hidden_items` | `item_hide` | as linhas ficam (os itens dos personagens continuam funcionando); o item sai das lojas de NPC (`apply_shop_rows`, em Rust e em Python) e da busca de "dar item" do painel |
| `removed_items` | `item_remove` de item adicionado | o item sai do conteúdo; quando o `game` carrega a publicação, todos os itens da classe saem dos personagens (desequipados antes) e as entregas pendentes dele são descartadas |

**Camada de sobreposição dos itens base:** o `heroesContents.db3` continua intocado. Em `content_additions.install`, uma tabela com sobreposição vira uma cópia TEMP na mesma ordem, com as células trocadas por `UPDATE`, e a `TEMP VIEW` lê essa cópia seguida das linhas adicionadas. Contagem e posição das linhas não mudam, então as conferências de ordem entre exportações continuam valendo e os 18 exportadores não mudaram. Um item adicionado clonado de um base editado copia as células editadas. No cliente, `apply_client` regrava as células a partir do backup `heroes.db3.pre-lab-additions` (desfazer volta ao original) e apaga as adições removidas (lista em `heroes.db3.lab-items.json`). O texto renomeado de um item base substitui a linha antiga do pacote de texto.

**Colunas editáveis** (`src/domain/item_editor.rs` e `content_additions.EDITABLE`, a mesma lista): `ItemClassInfo` `Icon`, `Category`, `TradeCategory`, `TradeCategorySub`, `Rarity`, `Level`, `RequiredLevel`, `ClassRestriction` (caixas das 10 classes do cliente), `MaxStack`, `Price`, `SellPrice`, `ExpireIn`; `EquipItemInfo` `CostumeType`, `MaxDurability`, `ATK`, `ATK_Speed`, `Critical`, `Balance`, `MATK`, `DEF`, `Res_Critical`, `STR`, `DEX`, `INT`, `WILL`, `LUCK`, `HP`, `STAMINA`, `Weight`, `Dyeable`. Cada uma tem limite e tipo; o ícone precisa ser um já usado por algum item.

**Kit e CostumeType:** `POST /api/item-editor/kits` aceita o zip do add-on (até 64 MiB, 512 arquivos, 256 MiB descompactados; sem compressão ou deflate, CRC conferido; só `models/` e `materials/` com `.mdl/.vvd/.vtx/.phy/.vtf/.vmt`, sem `..`, e `kit.json` com o modelo). O servidor grava em `.cache/private-assets/kit_<16 hex do SHA-256>` (montado gravável no `game`; o mesmo zip cai na mesma pasta). Ao salvar, cada arquivo do kit que já existir na cópia do cliente é recusado, exceto os do kit anterior do mesmo item. O script do modelo segue o item-modelo (`model_kit::ModelScript`):
- roupas e armaduras: `player_costume.txt`, seção da categoria; `CostumeType` novo é o primeiro livre a partir de 9100; o corpo (`category`) vem do formulário ou da entrada do item-modelo;
- armas: `scripts/weapon_<EquipClass>.txt`, bloco `WeaponData` → `SubTypeInfoOverride`, com `viewmodel` e `playermodel`; o número segue o último do bloco (as espadas vão até 10006, então o cliente já aceita números altos);
- escudos e escudos grandes: `scripts/shields.txt`, bloco `Fiona`, com `normal` e `broken` (o mesmo modelo); o número segue o último do bloco (1026 hoje).

Os números contam as entradas do cliente, os itens exportados do mesmo script e as adições pendentes e publicadas. Uma edição com kit novo mantém o número anterior do item. No cliente, `apply_lab_content_additions.py` escreve a entrada no arquivo do pacote `scripts` que o script indica (`kv_blocks` acha o bloco, `with_script_entries` acrescenta a entrada).

**Publicação e versão carregada:** um admin publica em Conteúdo, como as lojas. O `game` reinicia, grava a sobreposição e só marca a versão como aplicada se o selo `.cache/private-content/item-content.json` cobrir a versão de itens mais nova. Caso contrário, registra um aviso, e o Editor de itens mostra "o servidor exportou até ...". No host, `scripts/content/refresh_item_content.py`, chamado pelo `start-lab.bat` antes de atualizar a cópia do cliente, faz o seguinte:
1. valida a sobreposição;
2. compara o digest dos itens com o selo;
3. reexporta os 13 arquivos com a lista fixa de exportadores e argumentos;
4. grava o selo e reinicia o `game`.

Se algo falhar, ele devolve o backup e reinicia de novo. Nenhum dado da sobreposição vira comando.

**Prévia 3D:** o formulário usa o visualizador existente com o kit escolhido (lido no navegador antes do envio), com o kit guardado (`GET /api/item-editor/kits/{assets}`) ou com o modelo do item pelo script da cópia (roupa, arma ou escudo). Uma pasta de assets sem `kit.json` (como `inner_zero`, montada antes do editor) usa o único `.mdl` dela; se o kit guardado falhar, a prévia usa o modelo do cliente. A página Modelos 3D e o "Exportar kit" resolvem armas e escudos pelo mesmo caminho. Verificado: os kits de `longsword_swordwind`, `largeshield_kite` e `shield_swordwind` gerados em Rust são idênticos aos do `model_kit.py`; o leitor do visualizador lê os 3 e os 8 kits do inner_zero; o Blender 5.1, sem interface, importou e reexportou os 4 (3 de arma e escudo e o da Evy) com a mesma contagem de triângulos e caminhos novos para o cliente.

**Kit a partir de arquivos soltos:** `py -3.13 scripts/client/model_kit.py --folder <pasta no layout do jogo> models/.../<modelo>.mdl <pasta do kit> --zip <kit.zip>` monta o kit de um modelo (arquivos do modelo e os materiais que o `.mdl` cita) e o zip que o painel aceita.

**Quem tem o item:** `item_usage` conta `game_items` (com índice novo em `item_class_utf16`) e as entregas não processadas de `game_queued_items`, e lista até 50 personagens. Decisão do operador (27/10/2026), que substitui a regra da seção 2: ocultar mantém os itens dos personagens funcionando; remover vale só para itens adicionados e, quando a publicação carrega (início do `game`, antes de qualquer conexão), `minimum_equip::purge_class` tira o item de todos: um item vestido é desequipado (`part` nulo) e a aparência é refeita (`refresh_gear`, como no Enhance que destrói o item), depois o item, o equipamento e os atributos são apagados; itens do armazenamento compartilhado e entregas pendentes também saem. A limpeza roda a cada início para as classes removidas (idempotente) e deixa um registro no log do `game`.

**Limites:** ícone novo (arquivo `.tga`) e distribuição a jogadores dependem do launcher. Itens base não ganham modelo novo nem mudam de conjunto. Consumíveis (`FoodInfo`, `item_data.txt`) ficam para o I6. O formulário edita só a primeira linha de cada tabela, e a mudança vale para todas as linhas do item (as variantes de `Feature`). A remoção não confere linhas de loja, receitas nem drops: retire antes a linha de loja de um item adicionado que esteja à venda.

**Verificado:** `scripts/content` (71) e `scripts/client` em Python; testes de unidade Rust (`domain::item_editor`, `item_kits`, `content_overlay`); `admin_http` com o OpenAPI regenerado; teste PostgreSQL `item_editor_edits_reach_the_overlay`; `tsc` e build do painel. **Pendente:** teste no cliente (roteiro `docs/client-tests/B4-painel.md`, entrada I2).

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

O operador autorizou abrir o `FileSystem_Stdio.dll` e o `client.dll` no REA. O `FileSystem_Stdio.dll` foi descompilado (resultado em [game-launcher.md](game-launcher.md) seção 2.2). No `client.dll` original (`Vindictus1.69EN/…/bin/client.dll`, SHA-256 `b284ba719047556b…`, somente leitura), a análise automática completa do Ghidra pelo REA passou do limite de 330 s por chamada e foi cancelada. Por isso a leitura foi pontual: strings, referências `imm32` e desmontagem com o capstone de `scripts/client/.deps`, sem executar nada. Resultados:

- **Parser do `player_costume.txt`** (`0x38882f65` em diante): carrega o KeyValues; cada seção vira um índice de 0 a 11 pela tabela de nomes (`base` … `common_motion`); dentro dela, a chave que é um dos 10 nomes de classe vira bloco de classe (`default`, `default_ex`, `town`, `event_file_name`, `event_file_name_xe`, `color1..3`); a chave que começa com dígito vira entrada (`atoi`) de 0x74 bytes com `normal`, `broken`, `normal_sub_type`/`broken_sub_type` (variantes `%s01.mdl`…`%s%02d.mdl`, até 10, conferidas no sistema de arquivos `GAME`), `label`, `category` (índice na tabela `None`, `Male`, `Female`, `Giant`, `Tall`; ausente = −1), `hide_normal`, `hide_broken`, `onepiece`, `hide_rear`, `skin` e `body`. O número fica em `+0x30` da entrada; a entrada é acrescentada a um vetor dinâmico da seção depois de uma busca linear por número igual. **Sem teto fixo de `CostumeType`** no código lido.
- **Classes:** 10, com limite explícito (seção 1.1). Delia não existe neste cliente.
- **Consumíveis:** os tipos do `item_data.txt` estão no `server.dll` do cliente, não no `client.dll` (seção 2).
- **Ainda não lido:** o que o cliente faz com um `ItemClass` ausente do próprio `heroes.db3` (o corte 1 o evita publicando os dois lados juntos) e a tabela que liga `EquipItemInfo.CostumeType` à entrada do script.
- O REA não lê o launcher do Windictus (proteção de metadados, ver [game-launcher.md](game-launcher.md)).

## 6. Riscos

- Itens novos sem a linha correspondente no cliente: tooltip vazio ou recusa (**hipótese**). Mitigação: publicação conjunta pelo launcher.
- Modelos, materiais e scripts só entram por pacote com o índice HFS regravado, e o código que lê o índice está protegido com VMProtect (L0-d decide). Texto e ícone novo têm caminho solto. **Resolvido em 26/10/2026:** o L0-d, o L0-e e o L0-f passaram (`docs/future/game-launcher.md`, seção 2.4); a regra do nome das pastas novas também foi recuperada, e o L0-g passou.
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
| I5 | importar GLB, gerar `.mdl` v44 e entrada no `player_costume.txt`; extensão Blender 5.1 | inner armor com modelo novo em uma classe, depois nas 10 (depende do L0-d do launcher) |
| I6 | consumíveis: efeito (`FoodInfo`/`StatusEffectGroupInfo`) e `item_data.txt` | poção nova usada na cidade e na missão |
