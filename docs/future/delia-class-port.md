# Porte da classe Delia (classe 10) do cliente NA para o laboratório 1.69

Registro único de tudo o que a Delia exigiu, do primeiro teste sem patch (27/10/2026) à criação pela grade completa até a cidade (C3-23, 30/10/2026). Serve de receita para a próxima classe (Miri, 11).

Fontes: `docs/history/session-log.md` (de "Caminho C, etapa C2" ao "Passo 50", "Fechamento do C2 da Delia" e "Passo C3-1" a "Passo C3-23"), `docs/future/new-classes.md` (8.1–8.15, 9.1), `docs/new-class-guide.md` (rascunho anterior, absorvido aqui), `docs/client-tests/C2-delia.md`, `docs/client-tests/C3-criacao.md`, `scripts/client/patch_class_routing.py`, `patch_class_table.py`, `patch_engine_class_flags.py`.

Convenções: endereços são do 1.69 EN de laboratório (`client.dll` base `0x38000000`, `server.dll` local base `0x40000000`, `engine.dll` base `0x42000000`); "NA" é o cliente oficial (`F:\Games\vindictus`, somente leitura; x64 em `.cache/bin_x64`, 32 bits em `appdata\en-US\bin`). Os passos citados ("passo 31", "C3-9") são entradas do `docs/history/session-log.md`; "D…"/"G…" são entradas dos roteiros de cliente.

---

## 1. Visão geral e estado

### Por que a Delia precisou de código nativo
- O conteúdo do 1.69 já tinha a Delia quase inteira: `heroes.db3` com classe 10, bit 1024 em `ClassRestriction`, 196 itens, 304 ranks e 512 argumentos de habilidade, `BASTARDSWORD`; modelos, animações, efeitos, scripts de estado de ação, `.csf`, vídeos (`docs/future/new-classes.md` 8.1).
- O que faltava era código: os DLLs 1.69 tratam 10 classes em tabelas fixas, `switch` com limite 9, vetores de 10/11 posições, `m_iClass` com 4 bits e o pet no índice 10 (8.2, 8.13). A classe 10 caía no caso do pet ou lia a tabela seguinte.
- A decisão (8.3, 8.9): estender tabelas e rotear a classe 10 por uma tabela (`EXTRA_CLASSES`), sem clonar a entidade da Fiona e sem copiar código do NA (a base do jogador do NA tem 538 métodos e `0x7c20` bytes, contra 452 e `0x4bf8` no 1.69).
- Layout escolhido, igual ao NA: classes 0–27 na ordem da tabela do NA, pet no 28, `m_iClass` com 5 bits, teto 32 (máscara `int`).

### O que funciona (testado no cliente)
- Seleção: modelo, rosto, cabelo e roupa certos, sem a faixa branca (D17, passo 29).
- Cidade e fases: corpo e animações (D8v, passo 21); uma espada nas costas (D18, passo 31); estamina normal (D19, passo 32).
- Habilidades: janela com as habilidades certas (D12–D13); todas as ativas até o nível 45; SP1/SP2/SP4/SP5 pela barra (D22, passo 42).
- Guarda: bloqueia o dano, contra-ataque sai, sem som de impacto (D22–D23, passos 34, 37, 44).
- Wild Star (4º estágio) e Star Divider com os efeitos de estado (D22, passos 39–40).
- Rose Crush (SP5): quebra e tira a armadura na fase, volta com o conserto; no barco, só o visual some e volta (D23, passos 44–50).
- Itens da Delia sem fundo vermelho; dica sem "Cannot be equipped" (D22, passo 43).
- Fechamento do C2 (29/10/2026, operador: "funcionou perfeitamente"): 154 caixas encerradas no `C2-delia.md`; só o D6 ficou aberto e passou ao C3.
- Criação pela grade (C3): grade com 11 classes na cena `lobby_beautyshop_change`, atores em pose de apresentação, customização da Delia com o modelo certo, botão de voltar, campo de nome e "Check", roupas de amostra (G6), WASD travado e troca imediata de pose/ação (C3-1 a C3-19).
- Criação até a cidade (30/10/2026, operador: "Funciona completamente agora"): "Create" → estalagem (`create_character`) → fala da Tieve → "Go to town" → cidade com a Delia nova, com WASD e poses da customização ainda certos (C3-20 a C3-23).

### O que ainda não foi confirmado no cliente
- G5 (voltar da grade sem criar) e G3 (classe antiga pela grade até a cidade): não relatados depois do C3-23.
- Volta da cidade à seleção com a personagem criada pela grade: não relatada separadamente (a seleção da `DeliaLab` pela lista já funcionava no C2).

### O que ficou aproximado
Lista completa na seção 7. Os principais: `As_Cond_Can_Armor_Purge` sempre ligada, `SP3_Activated` não portada, ataque forte e habilidade segurada (E4/E5 itens 5–6) não implementados, `weapon_greatsword` no lugar da `weapon_bastardsword`, atores da grade sem a pose `SELECTED_S3` animada do NA, nome em cinza na customização.

---

## 2. Arquivos de conteúdo

### 2.1 Origem: nenhum asset do NA entrou na cópia de laboratório
- Todos os arquivos da Delia que os patches pedem já estavam no 1.69 (`.cache/play-client/hfs_extracted`). Comparação NA × laboratório: `.cache/verification/d13-delia-na-vs-lab.txt` (485 arquivos no laboratório, todos no NA, 107 diferentes: malhas principais iguais, animações e cabeçalhos `.mdl` atualizados no NA; passo 26). Os do 1.69 foram mantidos.
- O operador autorizou copiar o filme de criação do NA (8.9), mas o C3-1 decidiu usar o filme da grade do 1.69 (par do binário, completo para 11 classes). O filme do NA fica para a classe 11+.
- As únicas mudanças de conteúdo foram na criação (C3-12 a C3-19), todas com arquivos do próprio 1.69 (seção 2.4).

### 2.2 Arquivos da Delia usados (já presentes no 1.69)

| Arquivo | Quem lê | Patch que o liga |
|---|---|---|
| `models/player/delia/` (146 arquivos): `delia_base.mdl`, `pc_delia_face01..03.mdl`, `delia.mdl` | pré-carga, roupa, `cl_playermodel` | `PRECACHE`, `COSTUME` |
| `models/player/_anim/female/delia/pc_delia_town_motion.mdl`, `pc_delia_battle_motion.mdl`, `pc_delia_battle_motion_ex.mdl` (13 arquivos na pasta) | modelos de animação do servidor local | `TABLES['server.dll']`, `PRECACHE` |
| `models/player/_anim/female/delia/pc_delia_motion.txt`, `pc_delia_motion_ex.txt` (15 `$include$`), `pc_delia_motion_seqGroup.txt` | arquivo de eventos da animação; `ENABLE_ENDURE_MODE` da guarda; `spskill_activate_states` com `$immediately$`; `PURGE_ALL_ARMOR` no quadro 125 do `delia_SP5` | `EVENT_FILE`, `GUARD`, `FRAME_KEYS`, `DELIA_EFFECTS` |
| `models/weapon/bastardsword` | modelo do item | (a entidade é `weapon_greatsword`) |
| `effects/player/delia` (61) | efeitos | — |
| `scripts/actionstate/player_action_state_delia[_ex].txt` | estado de ação (usa `weapon_defaultsword` e as 5 condições novas) | `SWITCHES`, `CONDITIONS` |
| `scripts/expression/expression_table_delia[_ex|_common].txt` | expressões | `SWITCHES` |
| `scripts/skill/delia_skill_keyword_table.txt` | palavras-chave | `SWITCHES` |
| `scripts/player_costume.txt`, seções `delia` (base, face, motion) | gerenciador de roupa | `COSTUME` |
| `scripts/translated_spskill.txt`, seção `delia` | habilidades SP | `CLASS_NAME` |
| `scripts/status_effect.txt` e efeitos do `heroes.db3` (`skill_delia_strong_attack[_week|_hit|_delay]`, `skill_delia_stamina_adder[_two]`, `skill_delia_will`) | efeitos de estado | `DELIA_EFFECTS` |
| `scripts/skillactionstatematch` (13 linhas `skill_delia_*`) | `SkillActionStateMatchTable` | `MATCH` |
| `csf/create_delia.csf` (inner 1002, cabelo 6036, sobrancelha 8000), `csf/prologue_delia.csf`, `csf/costume_default_delia.csf` | tabelas do servidor; roupa-base | `TABLES['server.dll']`, `lab-set-class.ps1` |
| `csf/bodyshape_delia_default.fig`, `csf/bodyshape_*_delia.fig`, `csf/default_delia.bsh` (`isDefaultFigure`), `csf/bodyshape_fig_list.txt` (chave 106) | figura do corpo, forma do corpo | `ENTITY_SLOTS`, `lab-set-class.ps1` |
| `csf/createcharacter_delia.csf` e os outros 10 `createcharacter_<classe>.csf` | atores `npc_<classe>` da grade | `CREATION_GRID` |
| `csf/queen_after_trial_delia1..4.csf` (Crystal Rose 700, Training Cube 704, Infinity Crown 705, Lugh Lámhfhada 678; iguais byte a byte aos `global_queen_after_trial_delia1..4.csf` do NA) | roupas de amostra da customização | `SWITCHES` (`0x384A3308`) |
| `scaleform/video/.../NewCharacter_Delia.usm` e 20 vídeos de habilidade | grade e janela | — (o filme da grade toca o `.usm` sozinho) |
| `scaleform/NewCharacterDialog_V2.gfx` do 1.69 (11 classes, Delia incluída) | grade `NewCharacterDialogS3` | `CREATION_GRID` |
| `maps/lobby_beautyshop_change.bsp` (11 `npc_character_creation_actor` `npc_<classe>`, `point_teleport` `npc_character_create_select`/`_nonselect`) e `maps/lobby_beautyshop_body.bsp` | cena da grade e da customização | `CREATION_GRID` |
| `EyeTargetTestPanel.gfx` (poses `cc_play_repeatable_emotion 1..5`, ações `6..10`) | painel da customização | `CREATION_GRID` (C3-18) |

