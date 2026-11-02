# Procedimento do porte, passo a passo

Cada passo termina com testes focados, a linha do `MIGRATION.md` atualizada, uma entrada acrescentada ao `docs/history/session-log.md`, uma entrada no roteiro de cliente quando
há efeito visível e um *commit* local. "Delia" (classe 10) cita o caminho de uma classe cujos arquivos já estavam no 1.69 (seção do
`docs/future/delia-class-port.md` ou passo do histórico). "Miri" (classe 11) cita o caminho de uma classe que veio inteira do NA
(seção do `docs/future/miri-class-port.md` ou passo `M<n>`). Para a próxima classe, siga a Miri. O que ainda não foi confirmado
está marcado como **a conferir**.

## 0. Medir a classe (sem patch)
Objetivo: saber o que já existe, o que vem do NA e o que é código novo, antes de pedir decisões.
- **Medição versionada** (Miri M0): `py -3.13 scripts/reverse/na_class_measure.py --class <classe> --weapon <arma> {db,models,tokens}`,
  com logs em `.cache/verification/<classe>/m0-*.log`.
  - `db`: linhas por tabela do `heroes.db3` do NA (`.cache/na-db/heroes-plain.db3`) e do 1.69. No NA `ClassRestriction` é texto (`all`, `female`, `delia,miri`, `female(!miri)`); no 1.69 é máscara.
  - `models`: versão `IDST` dos `.mdl`. O NA recente usa v45, que se converte para v44 (passo 1).
  - `tokens`: chaves `$...$`, comandos `AE_CUSTOM_MOTION`, condições `As_Cond_*` e eventos `AE_*` dos *scripts* da classe que os DLLs 1.69 não conhecem. São o código a portar ou a aproximar.
- **Entidade no NA x64**: copiar `scripts/reverse/na_x64_miri_entity.py` (fábrica `player_<classe>` → criador → construtor
  → vtable, *slots* trocados, constantes do cliente 190/191 e do servidor 240/241/466) e cruzar com `na_x64_to_169_vtables.py`.
- **Mecânicas nativas**: um *script* de medição por mecânica (`na_x64_draker_weapon.py`, `na_x64_miri_dragon_energy.py`), com
  JSON local em `docs/evidence/client/` (fora do Git).
- **`_ex`**: confira se o `pc_<classe>_battle_motion_ex.mdl` e o `player_action_state_<classe>_ex.txt` do NA são completos.
  Na Miri eram casco e cópia da Delia (M7d, M7e).
- Registrar a medição na seção 2 do documento da classe e no histórico ("cliente: não necessário").
- **Parar e pedir ao operador**: cópia de assets do NA (volume e origem), escopo das mecânicas (portar ou aproximar cada
  uma), arma (entidade 1.69 existente ou porte), conversão de modelos, caminho da grade (passo 8).

## 1. Conteúdo do cliente
- Delia: nada copiado; os arquivos que os patches pedem já estavam no 1.69 (2.1–2.2 da Delia).
- Miri (M1a–M1c, M7a–M8l; `miri-class-port.md` 3.1–3.2): todo o conteúdo vem do NA por dois *scripts* reprodutíveis e um
  único gravador:
  1. `py -3.13 scripts/client/stage_na_class.py --class <classe> --weapon <arma>` prepara em `.cache/private-assets/classes/<classe>` os
     arquivos com o nome da classe ou da arma, exceto relacionamento, menu inicial, *hero path* e CN/JP. Também:
     - fecha as dependências (materiais na ordem `cdmaterials`, `includemodel`, `include` de VMT, `.efx`, irmãos `.vvd`/`.vtx`/`.phy`/`.ani`);
     - converte os `.mdl` v45 (`mdl_v45_to_v44.py`);
     - aplica as transformações de modelo e de *script* (tabela 3.2 da Miri);
     - nunca prepara um caminho que o 1.69 já tem;
     - grava o manifesto `class-assets.json` (origem e SHA-256).
  2. `scripts/client/apply_lab_content_additions.py --client-copy .cache/play-client --apply` (o `start-lab.bat` chama) é o único
     gravador do índice HFS, dos pacotes e do `heroes.db3` da cópia, sempre a partir dos *backups*:
     - pacotes da classe por pasta, nas partes seguintes às do 1.69 (≤ 192 MiB, cache em `lab-backup/class-packs.json`);
     - *scripts* de pastas já existentes na última parte original;
     - mesclagem KeyValues dos 21 *scripts* compartilhados (`kv_merge.py`: só nós que citam a classe e faltam; conflito fica 1.69);
     - textos, `UIElements.xml`, retratos.
