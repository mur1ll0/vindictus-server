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

### Estado
- M0 (medição) feito. Nada aplicado na cópia de laboratório ainda. Nenhum teste de cliente.

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

---

## 3. O que veio do NA

Nenhum arquivo copiado ainda. Cada cópia entra aqui com destino, origem e SHA-256 (lista completa em `.cache/verification/miri/`, fora do Git).
