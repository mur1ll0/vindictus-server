# Porte da classe Miri (classe 11) do cliente NA para o laboratório 1.69

Registro do porte da Miri, na estrutura de `docs/future/delia-class-port.md` (que continua sendo a referência dos mecanismos comuns). Procedimento: skill `.agents/skills/new-class-port`. Passos `M<n>` no `docs/history/session-log.md`.

Convenções iguais às da Delia: `client.dll` base `0x38000000`, `server.dll` local base `0x40000000`, `engine.dll` base `0x42000000`; "NA" é o cliente oficial (`F:\Games\vindictus`, somente leitura; extração em `F:\Games\vindictus\hfsExtracted`).

---

## 1. Visão geral e estado

### Decisões do operador (30/10/2026)
- Classe: Miri, índice 11 (entrada 11 da `.hcls`, corpo 2, feminino). Branch `feat/miri-class`.
- **Cópia de assets do NA autorizada**: itens, armas, animações, receitas de forja e os scripts do banco ("precisa trazer vários arquivos de itens, animações, forjas, e os scripts para o banco de dados").
- **Mecânicas: portar tudo** nesta branch (transformação Dragon Knight, Dragon Smash, energia do dragão e os eventos que a classe usa), como a Delia: patch nativo, teste unitário e entrada no roteiro de cliente por mecânica.
- **Arma**: entidade `weapon_greatsword` do 1.69 com os modelos e itens `DRAKER` do NA (mesmo caminho da Delia; o 1.69 não tem `weapon_draker` no código).
- **Modelos**: converter os `.mdl` v45 do NA para v44 por script reprodutível (seção 2.2).
- **Painel admin**: compatível com a classe nova.
- **Volume**: todos os itens do NA com a Miri (3,8 GB preparados, 1,27 GiB em pacotes), inclusive os inners de 2021–2026 que usam texturas da pasta da Fiona (escolha do operador entre "sem inners de outra pasta", "tudo" e "só o núcleo").

### Estado
- M0 (medição), M1 (assets e scripts compartilhados na cópia), M2 (banco e textos) e M3 (dados de criação e painel) feitos. Nenhum teste de cliente; MI1 (não regressão) e MI2 (painel) pendentes em `docs/client-tests/C4-miri.md`. Falta todo o lado nativo (M4–M8): sem ele o cliente 1.69 não carrega a classe 11.

---

## 2. Medição (passo M0, sem patch)

Script versionado: `scripts/reverse/na_class_measure.py --class miri --weapon draker {db,models,tokens}`. Logs em `.cache/verification/miri/m0-*.log`.

### 2.1 Banco
- NA (`.cache/na-db/heroes-plain.db3`): 78 tabelas e 10.373 linhas citam `miri`. As principais: `ItemClassInfo` 1.347, `EquipItemInfo` 855, `CustomizeItemInfo` 998, `CustomizeDefaultInfo` 632, `CustomizePriceInfo` 3.396, `SkillInfo` 19, `SkillRankInfo` 304, `SkillArgumentInfo` 400, `ManufactureRecipeInfo` 23, `RecipeMaterialInfo` 56, `CraftShopInfo` 8, `FeatureMatrix` 6, `TitleInfo` 22, `ItemPackInfo` 22, `RandomItemV2Recipe_Client` 706.
- `ItemClassInfo.ClassRestriction` com a Miri: `miri` (568) e `female(!miri)` (1). Categorias: `INNERARMOR` 140, `HAIR` 123, `WEAPON` 76, `RAREITEM` 76, `CASHITEM` 47, peças de armadura (6–7 de cada), avatares (8 de cada).
- Arma única: `DRAKER` (76 itens). Habilidades: `skill_miri_attack`, `smash0..4`, `speedmove`, `speedmove_smash`, `dragon_striker`, `dragon_protect`, `dragon_revenge`, `dragon_energy`, `dragon_mark`, `sp1..sp5`, maestria `skill_miri_draker_mastery` (todas `IsFirstWeapon`).
- 1.69 (`heroesContents.db3` do servidor): nenhuma linha com `miri`. Esquema 1.69 de `ItemClassInfo` tem 37 colunas contra 44 do NA (sem `IconFG`, `IconBGInnerCensorCN`, `SellPriceAP`, `AccountSharable`, `AccountShareOnBind`, `TradePeriodItem`, `ConnectCategory`), e `ClassRestriction` é máscara `int`.