- `models/`, `materials/` e `scripts/` só são lidos de pacotes listados no índice. Arquivos soltos valem para `scaleform/` e `cfg/`.
- Volume: a Miri preparou 4.331 arquivos (3,8 GB, 1,27 GiB em pacotes) e levou o jogo a uns 2 GB de memória privada. Com isso
  apareceu um defeito do 1.69 com ponteiros acima de 2 GB (`patch_engine_iocp_sign.py`).
- Nunca troque um arquivo à mão na cópia: o que precisar mudar entra no preparador ou no aplicador.
- **Teclas**: a classe nova usa as teclas do EU. O preparador não traz `cfg/` do NA, e o arquivo `cfg/config_<classe>_ex.cfg` é
  gravado pelo jogo a partir do `config_default_ex.cfg` do EU (armadilhas, seção 10).
- Nada disso entra no Git: manifesto, assets, bancos e pacotes ficam em `.cache/`.

## 2. `heroes.db3` do cliente e conteúdo do servidor
- **Importação** (Miri M2, M3): `py -3.13 scripts/content/class_imports.py --class <classe> --weapon <arma> --index <n> --stats-from <doadora>`
  grava `.cache/private-content/class-imports/<classe>.db3`. Uma conversão serve aos dois lados:
  - colunas do 1.69 (valor do NA ou o mais comum da coluna); `ClassRestriction` → bit `1 << n`; booleanos na forma da coluna;
  - região `enUS` → `enEU` (sem isso nenhum item é oferecido e a aparência de criação é recusada);
  - linhas com *feature* fora da `FeatureMatrix` do 1.69 ficam fora, junto com as dependentes;
  - `FeatureMatrix` da classe com a versão EN-EU da doadora;
  - `CostumeType` renumerado (roupas que colidem com itens do editor; armas `10100 + n`).
- **Servidor**: `content_additions.connect` instala tabelas TEMP `_imported_*` e visões; o banco de referência nunca muda.
  `CharacterClassInfo`/`CharacterBaseLoadInfo` vêm da doadora (o banco do NA é do cliente e não tem atributos de classe).
- **Cliente**: o aplicador restaura as tabelas do *backup* e acrescenta as linhas importadas antes das do editor de itens.
- **Máscara 2047**: as linhas "todas as classes" do 1.69 precisam do bit novo, senão a classe fica sem habilidades comuns,
  histórias e títulos (`class_imports.shared_skills`, `SHARED_TABLES`; Miri MI4, M7v). Itens existentes ganham o bit quando a
  máscara cobre todas as classes do mesmo corpo e a expressão do NA não exclui a classe.
- **Textos**: `class_text` (chaves da classe e dos ids importados), `set_tokens` (nome e bônus de conjunto gerados do
  `SetEffectInfo` quando o NA não tem o texto), `donor_xe_tokens`. O cliente acha o texto da habilidade pelo id.
- **Exportações**: `refresh_item_content.py` refaz as exportações privadas quando muda o `classes_sha256` (banco importado e
  manifesto), inclusive `export_skill_profile.py --all-skills` (sem as habilidades importadas, a seleção é recusada).
- Rust: `src/domain/character_class.rs` já tem os 28 nomes; o editor de itens aceita 12 bits de `ClassRestriction` e o painel
  lista as classes em `web/admin/src/api/characters.ts`/`items.ts` e `admin_tooltip.rs` (acrescente a classe nova).

## 3. Dados de criação no servidor
Delia: seção 3.2–3.3 e passo 2 do `docs/history/session-log.md`. Para a classe nova:
- linha em `scripts/content/class_additions.json` (`default_items`, `default_skills`, `inner`), escolha de
  laboratório declarada quando não vier do NA;
- reexportar `class-initial.json` e `class-<n>-initializer.json` (`export_class_initial.py`,
  `export_fiona_initializer.py`), entrada em `docker/lab/minimum-creation-assets.json`;