O mapa `s3_game_create_character.bsp` (só 9 classes, igual no NA) não serve e não é usado (D6, 8.7).

### 2.3 `heroes.db3` do cliente e FeatureMatrix
- `ClassRestriction` 1024 nos itens da Delia, 2047 nos comuns (solto e empacotado, conferido no passo 9).
- FeatureMatrix do cliente, região `EN-EU-X-GM` (`CurrentVer` 3.09.00): `Delia` 3.09.00 (ligada), `CharacterCreation_UI_V3` 3.09.00 (ligada; o filme novo da customização testa essa *feature*, C3-14), `S3_CharacterCreation` NULL em todas as regiões (o servidor de laboratório a envia, seção 3.4).
- O `heroes.db3` não foi alterado.

### 2.4 Mudanças de conteúdo da criação (C3-12 a C3-19), só na cópia `.cache/play-client`
A cópia carrega os filmes soltos de `scaleform/`; o pacote `9FB527B3…`, que tem a versão mais nova, está renomeado `RENOMEADo_9FB527B3….hfs` e não é lido (C3-12).

| Arquivo solto | Antes | Depois | Backup / desfazer |
|---|---|---|---|
| `scaleform/CreateCharacterDialog.gfx` | `1a51f68934503d8f14b2cf8e40b38032c0fda5499b9b176c29f660a0bc5efed4` (77 536 bytes, 2016, sem `Button_Back` no palco) | versão do pacote do próprio 1.69, `d9f2dd66a109ed1821d682781322a4360e9f0cf3b4dfad8eb500403134ab5665` (origem `hfs_extracted/PATH_NOT_FOUND/RENOMEADo_9FB527B373DCBD159552ADD1710487A724237AF9/CreateCharacterDialog.gfx`) | `scaleform/CreateCharacterDialog.gfx.1a51f689.loose.bak` (C3-12) |
| o mesmo, editado no ramo V3 do `onShow` | `d9f2dd66…` | `11973c0cb2c06117b9af05f236e1ceb2b3c22088a6b91774e8157424473af007`: `0xe425` `false`→`true` (campo de nome, texto e "Check" visíveis), `0xe45b` `true`→`false` (`t_Notic` oculto), `0xe464` `push r0`→`push true` (`Button_Back` visível) | original no `hfs_extracted` (C3-14) |
| o mesmo, cor do nome | `11973c0c…` | `4c8b8275d548445f1ccc6effae7db387c2cf10b9b6a0d63bc1d514cab07da878`: `Text_Name` (`DefineEditText` 87) `#cccccc`→`#ffffff` em `0xf381` e no HTML inicial | (C3-15) |
| o mesmo, `onKillFocus` | `4c8b8275…` | `ac5543b072761a3f648f5ba98d6c9316d331173dfb2d52343835425a662c59ec`: constante `6710886` (`0xd67f`) → `16777215` (filme em uso no fim) | (C3-17) |
| `scaleform/CreateCharacterDialog_I30.dds` | `0759d216…` (ícone de 4 224 bytes) | `04084564…` do pacote renomeado (fundo `MC_BG_Scale`, 524 416 bytes) | `<arquivo>.<hash>.loose.bak` (C3-13) |
| `scaleform/CreateCharacterDialog_I89.dds` | `f250de86…` | `9a73326f…` (painéis de cor) | idem (C3-13) |
| `scaleform/CreateCharacterDialog_I5.dds` | `98c11a45…` | `04642050…` (atlas de `MC_SheetBodyShape`, `MC_SheetHair` e dicas de peso; voltou ao solto no C3-13 por diagnóstico errado e ao do filme no C3-19) | idem |

As outras 31 imagens do filme novo vêm do pacote legível de `scaleform` parte 7 (`E67F5DB8…`). Os `createcharacter_*.csf`/`.usm` e os dois `lobby_beautyshop_*.bsp` já estavam no 1.69 e não mudaram.

---

## 3. Banco de dados (PostgreSQL) e dados de criação do servidor

### 3.1 Nenhuma migração nova
Nenhuma migração de `migrations/` foi criada ou alterada para a Delia. A coluna `game_character_info.class` (`SMALLINT`) e as máscaras `ClassRestriction` (`wrapping_shl(class & 31)`) já eram genéricas (8.6, 8.13).

### 3.2 Dados de criação da classe 10 (sem tocar o banco)
O servidor 1.69 (`heroesContents.db3`) tem `CharacterClassInfo` e `CharacterBaseLoadInfo` da classe 10, mas não `DefaultItemInfo`, `DefaultSkillInfo` nem o caso `INNERARMOR` (8.13). Passo 2 (commit `d90bfe45`):
- `scripts/content/class_additions.json`: linhas de laboratório no padrão da Hagie (não são dados recuperados do NA):
  - `default_items`: `life_potion_lvl0` 20, `phoenix_feather_lvl0` 4, `repair_kit_lvl0` 10, `gold` 10000, `javelin_lvl0` 14, `bastardsword_newbie` 1;
  - `default_skills`: `skill_delia_bastard_mastery` rank 1 (`LevelConstraint` 8);
  - `inner`: `delia_inner_free`, cores `13158600` ×3.
- `scripts/content/content_additions.py` (`load_classes`, `install_classes`, linhas 219/240): anexa as linhas depois das originais por `TEMP VIEW`.
- `export_class_initial.py` e `export_fiona_initializer.py` aceitam as classes de laboratório. Exportação privada: `class-initial.json` (11 classes; Delia com 34 pedidos e 200 níveis), `class-10-initializer.json` (+ `class-10-initializer-reference-pairs.json`).
- `docker/lab/minimum-creation-assets.json`: entrada `class_initializers` da classe 10 (`/content/class-10-initializer.json`).
- Teste: `python -m unittest test_content_additions test_export_fiona_initializer` (21, 0 falhas).

### 3.3 Personagem de laboratório: `scripts/dev/lab-set-class.ps1`
Troca a classe de uma personagem desconectada pelo nome, numa transação dentro do contêiner `postgres` (sem credencial na linha de comando). Imprime a classe anterior para desfazer.
- `game_character_info.class` → classe nova (`ValidateRange(0,10)`).
- `game_skills`: acrescenta as habilidades iniciais da classe nova que faltam (rank 1, `row_id` em *ticks* UTC como `MakeEntityID`), depois das linhas do inicializador; nunca renomeia (a seleção recusava, D10; passo 24). Delia: `skill_delia_bastard_mastery`, `skill_delia_guard` (escolha de laboratório, passo 23).
- `game_costumes` base: inner (tipo 0 chave 1), cabelo (0,3), sobrancelha (1,0) que ainda estejam no padrão da classe antiga passam ao `create_<classe>.csf` da nova (Delia 1002/6036/8000; passo 23).
- `game_costumes` tipo 3 (forma do corpo): troca o conjunto de chaves entre classes conhecidas (Fiona, Hagie, Delia: 18, 19, 20, 24, 25, 40, 71–75) e grava a figura padrão (Delia 106 = 1000; passo 26).
- Tabelas no script: `$defaults`, `$looks`, `$shapes`, `$figures`.
- `DeliaLab`: cid 639287373299843079, conta `lab169_a`. Itens dados com `lab-give-item.ps1` (`bastardsword_chieftain_restoration`, `delia_china_inner`).
- Perfil de habilidades do servidor: `skill-profile-v5.json` com as 19 da Delia (passo 24).
- Nível e AP para testar SP: painel admin, progressão "até o nível N" (`admin_progression.rs`, aceita 0–27; passo 35).

### 3.4 FeatureMatrix enviada pelo servidor
- `src/infrastructure/lab_frontend.rs` (≈ linha 4329): o laboratório envia `S3_CharacterCreation = 3.09.00` junto de `NGM_disable`, `CashShop` e das URLs da loja. Histórico: ligado no commit `8305d80` (D6), desfeito no `5aa5cf2` (a grade antiga travava), religado no C3-1 (`d7b759d6`).
- A *feature* `Delia` já vem ligada do `heroes.db3` do cliente; não precisa de substituição.
- Teste: `cargo test --test lab_frontend` (5).

---

## 4. DLLs

### 4.1 Procedimento de aplicação
- Sempre na cópia `.cache/play-client/bin`, nunca no original (`reject_original`), com o **jogo fechado** (os DLLs ficam carregados enquanto o jogo roda).
- Ordem e pré-requisitos:
  1. `patch_class_table.py --client-copy .cache/play-client --apply` (`.hcls`): `client.dll` `314272ec…` → `a9591d3a…`, `server.dll` `d7727e18…` → `af43431e…`; backups `*.pre-hcls.bak`.
  2. `patch_engine_class_flags.py --client-copy .cache/play-client --apply`: `engine.dll` `a3add974…` → `d25a6e28…` (versão final, passo 43); backup `*.pre-classflags.bak`. Reaplicar a partir desse backup.
  3. `python -s -E scripts/client/patch_class_routing.py --client-copy .cache/play-client --apply`: exige a `.hcls` (`EXPECTED` = `a9591d3a44a2b538`/`af43431e78666783`). Para reaplicar, copiar os `client.dll.<hash>.pre-routing.bak`/`server.dll.<hash>.pre-routing.bak` sobre `bin/` antes. Grava `bin/class-routing.json` (endereços dos *stubs*, lido por `scripts/dev/lab_memory.py`).