### 2.2 Arquivos e modelos
- Arquivos no NA com `miri`/`draker` no nome: 5.993 (`.cache/verification/miri/m0-na-files.txt`); pastas: `models/player/miri` (2.368 arquivos, 440 MB), `materials/models/player/miri` (152, 291 MB), `models/weapon/draker` (184), `materials/models/weapon/draker` (139), `models/player/_anim/female/miri` (14), `effects/player/miri` (147), `sound/voice/npc/Relationship/Miri` (2.384, 129 MB, só do sistema de relacionamento).
- **Versão dos modelos**: o 1.69 tem 16.802 `.mdl`, todos IDST v44. A Miri do NA tem 501 v45 (quase todo o corpo, `pc_miri_battle_motion.mdl`, `pc_miri_town_motion.mdl`) e 157 v44 (armas `draker_*`, armaduras, NPCs). `.vvd` (v4), `.vtx` (v7), `.phy` e `.vtf` (7.2) têm as mesmas versões do 1.69.
- **v45 = v44 + nome longo**: no par `pc_delia_hair_crystal_rose_04.mdl` (NA v45, 22.240 bytes × 1.69 v44, 21.980), o cabeçalho tem o mesmo leiaute; o v45 insere 260 bytes (`MAX_PATH`, caminho do modelo terminado em zero) logo depois do cabeçalho de 408 bytes, e todos os deslocamentos absolutos do cabeçalho andam 260. O resto difere só por arredondamento de `float` da recompilação. O `datacache.dll` 1.69 testa só o `IDST` antes de entregar o cabeçalho ao motor (`0x1000bcd8`). Decisão: converter para v44 (remover o bloco, corrigir os deslocamentos do cabeçalho e os `baseptr` de sequências e animações).

### 2.3 Scripts da classe
Tokens dos scripts da Miri do NA ausentes nos DLLs 1.69 (`m0-tokens.log`; "1.69 só em scripts" = o 1.69 também não tem código, como na Delia):
- Chaves de quadro: `$immediately$` (já tratada pela Delia em `FRAME_KEYS`), `$endframe$`.
- Condições próprias: `As_Cond_Miri_DragonKnight`, `As_Cond_Miri_Can_Transform_DragonKnight`, `As_Cond_Miri_Can_DragonSmash`; outras novas: `As_Cond_Maintain_Custom_MotionSpeed`, `As_Cond_RelationshipSkill_Can_Follow`, `As_Cond_Town_Interaction_End/Stop`. A Miri também cita as 5 condições da Delia e `AS_Cond_Force_Player_Draw`, `AS_Cond_Ignore_SuperArmor_Flinch`, `As_Cond_Can_Armor_Purge` (sem código no 1.69).
- Comandos `AE_CUSTOM_MOTION` próprios: `EQUIP_DRAGON_KNIGHT_LOOK`, `PURGE_DRAGON_KNIGHT_LOOK`, `REDUCE_DRAGON_KNIGHT_SP_GAUGE`, `USE_DRAGON_ENERGY`, `HOLD_SKILL_REMOVE_EFFECT_MODELS`, `SET_VISIBLE_HUD_NAME`, `SP_Paladin_*`/`SP_DarkKnight_*` (`BladeAttack`, `ForceHole`, `ThrowAttack`).
- Eventos `AE_*` novos (26, quase todos visuais): `AE_ATTACH/DETACH_ANIMATING_PROP`, `AE_CL_TEXTURE_TRANSITION`, `AE_SUPERSEDE/RESTORE_SKIN_SHININESS`, `AE_SCALE_BONE_ONCE`, `AE_WEAPON_ENABLE/DISABLE_VERIFY_ATTACK`, `AE_UPDATE_INNERARMOR_MODE`, `AE_SET_HAND_IK_AUX_MODEL`, entre outros.
- `scripts/weapon_draker.txt`: mesma família do `weapon_greatsword.txt` (`Melee`, `anim_prefix draker`, subtipos 29/32).