- `scripts/dev/lab-set-class.ps1`: `ValidateRange`, `$defaults` (habilidades iniciais), `$looks`
  (roupa-base do `create_<classe>.csf`), `$shapes` (chaves de forma do corpo), `$figures` (figura padrão de
  `bodyshape_fig_list.txt`/`default_<classe>.bsh`);
- maestria da arma em `src/domain/equipped_skills.rs` com teste em `tests/equipped_skills.rs` (Delia:
  `BASTARDSWORD` → `skill_delia_bastard_mastery`; Miri: `DRAKER` → `skill_miri_draker_mastery`, M3);
- o NA não traz `DefaultItemInfo`/`DefaultSkillInfo` (tabelas do servidor): os itens e as habilidades iniciais são escolha
  de laboratório registrada no documento da classe (Miri: consumíveis e ouro da Delia, `draker_newbie`, maestria,
  `skill_miri_speedmove`, `miri_inner_free`);
- o teste privado das classes (`tests/private_class_content.rs`) roda com `LAB_SKIN`/`LAB_EYE` definidos, porque a cor padrão 0
  é recusada na criação (Miri M3).
- Testes: `python -m unittest test_content_additions test_export_fiona_initializer` (em `scripts/content`),
  `cargo test --lib -- character_class class_initial initializer`, `cargo test --test equipped_skills`.
- Docker só no fechamento do grupo: `COMPOSE_PARALLEL_LIMIT=1 docker compose build server` e
  `docker compose up -d --wait`.

## 4. `engine.dll`: *flag* de classe do item
- O *setter* `0x42245110` grava dez `bool` em `item+0x198..+0x1a1`; `+0x1a2` é `Indestructible`. Desde o M4,
  `patch_engine_class_flags.py` troca o fim do *setter* por um salto para a seção `.hflg`, onde um *stub* grava um *byte* por classe extra
  (`CLASS_FLAGS`: 10 → `+0x1cd`, 11 → `+0x1ce`, bytes de enchimento depois de `UseInQuest +0x1cc`).
- Próxima classe: uma entrada a mais em `CLASS_FLAGS`. O candidato é `+0x1cf`, último *byte* do *dword* que o `ITEM_COPY` copia
  (**a conferir** com a varredura estática do M4, `.cache/verification/miri/m4-scan-1ce.log`, e ao vivo). Depois dele não há mais
  enchimento conhecido.
- `FLAG_CHECKS`, `TOOLTIPS` e `ITEM_MASK` de `patch_class_routing.py` leem `CLASS_FLAGS`.
- O *script* sempre parte do original e, por isso, desfaz o patch IOCP: depois dele, rode `patch_engine_iocp_sign.py` de novo
  (`docs/client-dll-patches.md`).

## 5. `EXTRA_CLASSES`, `ACTIVE_CLASSES` e revisão dos grupos
- **`patch_class_table.py`**: a `.hcls` tem as 28 classes do NA; `ACTIVE_CLASSES` = 12 desde a Miri. Para subir:
  1. restaurar os `*.pre-hcls.bak` (`client.dll.314272ec54054ed0`, `server.dll.d7727e182f6cdec1`);
  2. reaplicar a `.hcls`;
  3. atualizar o `EXPECTED` de `patch_class_routing.py` e o de `v169_class_uses.py` no mesmo *commit*.
- **Linha nova** (a da Miri, M5):
  `ExtraClass(11, 'miri', 'hagie', 'female', 'csf/bodyshape_miri_default.fig', (0.865, 0.945), 3, weapons=('weapon_greatsword',), server_constants=(0.865, 0.945, 92.0))`.
  - `clone_of`: classe antiga cujos casos são copiados com os nomes trocados e cuja coluna as tabelas de padrão leem (`CLASS_DEFAULTS`, `CLASS_ALIAS`). A Delia e a Miri usam a Hagie.
  - Constantes: cliente 177/178 ↔ NA cliente 190/191; figura 264 ↔ NA 301; servidor 221/222/414 ↔ NA 240/241/466 (`na_x64_<classe>_entity.py`).
  - `faces`: quantidade de `pc_<classe>_faceNN.mdl`. Índices contíguos a partir de 10, abaixo do pet 28 (`check_classes`).