- Sem `--apply` é simulação (relatório JSON). `--fallback-entity player_fiona` é só diagnóstico (D8k) e não faz parte do porte.
- Todos os patches conferem os bytes originais de cada ponto e recusam se não baterem.
- **Hashes finais do laboratório** (último passo aplicado, C3-23; `.cache/verification/c3/class-routing-apply-c3r.log`): `client.dll` `973bf082fede76ffb5c71a283aa213856d06dca5782399b23aec006c691cfdc2`, `server.dll` `c44e51a274da9ab07d4922b4b0a0b948bc95b097c6f3e7d11b3038cabf3f3d91` (igual desde o C3-6), `engine.dll` `d25a6e285421d9d0648f79d23f402b25a25ba5fbb0e2202cfe7893b826ee5289`. Lista completa de arquivos e backups na seção 9.
- Testes: `cd scripts/client && python -s -E -m unittest test_patch_class_table test_patch_engine_class_flags test_patch_class_routing` (`test_patch_class_routing.py` tem 36 testes, inclusive um plano com uma segunda classe extra fictícia, Miri 11, e `test_every_new_pointer_is_relocated`).

### 4.2 `patch_class_table.py` (`.hcls`, passo 1 do C2, commit `a6c6304f`)
- Defeito: tabelas de 10 classes fixas. `client.dll` pares {nome, corpo} em `0x38cafd50` (acessores `0x3879ec50`, `0x3879ec60` com `cmp eax, 0xa`); `server.dll` os mesmos pares em `0x40d0cdf8` (`0x40755b40`/`0x40755b50`) e lista de nomes em `0x40d140dc` (`0x408b3980`, `cmp eax, 9`).
- Mudança: seção `.hcls` com 32 posições, preenchida com as 28 classes do NA (`client.dll` NA `0x3958ad38`; Delia corpo 2), acessores apontados para ela, limite 11 (`ACTIVE_CLASSES`); o diretório de relocação passa à seção nova com um bloco a mais.
- Evidência: as 10 primeiras são idênticas às do 1.69; leitura de `Delia` na entrada 10; D7 sem regressão.

### 4.3 `patch_engine_class_flags.py` (`engine.dll`; passos 7 e 43, commits `002404d2`, `e953d7d5`)
- Defeito: o *setter* `0x42245110` (único chamador `0x422be9f8`) desdobra `ClassRestriction` em um `bool` por classe a partir de `item+0x198` e para no bit 9. O registro só tem 10 *flags*: o carregador grava `Indestructible` em `+0x1a2` logo depois (`0x422bea4a`). Toda arma e roupa negava a Delia ("Cannot be equipped", itens vermelhos).
- Mudança: reescrita no lugar do fim do *setter* (`0x42245170`..`0x4224518f`, 32 bytes; sem seção nova, sem *dword* relocado); o bit 10 vai para `DELIA_FLAG` = `+0x1cd` (enchimento depois de `UseInQuest`, `+0x1cc`) por `setne`; a saída antecipada (`je` em `0x42245116`) vai para o `ret 4` novo.
- Evidência: leitura ao vivo dos registros `ItemClassInfo` (passo 0x27c) no D22: Campfire Kit `+0x198..+0x1a1` = 1 e `+0x1a2` = 0; nenhum código do `engine.dll` ou do `server.dll` lê `+0x1cd..+0x1cf`. A primeira versão (passo 7) gravava em `+0x1a2` e era apagada.
- Limite: só uma classe extra cabe (`bool[11]` efetivo); a classe 11 precisa de outro byte livre ou de um vetor maior.

### 4.4 `patch_class_routing.py` (`.hrte`): grupos na ordem em que o patch os aplica
Tabela dirigente `EXTRA_CLASSES` (passo 22):

```python
ExtraClass(10, 'delia', 'hagie', 'female', 'csf/bodyshape_delia_default.fig', (0.88, 0.96), 3,
           weapons=('weapon_greatsword',), server_constants=(0.88, 0.96, 92.0)),
```

Campos: `index` (contíguo a partir de 10, abaixo do pet 28), `key`, `clone_of` (classe cujos casos são copiados e cuja coluna as tabelas de padrão leem), `body` (pasta do esqueleto), `figure`/`constants` (cliente *slots* 264, 177/178), `faces`, `shape_of` (padrão `lynn`), `weapons`, `server_constants` (servidor *slots* 221, 222, 414). `check_classes` recusa índices não contíguos.