### 2.4 O que é código novo (escopo "portar tudo")
1. Transformação Dragon Knight: 3 condições, troca de visual (`EQUIP/PURGE_DRAGON_KNIGHT_LOOK`, `csf/metamorphosis/miri_dragon_knight.csf`), gasto de SP (`REDUCE_DRAGON_KNIGHT_SP_GAUGE`).
2. Dragon Smash (`As_Cond_Miri_Can_DragonSmash`) e energia do dragão (`USE_DRAGON_ENERGY`, `skill_miri_dragon_energy`).
3. Ataques SP de Paladin/DarkKnight (`SP_*`), remoção de modelos de efeito da habilidade segurada.
4. Eventos visuais `AE_*` usados pela Miri: medir quais mudam o que o jogador vê antes de portar.
5. Condições compartilhadas com a Delia: os grupos `GUARD`, `CLIENT_GUARD`, `CLASS_CONDITIONS` e `DELIA_EFFECTS` despacham toda classe extra; restringir à Delia antes de pôr a Miri (skill, armadilha 9).

### 2.5 Entrega no cliente (medido no M1)
- `models/`, `materials/` e `scripts/` só são lidos de pacotes `.hfs` listados no índice (`docs/future/game-launcher.md` 2.3); pacotes novos e pastas novas funcionam com o índice regravado (L0-f, L0-g). O nome do pacote vem da pasta e da parte (`hfs.pack_name`).
- O 1.69 usa várias partes por pasta (324 de 1.924 pastas; `models/player/fiona` tem 14), em geral de ~5 MB, e há pacotes de até 207 MB. A Miri entra em pacotes novos, partes seguintes às existentes, de até ~48 MiB, para não passar de 14 partes por pasta.
- Scripts compartilhados que citam a Miri no NA (`.cache/verification/miri/m1-shared-na.txt`, 61): 21 existem no 1.69 e recebem só os trechos da Miri (`player_costume.txt` 651 linhas, `game_sounds_heroes_weapons.txt` 104, `game_sounds_heroes_player.txt` 94, `projectile.txt` 83, `action_table_player.txt` 59, `item_data.txt` 17, `face_layer.txt` 14, `skill_actionstate_match_table.txt` 13, `translated_spskill.txt` 6, entre outros); 40 são novos no NA (relacionamento, *quickchat*, `new_character_info.txt`, armas de outras classes) e só entram se a Miri precisar.
- A única autoridade sobre o índice e os pacotes da cópia é `scripts/client/apply_lab_content_additions.py` (reconstrói a partir dos *backups* a cada execução); a Miri entra por ele, não por um segundo gravador.

### 2.6 Conversor `.mdl` v45 → v44 (M1a)
- `scripts/client/mdl_v45_to_v44.py`: remove o bloco de 260 bytes, desloca os índices do cabeçalho até `bonetablebynameindex` (`+364`), as tabelas de *strings* absolutas (`cdtexture`, nomes de nós) e o `baseptr` das sequências e animações (passo achado pelo próprio `baseptr`, limitado pela seção seguinte). De `+368` em diante o cabeçalho do Vindictus tem campos próprios (marca `SCAL` em `+388`), iguais nas duas versões, que ficam como estão.
- Validação (`.cache/verification/miri/m1-validate.log`): 692 pares NA v45 × 1.69 v44 convertidos sem erro; nos 165 de mesma estrutura o cabeçalho convertido é idêntico ao do 1.69; 5 corpos com uma palavra diferente de exatamente 260 são ruído de `float` em ossos. Os 504 `.mdl` v45 da Miri/Draker convertem sem erro.