- **Grupos que seguem a tabela** (só conferir no teste): `SWITCHES`, `TABLES` (inclusive a chave de nascimento `0x40CFA1B0`),
  `MATCH`, `COSTUME`, `EVENT_FILE`, `CLASS_NAME`, `PRECACHE`, `CLASS_BITS` (16 bits: até a classe 15), `CLASS_STRING_BOUNDS`,
  `SERVER_CLASS_BOUNDS`, `COSTUME_GATE`, `CLASS_DEFAULTS`, `CLASS_WEAPONS`, `CLASS_ALIAS`, `CLIENT_WEAPON_SWITCH`, `ENTITY_SLOTS`,
  `SHAPE_CLASS`, `FLAG_CHECKS`, `TOOLTIPS`, `ITEM_MASK`, `KEYWORD_TABLES` (vetor de palavras-chave com `PET + 1` entradas; sem ele a
  classe 11 caiu em `server.dll+0x8b5472`), os nomes da grade (`creation_names()`).
- **Mecânicas presas a uma classe**: `class_dispatch` recebe a lista de classes de cada grupo. As da Delia ficam em
  `DELIA_MECHANICS` = `(10,)`; o *endure* em `GUARD_CLASSES`. Os grupos da Miri comparam com 11. A classe nova não herda nada
  disso sem entrar na lista; o teste `test_delia_mechanics_stay_on_delia` prova o isolamento.
- **Tabelas estáticas por classe com corte em 10 ou 11** aparecem fora da lista conhecida (vetor de palavras-chave,
  `characterImageLabel` no `SelectCharacterDialog_V2.gfx`). Varra `cmp reg, 0xa`/`0xb` antes de tabelas nos caminhos novos.
- **Nomes de tamanho diferente**: os casos clonados trocam "hagie" (5 letras) pelo nome novo. "miri" tem 4 e funcionou na
  seleção e na cidade; as roupas de amostra da grade (`0x384a26a0`) estão **a conferir** na sessão da grade (`MG<n>`).
- Testes: `test_miri_gets_her_own_entries` e os testes de roteamento, que valem para toda classe extra; manter
  `test_every_new_pointer_is_relocated`.

## 6. Arma
- A entidade vem de uma lista por classe (`CUtlVector` de 5 nomes; cliente `0x38FBC6C0`, servidor `0x40F68510`), indexada pelo
  `CostumeType` do item: 10000–19999 → *slot* 0, 20000–29999 → *slot* 1, senão `CostumeType / 64` até 4 (servidor `0x40823590`).
  A classe extra ganha linha própria em `CLASS_WEAPONS`, sempre com 5 nomes (com 1 nome o servidor caiu em `0x40825900`, D18).
- A entidade pega o modelo no `SubTypeInfoOverride` do seu *script* pelo `CostumeType`. Arma do NA sem entidade no 1.69 (Miri,
  `weapon_draker` → `weapon_greatsword`): os subtipos do NA entram no `weapon_greatsword.txt` como `10100 + n`, *slot* 0 e acima dos
  10001–10009 da espada grande (`class_costumes.LAB_WEAPON_ENTITY`, `WEAPON_SUBTYPE_BASE`, `weapon_costumes`; os itens recebem o
  mesmo número em `class_imports.converted`). 20000 + n e 5000 + n deixaram a Miri sem arma (MI5).
- **Rastros**: cada subtipo leva o `TrailData` da arma do NA, cortado para caber nas 12 alças do `C_PlaneTrail` (rastros × largura
  de `attack_points` ≤ 12; `class_costumes.fitting_trails`). Passar disso derrubou o cliente em `0x38828069` (M7u).
- **Mecânica da arma em código no NA** (corrente da Draker, M7f): o NA cria peças pelo servidor; a cidade do 1.69 não tem
  servidor local, então a peça vai para o *think* do cliente (`CHAIN`), com *precache* do modelo e teste de cidade pelo estado do
  jogo.
- Tipo de arma `WEAPON_*`: registrar um tipo novo só se os *scripts* da classe o lerem por nome. A maestria vai para
  `src/domain/equipped_skills.rs` (`DRAKER` → `skill_miri_draker_mastery`).