| Grupo | DLL e endereços | Defeito corrigido | Evidência / passo |
|---|---|---|---|
| `SWITCHES` | cliente: estado de ação `0x387ED842` (limite 9), expressões `0x3880401A` (10, pet), palavras-chave `0x38861C22` (10, pet), nome em minúsculas `0x387A3AE4` (9), roupas de amostra `0x384A3308` (9); servidor: `0x407EFDBD` (9), `0x40804ACA` (10, pet), `0x407CEB22` (10, pet) | classe 10 no caso padrão ou no do pet; o caso 10 é uma cópia do caso da Hagie com as *strings* da Delia; o pet vai para 28 e 11–27 caem no padrão | passo 3 (`879074ed`); `0x384A3308` no C3-9 (`edbacfa7`): a customização mandava a classe 10 ao *default* (botões de roupa sem comando); NA faz em laço (`0x38d15f9f`) |
| `TABLES['server.dll']` | `0x40CF9E64` `csf/create_{key}.csf`, `0x40CF9E3C` `csf/prologue_{key}.csf`, `0x40CF968C` `Player.{Name}.`, `0x40CFA0B0` `csf/costume_default_{key}.csf`, `0x40CFA0D8` PvP (nulo, como a Hagie); animação `0x40CF1F80` cidade, `0x40CF1FA8` batalha, `0x40CF1FD0` *ExtremeMode*; chaves de nascimento `0x40CFA1B0` `{key}` | tabelas de 10 sem limite: a classe 10 lia a tabela seguinte (esqueleto masculino; valor `0x3d8` na *ExtremeMode*; nascimento como Lethita na customização) | passos 3, 6 (`543eaaac`), C3-6 (`da42c2d7`); 32 posições, 11–31 repetem a 0 |
| `TABLES['client.dll']` + `CLASS_STRING_BOUNDS` | `0x38CB6D24` `{Name}`, `0x38CB6D4C` `{Initial}`; limites `0x388E6AC4`, `0x388E6AE4` | nome/inicial vazios acima de 9 em `SkillDialog.setClassInfo`: habilidades empilhadas num ponto | passo 26 (`5bb15d26`), D12–D13 |
| `SERVER_CLASS_BOUNDS` | `0x4055A336` (`cmp esi, 0xa` → `LAST+1`), `0x4055A194` (`cmp eax, 9` → `LAST`), `0x4055A19E` (`mov ebx, 9` → `LAST`) | o nascimento compara `cl_playerclass` com 10 chaves e chama `SetClass(0)`; `SetClass` (`0x4055a190`) corta em 9: Delia da grade nascia masculina | C3-6, memória com `m_iClass` 0 em `+0x1400`/`+0x18ec` |
| `BITS` | servidor `0x405368C5` (`push 0xa` → `push 0x1c`) | `m_iClass` em rede com 4 bits (0–15) | passo 3; NA usa `log2(28)+1 = 5` bits (`0x68db38`) |
| `GATES` | cliente `0x387033D7` (`cmp eax, 0xa` → `0x1c`) | a preparação do jogador local (`0x387033b0`) era pulada; o objeto de comandos `+0x437c` (`0x388895d0`) não existia e `CHeroesGameMovement` (`0x38789650`) caía em `0x387da6b0` | passo 4 (`116471f4`), D8 → D8b |
| `MATCH` | cliente `0x3885FB93`, `0x38860993`, `0x38860C1D`, comparação `0x38022280`; servidor `0x407CCBA3`, `0x407CD8A3`, `0x407CDB2D`, `0x400CEB80` | `SkillActionStateMatchTable` só tem 10 mapas (`+0x20`..`+0x11c`, passo `0x1c`); a classe 10 lia o mapa por nome: quedas no emote (`0x38a5df98`) e no Mercenary Outpost (`0x40994727`) | passo 5 (`e186e08f`, `a2f68e8d`): 11º mapa na `.hrte` (gravável), laço até 10 e `[esp+0x10]` também trocado (queda `0x407cda68` no D8c) |
| `FLAG_CHECKS` | cliente `0x382D4C34`, `0x388F9C86`; servidor `0x402682E8` | `switch` 0–9 sobre os *flags* do item; classe 10 → falso | passo 8 (`9e02cb16`); lê `DELIA_FLAG` desde o passo 43 |
| `TOOLTIPS` | cliente: bloco da Hagie `0x38410C19`..`0x38410C55` (junção `0x38410C5C`) e `0x385B09D8`..`0x385B0A12` (`0x385B0A18`) | dica sem a Delia, "Cannot be equipped" (`#Heroes_Item_Constraint_CharacterClass0`); cópia do bloco com `#GameUI_Delia` | passo 9 (`663cb25c`); lê `DELIA_FLAG` desde o passo 43 |
| `PRECACHE` | servidor `0x4054DE00` (`PrecacheModel` `0x40114940`) | a lista de pré-carga (registrada em `0x40a09bd3`) não pré-carregava a animação de cidade nem a *ExtremeMode* da Delia; depois, corpo-base e rostos (`DELIA_PRECACHE`: 3 animações, `delia_base.mdl`, `pc_delia_face01..03.mdl`) | passos 9 e 12 (`65e1e639`); D8l: índice de modelo -1 na Delia × 13 na Fiona |
| `COSTUME` | cliente: limite `0x38882FE0`, bloco `0x3888305B`, próxima seção `0x38883525`, leitores `0x3887CBC0`, `0x3887CC60`, `0x3887CC70`, `0x3887CCC0`, `0x3887CD10`; servidor `0x4081A900`, `0x4081A97B`, `0x4081AE45`, `0x40815120`, `0x408151C0`, `0x408151D0`, `0x40815220`, `0x40815270` | o carregador de `player_costume.txt` só reconhece as classes 0–9; a seção `delia` virava peça comum e as leituras da classe 10 caíam na coluna seguinte; objeto-sombra de 4 KiB por classe extra | passo 10 (`d7fc9392`); D8j confirmou a coluna carregada |
| `COSTUME_GATE` | servidor `0x405465DE` (`cmp eax, 9` → `cmp eax, LAST`) | a entrada da roupa do jogador (`0x405465d0`) pulava a montagem `CoStUmE_…` (`0x40546400`) e punha `lethita_base.mdl`: **corpo invisível na cidade** | passo 21 (`cba1da7a`); D8u (nome sintético da Fiona × `lethita_base.mdl` da Delia), D8v passou |
| `CLASS_NAME` | cliente `0x389145B0`, servidor `0x40252880` (comparação `0x38A5DFC6`/`0x40994755`) | nome → classe só conhecia "lethita".."hagie": a seção `delia` do `translated_spskill.txt` era descartada (sem SP) | passo 19 (`b637d5f2`); D8r: os oito valores de `+0x3418` são as SP (gravador `0x407dd6f0`) |
| `CLASS_BITS` | cliente `0x3835DF6E`, `0x38372D03`, `0x385A6763` | `cmp reg, 9; ja` antes de `1 << classe` em `IsAvailableClass`/`addBaseSkillItem`: janela de habilidades vazia | passos 25 e 27 (`d2ee20a8`, `5c90d469`); varreduras `d12-classbit-scan.txt`, `d14-classbit-scan2.txt` |
| `EVENT_FILE` | servidor `0x4081BB0F` (`cmp esi, 9; ja`), *ExtremeMode* `0x408B3A70` | `0x4081bae0` recusava classe > 9: arquivo de eventos da animação vazio (`+0x3f0`, cliente `+0x410`) | passo 15 (`475f22de`); D8n mostrou `…\delia\pc_delia_motion_ex.txt` chegando |
| `SHAPE_CLASS` | cliente `0x3876066F` (`esi`), `0x387606DE` (`ebx`) | gerenciador de forma do corpo pula classe > 9; a classe extra empresta a vaga de `shape_of` (Lynn) | passo 28 (`c0257976`); **sem efeito confirmado** (D15), aplicado "a rever" |
| `CLASS_DEFAULTS` | cliente `0x3887CE30`, `0x3887CF30`; servidor `0x40815390`, `0x40815490` | tabelas `[slot][10]` sem limite (`0x40f6494c`): a classe 10 lia um ponteiro como `m_bodyLayerList[0]` e o cliente desenhava um decalque branco da barriga às coxas na seleção; agora lê a coluna `clone_of` | passo 29 (`a46ce752`); rastreio ao vivo do `SkinMaterialParamModifier`; D17 passou |
| `CLASS_WEAPONS` | cliente `0x38FBC6C0` (montada em `0x3888e4d0`), servidor `0x40F68510` (`0x408256a0`); `WEAPON_SLOTS` 5 | os 5 leitores de cada DLL fazem `cmp R, 0xa; jl; xor R, R`: a Delia recebia a lista da Lethita (`weapon_dualsword`, duas espadas); linha própria com `weapon_greatsword` | passo 31 (`54568be5`); linhas com 5 nomes (`0e812767`) depois da queda `0x40825900` no D18 |
| `CLASS_ALIAS` | servidor `0x4053B5BE` (taxa de estamina), `0x40539AEC` (valor por classe de um `switch` de 10) | taxa de restauração 0 para classe > 9 (48 nas outras, 12 na Evy): estamina quase parada; a classe extra é lida como `clone_of` | passo 32 (`d5d2e1a6`); varredura `d19-class-chains.txt`; D19 passou |
| `CLIENT_WEAPON_SWITCH` | cliente `0x38705F62` (junção `0x38705FB4`) | o nascimento do jogador (`0x38705f30`) pulava a entidade de arma acima de 9; passa `weapons[0]` | passo 32 |
| `CONDITIONS` | cliente: epílogo `0x387F4E79`, mapas `0x38FAB1D8`/`0x38FAB1F4`, limites `0x387DE3FB`, `…44B`, `…55B`, `…5BB`; servidor `0x407F6F39`, `0x40F5D430`/`0x40F5D44C`, `0x407E63CB`, `…41B`, `…47B`, `…4DB` | o registro de condições vai até 120 (`< 0x79`); as 5 condições da Delia (NA 140–144) ganham ids 121–125: `As_Cond_Can_Armor_Purge`, `As_Cond_Delia_SPSkill3_Activated`, `AS_Cond_Delia_Special_Hold_Skill`, `AS_Cond_Delia_Guard`, `AS_Cond_Delia_Guard_Hit` | passo 33 (`d776bbba`); 8.11–8.12 (limite NA 231) |
| `DELIA_EFFECTS` | servidor: registro `0x404FD6D0`, gancho `0x40A087D4`, despachante `0x40838F20`, acerto *slot* 70, armadura `0x148C`/`0x40814620`/`0x408144D0`/`0x40547480`/`0x4054E160`, roupa `0x405465D0`/`0x4081BE60`, conserto `0x40530CF0`, sistema de armadura `0x40740A10`/`0x40D6F5E4` | 5 tipos de efeito sem fábrica no 1.69 (`stamina_adder_on_attack_time`, `delia_restore_health`, `skill_delia_strong_attack`, `damage_modify_to_action`, `restore_equipment_def`); comandos NA `ENABLE/DISABLE_STACK_CHARGE_SMASH_STATUS_EFFECT`, `ENABLE/DISABLE_ADD_STRONG_ATTACK_STATUS_EFFECT`; `PURGE_ALL_ARMOR` ausente (Rose Crush) | passos 39–40 (`470a3121`; NA `0xad89f0`, `0xad8900`, `0xad89a0`, `0xad8880`, `0xad8950`, acerto `0x6e58c0`); purga: passos 44–50 (queda `0x40383bab` resolvida reconstruindo no *think*; peça vestida -1 guardada em `+0x1c`; volta no fim da quebra e no conserto; recálculo `0x4081be60` no barco). Tabela de 16 entradas de `0x40` bytes na `.hrte` |
| `GUARD` | servidor *slots* 618/619/555, rotina da Fiona `0x40575A50`, desligamento `0x4060FE00`, condições `0x405380A0`/`0x407E6470`/`0x407E64D0` | a base não guarda o estado da classe (618 `ret 4`, 619 falso): a guarda animava sem bloquear; tabela `(entidade, estado)` e 124/125 levantadas no golpe defendido | passo 34 (`880bc610`); D19 |
| `CLIENT_GUARD` | cliente *slots* 429/430/367, cópia do golpe `0x38707FB0`, testes `0x38888400`/`0x38888180`, `HIT_IMPACT_SKIP` `+0x4B3D` | o golpe contra o jogador é decidido no cliente: o servidor só bloqueia com o bit 2 de `info+0x78`; decisão da Fiona (`0x3870cef0`) sem os campos dela, 124/125, e o *byte* que pula som e impacto | passos 37 e 44 (`ebd8e0fc`, `de1793a1`); teste com estado forçado em 1 ainda dava dano (8.15) |
| `ITEM_MASK` | cliente `0x384AC4C1` | a máscara de 10 bits de `0x384ac450` (comparação de itens em `0x384af6e0`) sem o bit 10 | passo 41 (`5be60e8e`) |
| `ITEM_COPY` | cliente `0x384E5DBB` | a cópia do registro de item copiava `UseInQuest` como *byte*; o *dword* leva `DELIA_FLAG` | passo 43 |
| `FRAME_KEYS` | cliente `0x387FFFD3` (`strcmp` `0x38A5DFC6`), servidor `0x40800963` (`0x40994755`) | o analisador 1.69 descarta a chave NA `$immediately$` (só a Delia usa, em `spskill_activate_states`): SP1/SP2/SP4/SP5 não saíam pela barra; vira quadro 0 | passo 42 (`427adcbb`); NA cliente `0x1efc53` |
| `CLASS_CONDITIONS` | cliente *think* 334 (ligar `0x387DE550`), servidor 460 (`0x405380A0`, `0x407E6470`) | ninguém ligava `As_Cond_Can_Armor_Purge` (portão da SP5); a classe extra roda a base e liga a 121 | passo 38 (`97d0d795`); NA `0x6e4d30` (*slot* 517) |
| `ENTITY_SLOTS` | cliente `C_HeroesPlayer` (vtable `0x38bb9974`, `m_iClass` `+0x18EC`): 264 figura pela trava da Hagie `0x3870D1D0`, 177/178 = 0,88/0,96; servidor `CHeroesPlayer` (`+0x1400`): 221/222/414 = 0,88/0,96/92 | a entidade base devolve figura nula e constantes da base | passos 22 e 33 (`46272102`, `d5e268a5`); NA *slots* 240/241/466 |
| `CREATION_GRID` | ver 4.5 | criação pela grade | C3-1 a C3-18 |

