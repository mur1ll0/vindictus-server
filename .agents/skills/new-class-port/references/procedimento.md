# Procedimento do porte, passo a passo

Cada passo termina com: testes focados, entrada no `MIGRATION.md`, entrada no roteiro de cliente quando
há efeito visível, *commit* local. "Delia" cita o que foi feito (seção do `docs/future/delia-class-port.md`
ou passo do `MIGRATION.md`); "Miri" é o que muda para a classe 11. Hipóteses não confirmadas estão
marcadas como **a conferir**.

## 0. Medir a classe (sem patch)
Objetivo: saber o que já existe, o que vem do NA e o que é código novo, antes de pedir decisões.
- **Banco do NA** (`.cache/na-db/heroes-plain.db3`, só leitura): contar por nome da classe as linhas de
  `ItemClassInfo`, `EquipItemInfo`, `SkillInfo`, `SkillRankInfo`, `SkillArgumentInfo`, `CustomizeItemInfo`,
  `CharacterClassInfo`; tipo de arma (`EquipClass`); `FeatureMatrix` da classe. No NA `ClassRestriction`
  é texto (`all`, `female`, `delia,miri`, `female(!delia)`).
- **Banco do 1.69** (cliente e servidor): as mesmas contagens. Delia: tudo presente (`new-classes.md` 7.10).
  Miri: nada (9.1).
- **Arquivos**: listar os caminhos com o nome da classe no `hfsExtracted` do NA e no
  `.cache/play-client/hfs_extracted`; comparar por SHA-256 (Delia: `.cache/verification/d13-delia-na-vs-lab.txt`).
- **Scripts da classe** (`pc_<classe>_motion*.txt`, `player_action_state_<classe>*.txt`,
  `*_seqGroup.txt`): listar chaves de quadro `$...$`, comandos `AE_CUSTOM_MOTION`, condições `As_Cond_*`/
  `AS_Cond_*`, tipos de efeito em `status_effect.txt`/`heroes.db3`, atividades `ACT_*`. Procurar cada nome
  nos DLLs 1.69 (`*.pre-routing.bak`): o que não existe é código a portar ou a aproximar (Delia:
  `$immediately$`, `PURGE_ALL_ARMOR`, 5 condições, 5 tipos de efeito; passos 33, 39–44).
- **Entidade no NA x64**: adaptar `scripts/reverse/na_x64_delia_entity.py` (fábrica `player_<classe>` →
  construtor → vtable, *slots* trocados) e cruzar com `na_x64_to_169_vtables.py`.
- **Modelos**: comparar a versão do cabeçalho `IDST` dos `.mdl` do NA com os do 1.69. A Delia usou só os
  do 1.69; se a versão do NA for mais nova, o motor 1.69 pode recusá-los (**a conferir** com um modelo antes
  de copiar o lote).
- Registrar a medição (seção no documento da classe ou em `new-classes.md`) e a entrada no `MIGRATION.md`
  ("cliente: não necessário").
- **Parar e pedir ao operador**: cópia de assets do NA (lista e origem), escopo das mecânicas (portar ou
  aproximar cada uma), arma (entidade 1.69 existente ou porte), caminho da grade (passo 8).

## 1. Conteúdo do cliente
- Delia: nada copiado; todos os arquivos que os patches pedem já estavam no 1.69 (2.1–2.2 da Delia). A
  tabela 2.2 lista cada arquivo e o grupo de patch que o liga; use-a como lista de conferência.
- Miri (receita 8, item 1): modelos `models/player/miri/`, animações `_anim/<corpo>/miri/` (town, battle,
  battle_ex), `pc_miri_motion*.txt`, efeitos, sons, `player_action_state_miri*`, `expression_table_miri*`,
  `miri_skill_keyword_table`, `csf/create_miri.csf`, `prologue_miri.csf`, `costume_default_miri.csf`,
  `bodyshape_*_miri.fig`, `default_miri.bsh` e a chave em `bodyshape_fig_list.txt`, seções `miri` de
  `player_costume.txt` e `translated_spskill.txt`, `createcharacter_miri.csf`, roupas de amostra
  (`queen_after_trial_miri1..4.csf`; no NA `global_queen_after_…`, **a conferir**), `NewCharacter_Miri.usm`,
  vídeos de habilidade, ícones, `models/weapon/<arma>`.