## 7. Mecânicas próprias
- Ler no NA x64 os *slots* trocados pela classe e o que cada um faz; casar com o 1.69 pelo mapa de *slots*; pares fracos
  conferidos à mão antes de qualquer patch (`new-classes.md` 8.11–8.12).
- **Condições**: os ids novos ficam em 121–127 (o conjunto tem 128 bits; os limites `cmp ecx, 0x79` viraram `0x7f`). Estão
  **todos ocupados** (Delia 121–124, Miri 125–127): a próxima classe com condições próprias precisa ampliar o conjunto (**a medir**).
  Os testes calculam os ids por `CONDITION_NAMES`; não fixe números.
- **Comandos e chaves que o 1.69 descarta em silêncio**: primeiro tente convertê-los no preparador para um comando do 1.69 de
  mesmo efeito (`REDUCE_DRAGON_KNIGHT_SP_GAUGE` → `AE_GAIN_SP_GAUGE -200`; `AE_CL_PLAY_CUSTOM_EFFECT` → `AE_CL_PLAY_EFFECT`). Quando
  o efeito depende de estado, vira gancho no DLL (`CUSTOM_EFFECT` no `FireEvent` do cliente, *slot* 200).
- **Estado visível a outros jogadores**: servidor como autoridade, por um campo de rede existente. O Flamebreath usa
  `m_gaugeValue` e o Dragon Knight usa `m_gaugeType` da barra de febre, sem mexer no protocolo. O cliente só desenha (HUD).
- **Efeitos de estado novos do NA**: registrar o nome como tipo sem operação quando o efeito do 1.69 já faz o resto
  (`dragonknight_transform`) e pôr o comportamento no *think* da classe.
- **Visual em código no NA** (asas, olhos, dragão invocado): procure um caminho de dados do 1.69 antes de escrever código.
  A Miri usou a peça de costas do `.csf` para as asas, `AE_CREATE_ATTACKABLE_MODEL` para o dragão e `.efx` de laboratório para os
  olhos. Só o que não tem caminho vira grupo (`SUMMON_PARENT`, `SUMMON_FOLLOW`).
- **Pulo, ataques no ar e XE** de uma doadora 1.69 (SKILL.md, seção do pulo).
- Estado que muda dentro de *callbacks* (efeito, evento de animação) vai para o *think* da classe (passos 39–40 e 46 da Delia).
- Cada mecânica: patch, teste unitário e entrada no roteiro. O teste de cliente pode ser uma sessão única no fim (D22), salvo
  quando a seguinte depender da anterior (regra 2 do `AGENTS.md`). Aproximações e o que não foi portado vão para a seção
  "Pendências e aproximações" do documento da classe.

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
- Geração, versionamento, reconstrução a partir dos originais e reaplicação: `docs/client-dll-patches.md`. Resumo:
  - ordem: `patch_shop_list_limit.py` (só `client.dll`) → `.hcls` → roteamento; `engine.dll` *flags* → IOCP;
  - reaplicar o roteamento exige copiar os `*.pre-routing.bak` sobre `bin/` (o *script* recusa um DLL que já tem `.hrte`);
  - o `engine.dll` volta do `*.pre-classflags.bak`, e o IOCP precisa ser aplicado de novo depois.
- Comandos (saída completa para o log; olhar só o código de saída e os *hashes*):
  - `python -s -E scripts/client/patch_class_table.py --client-copy .cache/play-client --apply`
  - `python -s -E scripts/client/patch_engine_class_flags.py --client-copy .cache/play-client --apply`
  - `python -s -E scripts/client/patch_engine_iocp_sign.py --client-copy .cache/play-client --apply`
  - `python -s -E scripts/client/patch_class_routing.py --client-copy .cache/play-client --apply > .cache/verification/<etapa>/class-routing-apply-<rodada>.log`
- Sem `--apply` é simulação (relatório JSON). `--fallback-entity` é só diagnóstico.
- Conteúdo: preparar (`stage_na_class.py`, `class_imports.py`) e abrir pelo `start-lab.bat`, que refaz as exportações e aplica
  o conteúdo (cerca de 15 min quando o manifesto muda).
- Registrar no histórico os *hashes* antes → depois de cada DLL e o nome do log. O `bin/class-routing.json` guarda os endereços
  dos *stubs* para o `lab_memory.py`.

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