Diagnóstico opcional, não aplicado: `--fallback-entity player_fiona` (`0x40371314`, passo 11; D8k mostrou que a entidade não era a causa).

### 4.5 `CREATION_GRID` (criação pela grade, fluxo `CharacterCreation_UI_V3` do NA com o filme da grade do 1.69)
Peças, na ordem do fluxo (`CREATION_GRID` em `patch_class_routing.py`, `creation_client`/`creation_server`):

Cliente (`client.dll`):
1. **Registro do *handler* da grade** (`register_test` `0x3858B7D0`, `register_failed` `0x3858B81E`, `register_done` `0x3858B82B`, `register_v2` `0x3858B830`): a partida da interface (`0x3858b7bc`) só registrava o `NewCharacterDialogS3` se `S3_CharacterCreation` já estivesse ligada, e a FeatureMatrix chega depois, no login. Os dois *handlers* (grade e barra V4) passam a ser registrados; a escolha do diálogo continua na hora (`0x38523690`). Causa do D6 e da primeira rodada do C3 (C3-2).
2. **Seletor de mapa** (`map_operand` `0x388E8F49`): `map_background s3_game_create_character` → `changemap lobby_beautyshop_change` (NA `0x380921b0`) (C3-1).
3. ***Handler* da grade** (`handler` `0x3890AB90`, resto `0x3890AE35`): responde `Select<Classe>` das 11 classes com `npc_character_create_select <n>` (em vez do `force_start_touch <classe>` do mapa antigo) e ignora o `OnStartCreation` do filme (C3-1, C3-11).
4. **Mapa pronto** (`start_camera` `0x385239D4`, `start_invoke` `0x38523A02`): manda `npc_character_create;force_start_touch trigger_cam_midview` e invoca `OnReadyCreation` direto (primeira classe e vídeo), só depois do carregamento; antes o vídeo da Lethita tocava no carregamento e reiniciava (C3-11).
5. **Transição para o estado 11** (`transition` `0x3852376D`): `NewCharacterSelection_Start` → `CharacterCreationState` (11, `CREATION_STATE`) em vez do prólogo (8); a grade não passa pelo tutorial (decisão do operador, "Keep the NA flow", C3-5).
6. **Entrada da customização** (`enter` `0x38524B00`): chama `0x38594860(classe)` (`cl_playerclass`/`cl_playermodel`, que o prólogo fazia) antes de `sv_prologue_mode 1`, `scaleform_fade_out 1.5`, `changemap lobby_beautyshop_body`, mais `hhud_should_draw 0` (C3-4, C3-10).
7. ***Slot* 4 de mapa pronto** do `CharacterCreationState` (vtable `0x38B79BC8`): corpo original da entrada (modo de criação, roupa) + `cam_select_beautyshop_camera`, `scaleform_fade_in 1.5`, `cc_show_create_character_dialog`; liga o *flag* "customização aberta" na `.hrte` (C3-1, C3-18).
8. **Saída da customização** (`leave` `0x38524BA0`): desliga o *flag* e segue o comando original (`sv_create_character_mode 0`). O `leave_command` do C3-10 (`…;hhud_should_draw 1`) saiu no C3-20: religava o HUD e mostrava "Lv. 0" na fala da Tieve; os estados da cidade religam o HUD, como no fluxo antigo.
9. **Tabela de transições** (`transitions` `0x3851D6D0`/`0x3851D6D5`, 141 entradas `{estado, evento, função}` em `0x38ca5cc8`): copiada para a `.hrte` com a entrada `(11, CharacterCreation_Back)` → desliga o *flag* de estalagem, `sv_prologue_mode 0`, estado 7 (a grade) (C3-4, C3-20; NA `0x389ac61c`, `CharacterBodyCreation_Back`). Máquina `0x3851E5F0`, `Fire` `0x385194D0`.
10. **Botão Back do diálogo** (`dialog` `0x384AF6E0`, resto `0x384AF6EF`): o *handler* do `CreateCharacterDialog` dispara `CharacterCreation_Back` no comando `Back`; restaura `ecx = esi` antes de voltar (queda `0x384b111d` no "Create", C3-15). As escritas de visibilidade e cor pelo C++ (C3-4 a C3-15) foram retiradas no C3-16; o botão vem do filme (seção 2.4).
11. **Teclas de movimento** (`input` `0x387ED0FA`, resto `0x387ED110`, `move_buttons` `0x618`): com o *flag*, `ProcessInputSequence` (`0x387ed080`) zera também `IN_FORWARD|IN_BACK|IN_MOVELEFT|IN_MOVERIGHT` (C3-18).
12. **Troca imediata de pose/ação** (`emotion_call` `0x388933B4`; `request` `0x387EA050`, `merge` `0x387EDD10`, `set_state` `0x387E49D0`, `kv_delete` `0x38917930`): depois do pedido de `cc_play_repeatable_emotion` (`0x388932c0`), aplica-o na hora (*merge* das opções, `SetState`) em vez de deixá-lo para o próximo `ProcessInputSequence` (C3-18).
13. **Roupas de amostra**: `switch` `0x384A3308` em `SWITCHES` (ver 4.4; C3-9).
14. **Estalagem depois do "Create"** (`inn_vtable` `0x38B79BE0` = vtable do `PrologueInnBState`, estado 12): `CharacterCreation_OK` leva ao estado 12, cuja entrada original (`0x38526620`) abre a fala `prologue_after_create_character` da Tieve. No fluxo antigo a customização já era na estalagem (`create_character.bsp`); na grade ela é na `lobby_beautyshop_body`. Um *flag* de estalagem na `.hrte` (1 = customização da grade aberta, ligado pelo *slot* 4 da customização; 2 = trocando de mapa; o `Back` zera) faz o *slot* 1 do estado 12 mandar `scaleform_fade_out 1.5;changemap create_character` e o *slot* 4 (mapa pronto, original `0x385190A0`) rodar a entrada original já na estalagem. O jogador nasce no `info_player_start` (-624, -1568, 164), longe dos gatilhos (C3-20).
15. **Byte de prólogo** (`set_prologue` `0x38519570`, leitura `0x38519590`, `[[máquina]+4]+4`): o botão "Go to town" (`ToTown`, `npc_talk_dialog.txt`) é tratado em `0x383c4390` e só toma o ramo do prólogo (`force_start_touch trigger_startgame;` em `0x383c452e` → `trigger_once` da `create_character.bsp`: `scaleform_fade_out 3` e, aos 3,5 s, `cc_start_game_first` → `0x38526140` → `Fire PrologueInnB_End` → `0x385263d0` seleciona o índice `contagem - 1` (a personagem mais nova) por `0x38594930` → estado 13 `EnterGameFirstSent` → `EnterGameSent_OK` → cidade) com o byte ligado; desligado, cai em `0x383c453b`: manda a conversa ao servidor (`QueryNpcTalk` 534) e marca a fala como `Loading` (a tela travada do C3-21). O 1.69 liga o byte só no início do prólogo (`0x38526d14`) e desliga ao entrar na cidade vindo do estado 13 (`0x3852e18c`) e em outras saídas (`0x38531275`, `0x385313c0`). A grade pula o prólogo, então o *slot* 4 da estalagem (item 14) liga o byte antes da entrada original (C3-22, C3-23). **Não ligar o byte na customização**: ele também troca o tratamento de mouse/teclado do prólogo (`0x38591500`, `0x385f8b20`) e, com ele, digitar o nome numa pose de andar voltava a girar o modelo e as poses paravam (regressão do C3-22, corrigida no C3-23).
16. **Tentativa descartada (C3-21)**: um *stub* que disparava `cc_start_game_first` no clique do `ToTown`. Partiu de uma premissa errada (o gatilho do mapa falhando); o ramo nem era alcançado, e disparar o evento dentro do *callback* da fala derrubaria o objeto em uso. Retirado no C3-22.

#### 4.5.1 Fluxo completo da grade até a cidade (estado da UI → mapa → peça)
1. Seleção (estado 5) → "nova personagem" → estado 7 (`NewCharacterSelection`): `changemap lobby_beautyshop_change` (item 2); no mapa pronto, `npc_character_create;force_start_touch trigger_cam_midview` e `OnReadyCreation` (item 4).
2. Clique na classe: `Select<Classe>` → `npc_character_create_select <n>` (item 3) → o servidor teleporta os atores (servidor 1) com a pose `SELECTED_S3` (servidor 2).
3. "Start with this character" → `NewCharacterSelection_Start` → estado 11 (item 5): `cl_playerclass`, `sv_prologue_mode 1`, *fade*, `changemap lobby_beautyshop_body`, HUD desligado (item 6); mapa pronto: câmera, *fade in*, diálogo, *flag* da customização e *flag* de estalagem 1 (item 7). O servidor local nasce o jogador com a chave da classe (servidor 3).
4. Customização: roupas de amostra (item 13), poses/ações sem fila (item 12), WASD travado (item 11), nome/"Check" (filme, seção 2.4), `Back` → estado 7 (itens 9 e 10).
5. "Create" → servidor Rust cria a personagem e reenvia a lista (7 entradas no teste) → `CharacterCreation_OK` → estado 12: troca para `create_character` (item 14) → mapa pronto: byte de prólogo (item 15) e a fala da Tieve.
6. "Go to town" → gatilho `trigger_startgame` → `cc_start_game_first` → `SelectCharacter` (índice da mais nova) → estado 13 → `EnterGameSent_OK` → estado 14 (cidade); o byte de prólogo é desligado pelo próprio 1.69.