- Onde pôr: arquivos soltos na cópia. Soltos têm prioridade sobre pacotes em `scaleform/` (C3-12);
  **a conferir** nas outras pastas com o primeiro arquivo (leitura de memória ou console) antes de copiar o lote.
- Arquivos compartilhados do 1.69 (`player_costume.txt`, `translated_spskill.txt`, `bodyshape_fig_list.txt`,
  `skillactionstatematch`, `status_effect.txt`): mesclar só as seções da classe, guardar o original como
  `<arquivo>.<hash>.loose.bak` e registrar antes/depois.
- Registro obrigatório de cada arquivo: caminho de destino, origem (pacote/caminho no NA), SHA-256 e a
  decisão do operador. Nada disso entra no Git.

## 2. `heroes.db3` do cliente e conteúdo do servidor
- **Máscara de classe**: no 1.69 `ClassRestriction` é `int` (`1 << classe`); Miri é o bit 11 (2048).
  Linhas "todas as classes" do 1.69 têm 2047 (11 classes, Delia incluída): para a Miri usar poções,
  *kits* e roupas comuns, essas linhas precisam do bit 11 (4095). Converter as expressões de texto do NA
  classe a classe.
- **Cliente**: Delia sem mudança (2.3). Miri: linhas convertidas de `ItemClassInfo`, `EquipItemInfo`,
  `SkillInfo`, `SkillRankInfo`, `SkillArgumentInfo`, customização e preços; tipo `DRAKER`. O 1.69 tem
  `heroes.db3` solto e empacotado (passo 9 conferiu os dois): saber qual é lido antes de mudar, por um
  *script* reprodutível com *backup*, nunca à mão.
- **FeatureMatrix**: a da Delia já vinha ligada (`EN-EU-X-GM` 3.09.00). Para a classe nova, ou a linha do
  cliente, ou a substituição enviada pelo servidor em `src/infrastructure/lab_frontend.rs` (como
  `S3_CharacterCreation`, teste `cargo test --test lab_frontend`).
- **Servidor**: a Delia já tinha itens, habilidades, `CharacterClassInfo` e `CharacterBaseLoadInfo` no
  `heroesContents.db3`; só faltavam os dados de criação (passo 3). A Miri não tem nada: o mecanismo de
  `scripts/content/content_additions.py` (linhas novas por `TEMP VIEW`, sem tocar o banco restaurado)
  precisa cobrir também itens, habilidades e a classe (**desenho a medir**; o banco de referência nunca é
  sobrescrito).
- Rust: `src/domain/character_class.rs` já tem os 28 nomes (Miri incluída); o editor de itens aceita 12
  bits de `ClassRestriction`. Conferir nomes de classe com 10/11 posições (`new-classes.md` 8.13, "Servidor Rust").

## 3. Dados de criação no servidor
Delia: seção 3.2–3.3 e passo 2 do `MIGRATION.md`. Para a classe nova:
- linha em `scripts/content/class_additions.json` (`default_items`, `default_skills`, `inner`), escolha de
  laboratório declarada quando não vier do NA;
- reexportar `class-initial.json` e `class-<n>-initializer.json` (`export_class_initial.py`,
  `export_fiona_initializer.py`), entrada em `docker/lab/minimum-creation-assets.json`;
- `scripts/dev/lab-set-class.ps1`: `ValidateRange`, `$defaults` (habilidades iniciais), `$looks`
  (roupa-base do `create_<classe>.csf`), `$shapes` (chaves de forma do corpo), `$figures` (figura padrão de
  `bodyshape_fig_list.txt`/`default_<classe>.bsh`);
- maestria da arma em `src/domain/equipped_skills.rs` com teste em `tests/equipped_skills.rs` (Delia:
  `BASTARDSWORD` → `skill_delia_bastard_mastery`; o conteúdo 1.69 já ligava o tipo em `expression_table_base.txt`;
  a Miri precisa dessa ligação também).
- Testes: `python -m unittest test_content_additions test_export_fiona_initializer` (em `scripts/content`),
  `cargo test --lib -- character_class class_initial initializer`, `cargo test --test equipped_skills`.