### 2.7 Assets e scripts na cópia (M1b, M1c)
- `scripts/client/stage_na_class.py --class miri --weapon draker` prepara em `.cache/private-assets/classes/miri` (nunca versionado) os arquivos com `miri`/`draker` numa pasta ou no nome, em `models/`, `materials/`, `effects/`, `expressions/`, `csf/`, `scaleform/`, `sound/` e `scripts/`, menos relacionamento, menu inicial do NA, *hero path* e variantes CN/JP; fecha os materiais e texturas que os modelos usam (na ordem de busca do motor: a primeira pasta `cdmaterials` que tem o material), os `includemodel` e os `include` de VMT; converte os v45; nunca prepara um caminho que o 1.69 já tem. `class-assets.json` guarda origem e SHA-256 de cada arquivo.
- Resultado: 4.331 arquivos (3,8 GB; 504 modelos convertidos), 21 scripts para mesclar, 18 referências que nem o NA tem (`*_lod.vmt`, `glint.vmt`, pupilas, 4 `.vtf`).
- `scripts/client/apply_lab_content_additions.py` (único gravador do índice e dos pacotes) monta os pacotes da classe por pasta, nas partes seguintes às do 1.69, até 192 MiB de dados por parte (75 pacotes, 1,27 GiB; `sound/` cru, o resto `.comp`), com cache por SHA-256 em `lab-backup/class-packs.json` (reaplicar sem mudança não recomprime); grava em fluxo (`hfs.write_pack`) e sincroniza o índice em lote (`hfs.sync_listed`).
- Mesclagem (`scripts/client/kv_merge.py`): o pacote do 1.69 que contém cada script é reconstruído do *backup* com só os nós do NA que citam `miri`/`draker` e faltam no 1.69; uma chave presente nos dois com outro valor é conflito e fica como no 1.69. Aplicado: 21 arquivos, 811 blocos, 7 linhas; 1 conflito (`common_state_etc.txt`, `RepeatableEmotion/.../sequence`). Ficaram de fora `chat_commands.txt` (bloco de emotes de voz de todas as classes do NA), o `precache` do relacionamento em `game_sounds_manifest.txt` e os scripts de NPC, pet e relacionamento.
- Renumeração (`scripts/content/class_costumes.py`): os inners da Miri 9100, 9101 e 9102 colidiam com itens do editor de itens (dados do operador, que ficam como estão); foram para 9044, 9134 e 9148, os primeiros números livres a partir de 9000. O servidor aplica a mesma renumeração ao `CostumeType` dos itens da Miri (M2).
- Verificação na cópia (`.cache/verification/miri/m1b-verify.log`): os 4.331 arquivos estão no índice; amostra de 4 pacotes (198 arquivos) idêntica à preparação.

### 2.8 Banco e textos (M2)
- `scripts/content/class_imports.py --class miri --weapon draker --index 11 --stats-from 10` grava as linhas da Miri do `heroes.db3` do NA, com as colunas do NA, em `.cache/private-content/class-imports/miri.db3` (nunca versionado). Seleção: itens cujo `ClassRestriction` nomeia a Miri ou cujo nome tem `miri`/`draker`, ausentes no 1.69 e de categoria que o 1.69 conhece (1.170; fora: 1 `AVATAR_WEAPON`), com `EquipItemInfo`, `Customize*`, `DyeableParts`, `PatternInfo`; 19 habilidades com ranks, argumentos e restrições; receitas cujo produto (`RecipeMaterialInfo` `Type 0`) é um item importado e cujos materiais existem (55 linhas, 11 receitas; 6 receitas fora por material ausente), com a loja do NA trocada pela do 1.69 que lista as mesmas receitas (`FERGHUS_Craft` → `_Craft`, depois da última linha); `StatusEffectGroupInfo` (13), `SynthesisSkillBonus` (5) e `FeatureMatrix` (3: `Miri`, `CharacterStatRankLimit_Miri`, `DressRankLimit_Miri`; relacionamento, filme inicial e evento fora).
- A mesma conversão serve aos dois lados: colunas do alvo (valor do NA ou o valor mais comum da coluna), `ClassRestriction` → bit 11 (2048), `CostumeType` renumerado como no M1 (9044/9134/9148), `FeatureMatrix` `Miri` com a versão `EN-EU` da Delia (3.09.00; as do NA são 4.x) e as outras com o valor `EN-US` do NA. Itens existentes ganham o bit 11 quando a máscara já cobre todas as classes femininas do 1.69 (1446) e a expressão do NA não exclui a Miri (`female(!miri)`).
- Servidor: `content_additions.connect` instala tabelas TEMP (`_imported_<tabela>`, `_class_base_ItemClassInfo`) e visões, sem tocar o banco de referência; `CharacterClassInfo` e `CharacterBaseLoadInfo` da classe 11 copiam as da Delia (escolha de laboratório: o banco do NA é do cliente e não tem tabelas de atributos). Conferido: +1.170 itens na visão, 9.088 linhas 2047 → 4095, ranks e loja de forja presentes.
- Cliente (`apply_lab_content_additions.py`): as tabelas tocadas voltam às linhas do *backup* (`heroes.db3.pre-lab-additions`, que só difere do atual pelos itens do editor) e recebem as linhas importadas antes das do editor de itens. Aplicado: 1.170 itens, 304 ranks, 11.870 linhas com o bit 11.
- Correções do fechamento do M3 (a reexportação recusou as linhas): booleanos seguem a forma da coluna no 1.69 (`CustomizeItemInfo.IsCash` é `'True'`/`'False'` no 1.69 e 0/1 no NA); linhas cuja `Feature` cita algo fora da `FeatureMatrix` do 1.69 mais as da Miri (outras classes do NA como `Calia`, eventos do NA) ficam fora, e com elas as linhas dependentes dos itens que perderam toda `ItemClassInfo` (1.166 itens, 300 ranks; os 4 ranks fora dependem de `S4EP*`). A região inglesa do NA é a americana e o laboratório é EN-EU: nas linhas importadas, `enUS` vira `enEU` nas `Feature` e `en-US` vira `en-EU` em `CashShopType` (como as linhas da Delia no 1.69); sem isso nenhum item da Miri era oferecido na região e a aparência de criação da classe 11 era recusada (`private_class_content`).
- Textos: `stage_na_class.py` copia `heroes_text_english.txt` do NA; `class_imports.class_text` escolhe as chaves que citam `miri`/`draker` ou terminam no id de um item ou habilidade importado; os pacotes de texto do 1.69 recebem só as que faltam (5.702). O painel usa a mesma função (`export_item_names.py`) e procura ícones também nos assets preparados (`export_panel_catalog.py`).