Servidor (`server.dll`):
1. **`ConCommand` novos** `npc_character_create` e `npc_character_create_select` (NA `0x406db0d0`/`0x406db3f0`), registrados no inicializador estático do `force_start_touch` (`init` `0x409FA70D`, construtor `0x408BC430`, *flags* `0x40`): o ator escolhido vai ao ponto de seleção (`-64, 0, 0`, ângulo 180) e os outros ao de não seleção (`-16, 276, 152`) por `Teleport` (*slot* `0x208`), achados por `FindEntityByName` (`0x401523E0`) (C3-1).
2. **Nome da atividade** (`activity` `0x4040F034`, `0x4040F03F`): o registro da atividade S3 do ator usa `ACT_CHARACTER_CREATION_SELECTED_S3` (existe nos 11 `pc_<classe>_town_motion.mdl`; NA `0x406d9af1`) em vez de `ACT_CHARACTER_CREATION_S3` (pose T na Hagie e na Delia, poses para o mapa antigo) (C3-3).
3. **Tabela de chaves do nascimento + limites do `SetClass`**: `TABLES['server.dll']` `0x40CFA1B0` e `SERVER_CLASS_BOUNDS` (ver 4.4; C3-6).

### 4.6 Ferramentas de rastreio usadas
- `scripts/reverse/v169_class_uses.py` (inventário das 181 leituras de `m_iClass` por DLL), `na_x64_delia_entity.py`, `na_x64_to_169_vtables.py`; crate de referência `tools/na-delia-reference` (fora do build).
- `scripts/dev/lab_memory.py` (`serve` elevado; `entities`, `costume`, `studios`, `heapdword`, `finddword`, `poke` só no laboratório) e `scripts/dev/lab_watch.py` (pontos de parada de escrita confiáveis; os de execução derrubam o jogo).
- Padrão recorrente: `cmp reg, 9`/`0xa` antes de indexar uma tabela de 10 classes.

---

## 5. Servidor Rust

| Módulo | Mudança | Passo / commit |
|---|---|---|
| `src/domain/character_class.rs` | 28 nomes do NA (`NAMES`), `LIMIT` 32, `DELIA` 10, `PET` 28, `is_class` (< 28) | passo 2 (`d90bfe45`) |
| `src/infrastructure/character_creation_core.rs:57`, `class_initial.rs:91/98`, `fiona_initializer_profile.rs:363` | criação e inicializadores aceitam qualquer classe nomeada | passo 2 |
| `src/application/creation_lifecycle.rs:241` + `minimum_assets.rs::supports_class` | a criação exige inicializador, inventário e aparência carregados da classe | passo 2 |
| `character_base_stats.rs`, `character_skills.rs:230`, `initial_inventory.rs:79`, `fiona_character_state.rs:482`, `minimum_character_list.rs:68` | posse `class BETWEEN 0 AND 27` (antes 0–9; a lista e o login já tinham passado a 0–10 em `8060a802`) | passos 2 e "preparação do teste sem patch" |
| `minimum_selection.rs::creating_initializer` (usado por `minimum_growth::base_stats`) | cache de habilidades e atributos de nível 1 pelo perfil gravado no marcador `game_minimum_character_initialization`, não por `initializer_for(classe)` (desconexão da primeira sessão) | "primeira sessão do teste da classe 10" |
| `src/domain/equipped_skills.rs:62–64` | maestria `BASTARDSWORD` → `skill_delia_bastard_mastery` (o conteúdo 1.69 liga o tipo em `expression_table_base.txt`); teste `tests/equipped_skills.rs` | passo 31 (`54568be5`) |
| `lab_frontend.rs` | FeatureMatrix `S3_CharacterCreation = 3.09.00` | C3-1 |
| `admin_tooltip.rs:40`, `admin_characters.rs:320`, `item_editor.rs:33` | nome Delia no painel; `ClassRestriction` até 12 bits no editor | passo 2 |
| `admin_progression.rs` | "até o nível N" aceita 0–27 e usa `CharacterClassInfo` da classe 10 (sem código novo) | passo 35 |
| `minimum_use` | `capsule_ap` com atributo `VALUE` usa o valor como quantidade (relato com a `DeliaLab`; vale para todos) | passo 48 (`6bbee9c0`) |
| `lab_frontend` (`EquipBundle`) + `minimum_mission::refresh_member` | barra de atalhos atualizada no barco depois de um `EquipBundle` | passo 48 |
| `Channel::update_look`, `channel_server::Runtime::update_look`, `lab_frontend::set_look`, `Mission::equip_allowed` | visual do equipamento no barco e na cidade; recusa de equipar em `IN_GAME` (relatado no teste da Delia; vale para todas as classes) | "Correção: visual do equipamento no barco" |
| `minimum_dye` / `domain::dye_ample` | ampola de tintura vazia (sessão paralela, pedida porque a `DeliaLab` não usava a Clodagh Dye Ampoule) | "Ampola de tintura vazia — DyeAmpleSession" (`13024e1`) |

Fora do Rust, mas do servidor: dados de criação (seção 3.2) e `lab-set-class.ps1` (3.3). Imagem Docker reconstruída ao fechar cada grupo (`COMPOSE_PARALLEL_LIMIT=1 docker compose build server`, `docker compose up -d --wait`).

Testes focados: `cargo test --lib -- character_class class_initial initializer`, `--test fiona_initializer_profile`, `--test equipped_skills` (6), `--test lab_frontend` (5), `--lib minimum_use`, PostgreSQL filtrado `selection growth level_up`.

---

## 6. Testes de cliente

### 6.1 `docs/client-tests/C2-delia.md` (encerrado em 29/10/2026)

| Entrada | Parte validada | Resultado |
|---|---|---|
| D0–D1 | `DeliaLab` (Fiona) e itens da Delia | feito |
| D2–D3 (sem patch) | seleção com classe 10; cidade | pose T sem rosto; desconexão (Rust, corrigida), depois queda `server.dll` `0x408b4700` |
| D6 | grade S3 antiga | falhou (mapa com 9 classes); passou ao C3 |
| D7 | `.hcls` | sem regressão |
| D8 → D8b | `SWITCHES`/`TABLES`/`BITS` → `GATES` | queda `0x387da6b0` → cidade carrega |
| D8c | `MATCH` | sem quedas no emote e no Mercenary Outpost |
| D8d–D8n | tabelas de animação, `engine.dll`, `FLAG_CHECKS`, `PRECACHE`, `TOOLTIPS`, `COSTUME`, `EVENT_FILE` | invisível na cidade; seleção mostra a Delia (D8i); inner "Only for Delia" (D8n) |
| D8o–D8u | depurador e memória | gravador das SP achado (`CLASS_NAME`); causa do corpo (`COSTUME_GATE`) |
| D8v | `COSTUME_GATE` | **Delia aparece na cidade** |
| D9–D13 | `EXTRA_CLASSES`, `ENTITY_SLOTS`, `lab-set-class.ps1`, `CLASS_BITS`, `CLASS_STRING_BOUNDS` | habilidades certas (D13) |
| D14–D17 | `SHAPE_CLASS` (sem efeito), `CLASS_DEFAULTS` | faixa branca resolvida (D17) |
| D18 | `CLASS_WEAPONS`, maestria | uma espada; combate ok |
| D19–D20 | `CLASS_ALIAS`, `CLIENT_WEAPON_SWITCH`, `CONDITIONS`, `GUARD` | estamina ok; guarda ainda não bloqueava |
| D21 | levantamento em nível 45 | quatro defeitos (8.15) |
| D22 | `CLIENT_GUARD`, `CLASS_CONDITIONS`, `DELIA_EFFECTS`, `ITEM_MASK`, `DELIA_FLAG`, `FRAME_KEYS` | itens, guarda, contra-ataque, SP, Wild Star, Star Divider ok; som do golpe e Rose Crush pendentes |
| D23 | `PURGE_ALL_ARMOR`, `HIT_IMPACT_SKIP`, conserto, barco, barra no barco | "funcionou perfeitamente" |

### 6.2 `docs/client-tests/C3-criacao.md` (G1–G6)
As caixas do roteiro continuam desmarcadas; os resultados estão nos relatos do `docs/history/session-log.md`:
- G1/G2 (grade, troca de classe): modelos no centro desde o C3-2; poses de apresentação certas e paradas desde o C3-3 (terceira rodada).
- G3 (Fiona/classe antiga): Arisha abre a customização com o modelo certo e troca as armaduras de *preview* (quarta rodada, C3-5). Criação até a cidade não relatada.
- G4 (Delia): customização com o modelo da Delia certo e sem queda (C3-8); botão de voltar aparece e funciona (C3-13); nome e "Check" funcionam (C3-15); "Create" caía em `0x384b111d`, corrigido no C3-15; WASD travado e troca imediata de pose/ação funcionaram (C3-19); "Create" ia direto à fala da Tieve no fundo da customização (C3-20: estalagem); "Go to town" travava em "Loading" (C3-21 diagnóstico, C3-22 byte de prólogo); o byte na customização trouxe de volta WASD e a fila de poses (C3-23: byte só na estalagem). **Passou até a cidade** (30/10/2026).
- G5 (voltar da grade sem criar): não relatado.
- G6 (roupas de amostra): funcionam na Delia (oitava rodada, relato do C3-10).
- HUD "Lv. 0" sobre a personagem: resolvido (C3-10). Travamento depois da senha secundária: sem relação com a grade (C3-11).