- Docker só no fechamento do grupo: `COMPOSE_PARALLEL_LIMIT=1 docker compose build server` e
  `docker compose up -d --wait`.

## 4. `engine.dll`: *flag* de classe do item
- Delia (4.3, passos 7 e 43): o *setter* `0x42245110` grava 10 `bool` em `item+0x198..+0x1a1`; `+0x1a2` é
  `Indestructible`; o bit 10 foi para `DELIA_FLAG = +0x1cd` (enchimento depois de `UseInQuest +0x1cc`).
- Miri: precisa de outro byte. Candidato `+0x1ce` (nada lê `+0x1cd..+0x1cf` nos DLLs medidos; **a conferir
  ao vivo**, como no passo 43). O `NEW_TAIL` de 32 bytes está cheio (sobra 1 `nop`): o bit 11 não cabe no
  lugar, então o *setter* precisa saltar para código fora do `.text` (seção nova no `engine.dll` ou outro
  espaço livre, **a medir**).
- Depois, `FLAG_CHECKS`, `TOOLTIPS` e `ITEM_MASK` em `patch_class_routing.py` usam a constante `DELIA`/
  `DELIA_FLAG`: generalizar para um byte por classe extra. `ITEM_COPY` já copia o *dword* `+0x1cc..+0x1cf`.
  As cópias `0x422a96a0` (`engine.dll`) e `0x38332590` (cliente) não levam esses bytes (limite conhecido).
- Reaplicar sempre a partir de `engine.dll.a3add9742f7fa03c.pre-classflags.bak`.

## 5. `EXTRA_CLASSES`, `ACTIVE_CLASSES` e revisão dos grupos
- **`patch_class_table.py`**: a `.hcls` já tem a Miri na entrada 11 (corpo 2), mas `ACTIVE_CLASSES = 11`.
  Subir para 12 muda o *hash* da `.hcls`; o *script* recusa uma cópia já patchada com outro limite. Então:
  restaurar `client.dll.314272ec54054ed0.pre-hcls.bak` e `server.dll.d7727e182f6cdec1.pre-hcls.bak`,
  reaplicar a `.hcls`, atualizar `EXPECTED` em `patch_class_routing.py` (e os *hashes* conferidos por
  `v169_class_uses.py` e pelos testes), e o roteamento passa a gravar novos `*.pre-routing.bak`.
- **Linha nova** (campos em 4.4 da Delia):
  `ExtraClass(11, 'miri', <clone_of>, 'female', 'csf/bodyshape_miri_default.fig', (<NA cliente 190/191>), <rostos>, shape_of=..., weapons=(5 nomes), server_constants=(<NA servidor 240/241/466>))`.
  - `clone_of`: classe antiga cujos casos são copiados com os nomes trocados, cuja coluna as tabelas de
    padrão leem (`CLASS_DEFAULTS`) e cujo comportamento a classe herda nas cadeias de comparação
    (`CLASS_ALIAS`: taxa de estamina 48; a Evy dá 12). A Delia usa a Hagie.
  - Constantes: cliente 177/178 ↔ NA cliente 190/191; figura 264 ↔ NA 301; servidor 221/222/414 ↔ NA
    240/241/466 (âncoras da Hagie em `new-classes.md` 8.11).
  - `faces`: quantidade de `models/player/<classe>/pc_<classe>_faceNN.mdl`.
  - Índices contíguos a partir de 10, abaixo do pet 28 (`check_classes`).
- **Grupos que já seguem a tabela** (nada a escrever, só conferir no teste): `SWITCHES` (inclusive as
  roupas de amostra `0x384A3308`), `TABLES` (inclusive a chave de nascimento `0x40CFA1B0`), `MATCH`,
  `COSTUME`, `EVENT_FILE`, `CLASS_NAME`, `PRECACHE`, `CLASS_BITS` (máscara lida em 16 bits: até a classe
  15), `CLASS_STRING_BOUNDS`, `SERVER_CLASS_BOUNDS`, `COSTUME_GATE`, `CLASS_DEFAULTS`, `CLASS_WEAPONS`,
  `CLASS_ALIAS`, `CLIENT_WEAPON_SWITCH`, `ENTITY_SLOTS`, `SHAPE_CLASS` (sem efeito confirmado), e os nomes
  da grade (`creation_names()`: `Select<Classe>`, `npc_<classe>`).