### 2.9 Criação e painel (M3)
- `scripts/content/class_additions.json`: Miri (11) com os consumíveis e o ouro da Delia, `draker_newbie`, `skill_miri_draker_mastery` e `skill_miri_speedmove` em rank 1 e o inner `miri_inner_free` (escolha de laboratório; o NA não traz `DefaultItemInfo`/`DefaultSkillInfo`, que são tabelas do servidor).
- Exportações privadas: `class-initial.json` (12 classes, 400 pedidos), `class-11-initializer.json` e `class-11-initializer-reference-pairs.json` (`measure-fiona-initializer-features.ps1`: 30 comparações, x86 = x64); `docker/lab/minimum-creation-assets.json` com a classe 11.
- `scripts/dev/lab-set-class.ps1`: classe 11 com as habilidades iniciais acima, aparência do `create_miri.csf` do NA (inner 1002, cabelo 8008, sobrancelha 12001), formas do corpo da Fiona (o `default_miri.bsh` usa os mesmos controles femininos) e figura 107 (`bodyshape_fig_list.txt` mesclado no M1).
- Maestria: `DRAKER` → `skill_miri_draker_mastery` em `src/domain/equipped_skills.rs` (teste em `tests/equipped_skills.rs`).
- Painel: nomes Delia e Miri na lista de personagens (`web/admin/src/api/characters.ts`), bits 1024 e 2048 nas caixas de classe do editor de itens (`items.ts`), Miri na dica de item (`admin_tooltip.rs`, 12 classes); nomes e ícones dos itens da Miri pelas exportações do M2. O carimbo das exportações (`item-content.json`) ganhou `classes_sha256` (`class_imports.digest`), para o `start-lab.bat` reexportar quando as classes importadas mudam.

---

## 3. O que veio do NA

Autorização do operador (30/10/2026): "precisa trazer vários arquivos de itens, animações, forjas" e, na escolha de volume, "Tudo (3,8 GB)". Origem: `F:\Games\vindictus\hfsExtracted`. A lista completa, com caminho de destino, caminho de origem e SHA-256 da origem e do arquivo convertido, está em `.cache/private-assets/classes/miri/class-assets.json` (fora do Git); os 21 scripts do NA usados na mesclagem e o texto em inglês ficam em `.cache/private-assets/classes/miri/merge/`; as linhas do banco, em `.cache/private-content/class-imports/miri.db3` (origem gravada na tabela `_class`). Na cópia `.cache/play-client`: 75 pacotes novos (lista em `lab-backup/class-packs.json`), índice e pacotes de scripts reconstruídos a partir de `lab-backup/` (desfazer: apagar `class-assets.json` e reaplicar).