---

## 7. Pendências e aproximações

Mecânica e efeitos:
- `As_Cond_Can_Armor_Purge` sempre ligada: não confere armadura nem bloqueio de purga (passo 38).
- `As_Cond_Delia_SPSkill3_Activated` não portada: o contêiner de recarga das SP no 1.69 não foi achado; os seguimentos `delia_SP1_one`/`AttackSP03_one` ficam fechados (passo 38). A SP3 também está comentada no grupo `spskill_activate_states` (D22).
- `SET_GUARD_LATCH_ATTEN` não portado (nenhum conteúdo da Delia usa).
- Ataque forte, lançamento de *ragdoll*, agarrar/segurar alvo, habilidade segurada especial, `DASH_TO_TARGET` (itens 5 e 6 do plano 8.15, E4/E5): não implementados; código de motor novo sem par mapeado no 1.69 (`AE_DELIA_RAGDOLL_LAUNCH`, `START/END_SPECIAL_HOLD_SKILL`, `SET_SAVE_HITTED_VICTIM`, *slot* 730 do NA ↔ 641, etc.; lista em 8.15).
- `AS_Cond_Delia_Special_Hold_Skill` registrada (id 123), mas ninguém a liga.
- Efeitos visuais não portados: `effectName`, `UPDATE/REMOVE_CHARGE_SMASH_EFFECT`, `update_endureReservedHP`; `damage_modify_to_action` usa a primeira taxa para todos; Sharpness +1 até 3 em vez de `mín(nível+2, 3)`; `restore_equipment_def` só soma armadura; `RELEASE_RESTORE_STATUS_EFFECT` não portado (passos 39–40).
- Purga: marca de purga do NA (`+0xaf48`) e visual do capacete (`0x6bd370`) não portados; purgar de novo com a armadura quebrada não muda nada (passos 44, 47).
- Barco: a Rose Crush e o conserto só mexem no visual (sistema de armadura desligado, `0x40740a10`); é o pedido do operador ("só pra bonito", passo 49).
- `ArmorBrokenMessage` (categoria 230) chega ao servidor Rust sem *handler*: a quebra não mexe na durabilidade (passo 47).
- Risco não testado: a quebra por golpe de monstro usa o caminho com aviso (`0x40565b00`) e poderia derrubar o jogo com a Delia (passo 45); o D23 "quebra por golpe" foi marcado como passou.
- Cópias de registro de item que não levam `+0x1cc`/`DELIA_FLAG` (`engine.dll` `0x422a96a0`, cliente `0x38332590`): se uma tela usar essas cópias, o item fica vermelho ali (passo 43).
- `Reload` da `SkillActionStateMatchTable` (console) repete as entradas da Delia, sem queda (8.14).
- *Inner* padrão por classe no servidor local (`0x40815a70`) dá 20 (masculina) à Delia; deveria ser 21 (passo 14). Não corrigido.

Arma:
- `weapon_greatsword` no lugar da `weapon_bastardsword` do NA (o 1.69 não tem a entidade); o tipo `WEAPON_BASTARDSWORD` (29 no NA) não foi registrado, porque nenhum script da Delia o lê (passos 30–31).