- **Grupos presos à classe 10** (constante `DELIA`): `FLAG_CHECKS`, `TOOLTIPS`, `ITEM_MASK` (passo 4) e
  `CONDITIONS` (lista `CONDITION_NAMES` da Delia, ids 121–125).
- **Grupos com o comportamento da Delia para toda classe extra** (`class_dispatch` percorre
  `EXTRA_CLASSES` para o mesmo corpo; os ganchos testam `cmp classe, 10; jl`): `GUARD`, `CLIENT_GUARD`,
  `CLASS_CONDITIONS` (liga `As_Cond_Can_Armor_Purge`), `DELIA_EFFECTS` (comandos, acerto, purga, conserto,
  *think*). Antes de acrescentar a Miri, restringir esses grupos à Delia ou dar a cada classe a lista das
  suas mecânicas (campo novo em `ExtraClass`), com teste que prove que a Miri não recebe a guarda nem a purga
  da Delia.
- **Nomes de tamanho diferente**: os casos clonados trocam "hagie" (5 letras) pelo nome novo; o C3-9 anotou
  que o auxiliar das roupas de amostra (`0x384a26a0`) recebe "nomes de 5 letras". "miri" tem 4: **a
  conferir** no teste `test_sample_outfits_*` e no cliente (G6).
- Testes: transformar `test_a_second_extra_class_gets_its_own_entries` (Miri fictícia) em testes reais da
  linha nova; manter `test_every_new_pointer_is_relocated`.

## 6. Arma
- Delia (passos 30–31): a entidade vem de uma lista por classe (`CUtlVector` de 5 nomes; cliente
  `0x38FBC6C0`, servidor `0x40F68510`), indexada pelo número de roupa da arma; a classe extra ganha linha
  própria em `CLASS_WEAPONS`. A Delia usa `weapon_greatsword` (o 1.69 não tem `weapon_bastardsword`).
- **Sempre 5 nomes** por linha (os sem uso vazios): com 1 nome o servidor caiu em `0x40825900` (D18).
- `weapons[0]` vai ao nascimento do jogador no cliente (`CLIENT_WEAPON_SWITCH`).
- Miri: o NA dá `weapon_draker`; o 1.69 tem 9 entidades (`phantomdagger`, `longblade`, `battleglaive`,
  `greatsword`, `pillar`, `staff`, `bow`, `longsword`, `dualsword`). Escolher pela forma e pelo encaixe do
  modelo, ou portar a entidade (decisão do operador, passo 0).
- Tipo de arma `WEAPON_*` (1.69: `BAREHAND` 0 … `PHANTOMDAGGER` 28): registrar o tipo novo só se os scripts
  da classe o lerem por nome (a Delia não lia; passo 31).

## 7. Mecânicas próprias
- Ler no NA x64 os *slots* trocados pela classe e o que cada um faz; casar com o 1.69 pelo mapa de *slots*;
  pares fracos conferidos à mão antes de qualquer patch (`new-classes.md` 8.11–8.12).
- **Condições**: o 1.69 limita os ids a `< 0x79` em 4 funções por DLL; a Delia ocupou 121–125 e o limite
  virou `< 126`. Um conjunto de 121 bits ocupa 4 *dwords* (128 bits): sobram os ids 126 e 127 sem mexer na
  memória do objeto. Mais de duas condições novas exigem medir e aumentar o armazenamento (**a medir**).
- **Efeitos de estado**: registro por nome (`0x404FD6D0`), depois do `stamina_adder_on_attack`; um tipo 1.69
  de mesma semântica pode só receber o nome novo.
- **Comandos de animação e chaves de quadro**: o 1.69 descarta em silêncio o que não conhece
  (`$immediately$`, `PURGE_ALL_ARMOR`); cada um vira gancho no despachante (`0x40838F20`) ou no analisador
  (`FRAME_KEYS`).
- Estado que muda dentro de *callbacks* (efeito, evento de animação) vai para o *think* da classe
  (passos 39–40, 46).
- Cada mecânica: patch, teste unitário e entrada no roteiro; o teste de cliente pode ser uma sessão única no
  fim (D22), salvo quando a seguinte depender da anterior (regra 2 do `AGENTS.md`). Aproximações e o que
  não foi portado vão para a seção "Pendências e aproximações" do documento da classe.

## 8. Criação pela grade
O fluxo inteiro está em 4.5/4.5.1 da Delia. O que depende da classe:
- **Filme da grade**: o `NewCharacterDialog_V2.gfx` do 1.69 tem 11 classes fixas no AS2. Para a classe 11+,
  ou editar o AS2 do filme do 1.69 (botão, ícone, vídeo, `feature_isEnable`), ou usar o filme do NA (28
  classes) e implementar no `client.dll` o contrato de dados (`SetNewIconCharacter`, `SetCharaterIntroMovie`…,
  `scripts/new_character_info.txt`; despachante NA `0x389e06f0`; `new-classes.md` 8.8). Decisão do operador.
- **Ator da cena**: o `lobby_beautyshop_change.bsp` do 1.69 tem 11 `npc_character_creation_actor`
  (`npc_<classe>`); a classe nova precisa do seu ator com `createcharacter_<classe>.csf` (mapa do NA ou
  edição do mapa: decisão de cópia). O servidor acha o ator pelo nome `npc_<classe>`.
- **Pose**: `ACT_CHARACTER_CREATION_SELECTED_S3` precisa existir no `pc_<classe>_town_motion.mdl`
  (sem ela: pose T, C3-3).
- **Customização**: modelo pela chave de nascimento (`TABLES`, `SERVER_CLASS_BOUNDS`); roupas de amostra pelo
  `switch` `0x384A3308` (clonado pela tabela) e pelos `.csf` de amostra e `create_<classe>.csf`.
- **Genéricos, sem mudança por classe**: registro do *handler*, `changemap`, transição para o estado 11,
  `Back`, WASD travado, troca imediata de pose, estalagem, byte de prólogo, "Go to town" (4.5, itens 1–15).
- A edição do `CreateCharacterDialog.gfx` solto (campo de nome, `Back`, cor) vale para todas as classes (2.4).

## 9. Aplicação
- **Jogo fechado** (os DLLs ficam carregados enquanto ele roda). Sempre na cópia `.cache/play-client`.
- Ordem: `.hcls` → `engine.dll` → roteamento. Reaplicar o roteamento exige copiar os `*.pre-routing.bak`
  sobre `bin/` (o *script* recusa um DLL que já tem `.hrte`); o `engine.dll` volta do `*.pre-classflags.bak`.
- Comandos (saída completa para o log; olhar só o código de saída e os *hashes*):
  - `python -s -E scripts/client/patch_class_table.py --client-copy .cache/play-client --apply`
  - `python -s -E scripts/client/patch_engine_class_flags.py --client-copy .cache/play-client --apply`
  - `python -s -E scripts/client/patch_class_routing.py --client-copy .cache/play-client --apply > .cache/verification/<etapa>/class-routing-apply-<rodada>.log`
- Sem `--apply` é simulação (relatório JSON). `--fallback-entity` é só diagnóstico.
- Registrar no `MIGRATION.md` os *hashes* antes → depois de cada DLL e o nome do log; o `bin/class-routing.json`
  guarda os endereços dos *stubs* para o `lab_memory.py`.

## 10. Testes e roteiro de cliente
- Testes focados (log em `.cache/verification/<etapa>/`):
  `cd scripts/client && python -s -E -m unittest test_patch_class_table test_patch_engine_class_flags test_patch_class_routing`.
  Com a cópia de laboratório presente; teste pulado não é evidência. Rust: só os alvos afetados.
- Um job pesado por vez; sem análise do binário inteiro; Docker só no fechamento do grupo.
- Varredura antes de cada teste de cliente: `cmp reg, 9`/`0xa` antes de tabelas por classe nos caminhos
  que a classe toca; ampliar `v169_class_uses.py` quando achar um padrão novo.
- Ordem dos testes de cliente (como C2 → C3 da Delia): personagem de laboratório trocada de classe
  (`lab-set-class.ps1`) para seleção, cidade, arma, habilidades, mecânicas e itens; depois a grade, a
  customização, a criação até a cidade e a volta à seleção; não regressão com a Fiona e a Hagie.