Seleção e corpo:
- `SHAPE_CLASS` aplicado sem efeito confirmado ("a rever", D15).
- Defaults de laboratório: `DefaultItemInfo`/`DefaultSkillInfo`/*inner* da Delia são escolha de laboratório no padrão da Hagie, não dados do NA (3.2).

Criação:
- Atores da grade sem `EF_NODRAW` e sem a sequência animada da pose selecionada: o NA usa o comando de cliente `cc_create_character_selection_play_sequence` (cliente NA `0x3888ded0`), ausente no 1.69; os atores ficam na pose estática `ACT_CHARACTER_CREATION_SELECTED_S3`; sem rotação nem idle alternado (C3-3, C3-4).
- Sem os pontos `_select_kalok`/`_select_tall` (Kalok e classes altas podem ficar deslocadas) (C3-1).
- Sem segunda arma na apresentação (o 1.69 não tem os `createcharacter_*_2nd.csf`) (C3-1).
- A grade é o filme do 1.69 (11 classes fixas no AS2); classe 11+ precisa do filme do NA (28 classes) e do contrato de dados do C++ (`SetNewIconCharacter`, `SetCharaterIntroMovie`, `scripts/new_character_info.txt`, ausente no 1.69) (C3-1, 8.8).
- A grade vai direto à customização, sem prólogo/tutorial (Bell Tower), como o NA (decisão do operador, C3-5).
- O NA pede o nome num diálogo à parte (`ShowNameDialog`), que o C++ 1.69 não tem: o filme de laboratório foi editado para manter o campo de nome do `CreateCharacterDialog` (C3-14).
- Texto do nome cinza: o campo não recebe foco do Scaleform e o 1.69 injeta os caracteres; sem alavanca no filme nem no C++ (mantido, decisão do operador, C3-19).
- Os *stubs* de visibilidade do `Button_Back` do C3-8 ao C3-11 foram retirados no C3-16; o botão depende do filme editado da cópia de laboratório.
- Não confirmados: G5 e G3 até a cidade (seção 6.2).
- A música da estalagem só some pelo gatilho do mapa (`sound_bgm_inn`/`ambient_inn` `FadeOut`), como no original.

---

## 8. Receita resumida para a próxima classe (Miri, 11, e seguintes)

Pré-requisitos já prontos: `.hcls` com a Miri na entrada 11 (corpo 2), limite de 32, pet no 28, `m_iClass` com 5 bits, `character_class::NAMES` com a Miri.

1. **Conteúdo (cópia de laboratório).** A Miri não existe no 1.69 (9.1): trazer do HFS do NA (decisão de cópia do operador) modelos (`models/player/miri/`, `_anim/<corpo>/miri/` town/battle/battle_ex), `pc_miri_motion*.txt`, efeitos, sons, `player_action_state_miri*`, `expression_table_miri*`, `miri_skill_keyword_table`, `csf/create_miri.csf`, `prologue_miri.csf`, `costume_default_miri.csf`, `bodyshape_*_miri.fig`, `default_miri.bsh`, seções `miri` de `player_costume.txt` e `translated_spskill.txt`, `createcharacter_miri.csf`, `<SeasonTag>_trial_miri1..4.csf`, ícones e vídeos. Registrar arquivo, origem e SHA-256; sem *commitar* assets. Conferir chaves de quadro e comandos NA novos nos scripts de animação (como `$immediately$`).
2. **`heroes.db3`/conteúdo do servidor.** Converter `ItemClassInfo`/`EquipItemInfo`/`SkillInfo`/`SkillRankInfo`/`CharacterClassInfo` da Miri para o esquema 1.69 (`ClassRestriction` texto → máscara, bit 2048); tipo `DRAKER`; FeatureMatrix `Miri` ligada em `EN-EU-X-GM`.
3. **Criação no servidor.** Linha da classe 11 em `scripts/content/class_additions.json`; reexportar `class-initial.json` e `class-11-initializer.json`; entrada em `docker/lab/minimum-creation-assets.json`; `lab-set-class.ps1` (`ValidateRange`, `$defaults`, `$looks`, `$shapes`, `$figures`); maestria `DRAKER` em `equipped_skills.rs` com teste.
4. **`engine.dll`.** Achar outro byte livre no registro de item para o *flag* da classe 11 (o `bool[11]` efetivo acabou em `+0x1cd`) e estender `patch_engine_class_flags.py`; depois estender `FLAG_CHECKS`, `TOOLTIPS`, `ITEM_MASK`, `ITEM_COPY`, que hoje só tratam a classe 10.
5. **`patch_class_routing.py`.** Acrescentar a linha `ExtraClass(11, 'miri', <clone_of>, 'female', 'csf/bodyshape_miri_default.fig', <constantes NA>, <rostos>, weapons=(...), server_constants=(...))`. Os grupos dirigidos pela tabela (`SWITCHES`, `TABLES`, `MATCH`, `COSTUME`, `EVENT_FILE`, `CLASS_NAME`, `PRECACHE`, `CLASS_BITS`, `CLASS_STRING_BOUNDS`, `SERVER_CLASS_BOUNDS`, `COSTUME_GATE`, `CLASS_DEFAULTS`, `CLASS_WEAPONS`, `CLASS_ALIAS`, `ENTITY_SLOTS`) já aceitam a segunda classe (teste com a Miri fictícia). Rever: `ACTIVE_CLASSES` (11 → 12) em `patch_class_table.py`; os grupos escritos para a Delia (`GUARD`, `CLIENT_GUARD`, `CLASS_CONDITIONS`, `DELIA_EFFECTS`, `CONDITIONS`, `FRAME_KEYS`) só valem se a classe nova tiver as mesmas mecânicas.
6. **Arma.** Achar uma entidade 1.69 equivalente à `weapon_draker` do NA ou portar a entidade; preencher `weapons` com 5 nomes por linha.
7. **Mecânicas próprias.** Ler a entidade da classe no NA x64 (`na_x64_delia_entity.py` como modelo), casar *slots* com o 1.69 (`na_x64_to_169_vtables.py`), registrar as condições novas (ids a partir de 126, limite das 4 funções por DLL) e os tipos de efeito/comandos ausentes.
8. **Criação pela grade.** O filme da grade do 1.69 só tem 11 classes, e o *handler* `Select<Classe>` responde às 11: a classe 11 exige o filme do NA (28 classes) com o contrato de dados no `client.dll` 1.69 (8.8; despachante NA `0x389e06f0`) ou a edição do AS2 do filme do 1.69. Conferir se o `lobby_beautyshop_change.bsp` tem um ator `npc_miri` (o do 1.69 tem só 11) e o `switch` das roupas de amostra (`0x384A3308`). O resto do fluxo (estalagem, byte de prólogo, "Go to town", 4.5 itens 14–15) não depende da classe.
9. **Aplicação e teste.** Jogo fechado; `.hcls` → `engine.dll` (do `pre-classflags.bak`) → roteamento (dos `*.pre-routing.bak`); testes unitários; registrar hashes no `docs/history/session-log.md`; roteiro de cliente na ordem do C2 (seleção, cidade, arma, habilidades, mecânicas, itens) e do C3 (grade, customização, criação, cidade).
10. **Varredura.** Antes de cada teste, procurar `cmp reg, 9`/`0xa` antes de tabelas por classe nos caminhos que a classe nova toca; o inventário de `v169_class_uses.py` lista os pontos conhecidos.

---

## 9. Inventário completo das alterações

### 9.1 Repositório (versionado), por área
Lista gerada com `git diff --name-only a6c6304f~1 HEAD` (primeiro *commit* do porte: `a6c6304f`, 27/10/2026), filtrada para o que é da Delia; lista bruta em `.cache/verification/c3/delia-changed-files.txt`.

- **Patches de DLL** (`scripts/client/`): `patch_class_table.py` (`.hcls`), `patch_engine_class_flags.py` (`engine.dll`), `patch_class_routing.py` (`.hrte`, todos os grupos da seção 4.4 e o `CREATION_GRID`), testes `test_patch_class_table.py`, `test_patch_engine_class_flags.py`, `test_patch_class_routing.py`; `install_lab_launcher.py` (console do motor gravado em log, D8g).
- **Conteúdo e dados de criação** (`scripts/content/`): `class_additions.json`, `content_additions.py`, `export_class_initial.py`, `export_fiona_initializer.py`, `test_content_additions.py`; `docker/lab/minimum-creation-assets.json` (inicializador da classe 10).
- **Ferramentas de laboratório** (`scripts/dev/`): `lab-set-class.ps1`, `lab_memory.py` (+ `test_lab_memory.py`), `lab_watch.py`.
- **Engenharia reversa** (`scripts/reverse/`): `v169_class_uses.py`, `na_x64_delia_entity.py`, `na_x64_to_169_vtables.py`; evidências `docs/evidence/client/na-x64-delia-entity.json`, `na-x64-to-169-vtables.json`; crate de referência `tools/na-delia-reference/` (`Cargo.toml`, `Cargo.lock`, `src/{action_event,client,condition,layout,lib,protected,server}.rs`; fora do build).
- **Servidor Rust** (seção 5): `src/domain/character_class.rs`, `equipped_skills.rs`; `src/infrastructure/character_creation_core.rs`, `class_initial.rs`, `fiona_initializer_profile.rs`, `minimum_assets.rs`, `character_base_stats.rs`, `character_skills.rs`, `initial_inventory.rs`, `fiona_character_state.rs`, `minimum_character_list.rs`, `minimum_selection.rs`, `lab_frontend.rs` (FeatureMatrix `S3_CharacterCreation`), `admin_tooltip.rs`, `admin_characters.rs`, `admin_progression.rs`, `minimum_use.rs`, `minimum_mission.rs`; testes `tests/equipped_skills.rs`, `tests/support/lab_frontend_common.rs`; painel `web/admin` (editor de itens com `ClassRestriction` de 12 bits).
- **Documentação**: `docs/history/session-log.md` (C2 passos 1–50, C3-1 a C3-23), `docs/future/new-classes.md` (8.x, 9.1), `docs/future/delia-class-port.md` (este), `docs/future/delia-character-creation-prompt.md` (*brief* do C3), `docs/new-class-guide.md` (rascunho absorvido aqui), `docs/client-tests/C2-delia.md`, `docs/client-tests/C3-criacao.md`.
- **Mudados no mesmo período, mas não são da Delia** (sessões paralelas): exclusão/reordem de personagens (`character_name_proof.rs`, `minimum_character_delete.rs`, `protocol/character_delete.rs`, `character_initialization_logs.rs`, `minimum_cash_shop.rs`), ampola de tintura (`dye_ample.rs`, `minimum_dye.rs`, `minimum_item_stat.rs`, `minimum_item_logs.rs`, `protocol/dye.rs`, `export_dye_ample.py`), visual no barco (`map_partition.rs`, `channel_server.rs`, `tests/minimum_map.rs`), história pelo painel (`npc_script.rs`, `progression_plan.rs`, `admin_character_edits.rs`), editor de itens (`http/admin/items.rs`, `ItemEditor.tsx`, `ItemPicker.tsx`) e os roteiros B1/B3/B4.
- **Banco**: nenhuma migração nova ou alterada (seção 3.1).

### 9.2 DLLs da cópia de laboratório (`.cache/play-client/bin`, não versionadas)

| DLL | Original 1.69 (backup) | Intermediário | Final (C3-23) | Patch |
|---|---|---|---|---|
| `client.dll` | `314272ec…` (`client.dll.314272ec54054ed0.pre-hcls.bak`) | `.hcls`: `a9591d3a…` (`client.dll.a9591d3a44a2b538.pre-routing.bak`) | `973bf082fede76ff…` | `patch_class_table.py` → `patch_class_routing.py` |
| `server.dll` | `d7727e18…` (`server.dll.d7727e182f6cdec1.pre-hcls.bak`) | `.hcls`: `af43431e…` (`server.dll.af43431e78666783.pre-routing.bak`) | `c44e51a274da9ab0…` | idem |
| `engine.dll` | `a3add974…` (`engine.dll.a3add9742f7fa03c.pre-classflags.bak`) | — | `d25a6e285421d9d0…` | `patch_engine_class_flags.py` |

- Outros backups na pasta: `client.dll.0e1b1800c92dc742.routing.bak`, `server.dll.eda9ac59767a42f6.routing.bak` (aplicações antigas do roteamento) e `client.dll.b284ba719047556b.bak` (anterior ao porte). Não são entrada de nenhum patch atual.
- `bin/class-routing.json`: relatório da última aplicação (endereços dos *stubs*), lido por `lab_memory.py`.
- Logs de aplicação: `.cache/verification/class-routing-apply-*.log` (C2) e `.cache/verification/c3/class-routing-apply-c3*.log` (C3; final `c3r`).

### 9.3 Conteúdo da cópia de laboratório (`.cache/play-client`, não versionado)
- Só `scaleform/`: `CreateCharacterDialog.gfx` (final `ac5543b0…`, cadeia `1a51f689` → `d9f2dd66` → `11973c0c` → `4c8b8275` → `ac5543b0`), `CreateCharacterDialog_I30.dds` (`04084564…`), `_I89.dds` (`9a73326f…`), `_I5.dds` (`04642050…`); backups `*.loose.bak` com o hash do solto anterior (tabela da seção 2.4). Origem: o pacote renomeado do próprio 1.69 (`RENOMEADo_9FB527B3…`), não o NA.
- Nenhum outro arquivo de conteúdo mudou: modelos, scripts, `.csf`, mapas e vídeos da Delia já estavam no 1.69 (seções 2.1–2.2); `heroes.db3` do cliente sem mudança (2.3).

### 9.4 O que veio do cliente NA
Nenhum arquivo do NA foi copiado para a cópia de laboratório nem para o repositório. Do NA vieram só dados e comportamento recuperados, reimplementados nos patches:
- a tabela das 28 classes (nome e corpo; `client.dll` NA `0x3958ad38`) para a `.hcls`, o pet no 28 e `m_iClass` com 5 bits;
- os ids e nomes das 5 condições da Delia (NA 140–144), os 5 tipos de efeito e comandos (`DELIA_EFFECTS`), a chave `$immediately$`, a guarda e a purga (endereços NA na seção 4.4);
- o fluxo `CharacterCreation_UI_V3`: `changemap lobby_beautyshop_change` (NA `0x380921b0`), os `ConCommand` `npc_character_create`/`_select` (NA `0x406db0d0`/`0x406db3f0`), a atividade `ACT_CHARACTER_CREATION_SELECTED_S3` (NA `0x406d9af1`), o `CharacterBodyCreation_Back` (NA `0x389ac61c`), o laço das roupas de amostra (NA `0x38d15f9f`) e o HUD desligado na customização (NA `0x38bd8683`);
- conferências: os `queen_after_trial_delia1..4.csf` do 1.69 são iguais byte a byte aos `global_queen_after_trial_delia1..4.csf` do NA; a comparação dos 485 arquivos da Delia (`.cache/verification/d13-delia-na-vs-lab.txt`) manteve os do 1.69.

