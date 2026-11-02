---
name: new-class-port
description: >
  Procedimento para portar uma classe do cliente NA oficial para o laboratório Vindictus 1.69, a partir
  dos portes da Delia (classe 10, arquivos já no 1.69) e da Miri (classe 11, conteúdo inteiro do NA:
  modelos, animações, efeitos, scripts, banco e mecânicas nativas). Use quando o operador pedir
  para "trazer", "portar" ou "habilitar" uma classe que o 1.69 não tem, ou para continuar um porte já
  iniciado (conteúdo, heroes.db3, dados de criação, engine.dll, EXTRA_CLASSES, arma, mecânicas, criação
  pela grade, testes de cliente). Não use para variantes de classes existentes, itens ou modelos novos
  das 10 classes antigas, nem para trocar a base do projeto pelo cliente NA.
metadata:
  author: vindictus-server (escrita a partir dos portes da Delia e da Miri)
  version: "2.1.0"
  date: 2026-11-02
---

# Porte de uma classe nova do cliente NA para o 1.69

Há dois exemplos completos. `docs/future/delia-class-port.md` cobre os mecanismos comuns: tabela de classes, roteamento e
grade de criação. `docs/future/miri-class-port.md` é a referência para uma classe que vem inteira do NA: ciclo de preparação
(3.1), conversões de assets e *scripts* (3.2), mecânicas (3.3), armadilhas (3.4), pendências (3.6) e receita (6). Esta skill
não repete os dois documentos: ela diz em que ordem trabalhar e o que costuma dar errado. Cite a seção do documento sempre que
precisar do endereço, do grupo ou do passo exato. Os DLLs são gerados só por código versionado (`docs/client-dll-patches.md`).
Nunca edite um DLL à mão.

## Quando usar
- O operador pede uma classe que existe no NA e não no 1.69 (feitas: Delia 10, Miri 11; a próxima é a classe 12).
- Continuar um porte de classe já começado (algum passo do procedimento abaixo).
- Revisar um patch de classe extra em `scripts/client/patch_class_*.py` ou `patch_engine_class_flags.py`.

## O que a skill não faz
- Não autoriza copiar assets do NA para a cópia de laboratório: cada cópia é decisão explícita do operador.
- Não troca a base para o cliente NA (caminho B de `docs/future/new-classes.md` 4.B).
- Não substitui as regras do `AGENTS.md` (commits, testes focados, um job pesado por vez, logs em
  `.cache/verification`, sem assets nem credenciais no Git). Em conflito, vale o `AGENTS.md`.
- Não declara a classe pronta sem o teste de cliente (checklist final abaixo).
- Não cobre segundas armas (Hurk `TEIDE`, Lynn `BLUTE`) nem o Dullahan (`new-classes.md` 9.2–9.3).

## Antes de começar
Leia, nesta ordem:
1. `AGENTS.md`, o `MIGRATION.md` e, por busca (`rg -n "<passo>" docs/history/session-log.md`), as entradas do histórico que o passo cita.
2. `docs/future/miri-class-port.md` (seções 1, 3 e 6) e `docs/future/delia-class-port.md` (4.4, 4.5, 4.5.1, 7, 8 e 9).
3. `docs/client-dll-patches.md` (como os DLLs são gerados, reconstruídos e reaplicados).
4. `docs/future/new-classes.md` 8.13 (limites de 10 classes), 8.15 (mecânicas) e 9 (classes seguintes).
5. `references/fontes-e-ferramentas.md` (clientes, cópia de laboratório, ferramentas).

Confirme com o operador, antes de qualquer patch: a classe, a decisão de cópia de assets do NA e o
escopo das mecânicas próprias (o que portar e o que aproximar). Sem isso, pare (regra 3 do `AGENTS.md`).

## Procedimento (ordem de trabalho)
Detalhes, comandos e o que muda da Delia para a Miri: `references/procedimento.md`.

0. **Medir** a classe no NA e no 1.69 (conteúdo, arquivos, scripts, chaves e comandos novos); nada é patch.
1. **Conteúdo** do cliente: `stage_na_class.py` prepara os arquivos e as transformações (manifesto com origem e SHA-256) e
   `apply_lab_content_additions.py`, o único gravador da cópia, monta pacotes e mesclagens. Só com decisão do operador.
2. **`heroes.db3` e conteúdo do servidor**: `class_imports.py` converte as linhas do NA para o esquema 1.69 (máscara
   `1 << índice`, região, booleanos, máscara 2047, textos, conjuntos) para servidor e cliente.
3. **Dados de criação** no servidor Rust: `class_additions.json`, exportação, `minimum-creation-assets.json`,
   `lab-set-class.ps1`, maestria em `equipped_skills.rs`.
4. **`engine.dll`**: *flag* de classe do item para o índice novo (o `bool[11]` acabou na Delia).
5. **`EXTRA_CLASSES`** e **revisão dos grupos** de `patch_class_routing.py`, mais `ACTIVE_CLASSES` em
   `patch_class_table.py`. Mecânicas exclusivas ficam presas ao índice da classe (`DELIA_MECHANICS`, `GUARD_CLASSES`).
6. **Arma**: entidade 1.69 mais próxima (5 nomes por linha), subtipos `10100 + n`, rastros ≤ 12 alças.
7. **Mecânicas próprias**: entidade NA x64 → *slots* 1.69; primeiro converter no preparador, depois grupo de patch, com o
   servidor como autoridade. Os ids de condição 121–127 acabaram: a próxima classe precisa ampliar o conjunto.
8. **Criação pela grade**: filme, ator `npc_<classe>`, roupas de amostra; estalagem, byte de prólogo e
   "Go to town" já são genéricos.
9. **Aplicação** com o jogo fechado, a partir dos *backups*, na ordem `.hcls` → roteamento e `engine.dll` *flags* → IOCP
   (`docs/client-dll-patches.md`); conteúdo pelo `start-lab.bat`.
10. **Testes** focados e **roteiro de cliente** por etapa; teste no cliente na ordem seleção → cidade →
    arma → habilidades → mecânicas → itens → grade → customização → criação → seleção.

Antes de cada teste de cliente, procure `cmp reg, 9`/`0xa` antes de tabelas por classe nos caminhos que
a classe nova toca (`scripts/reverse/v169_class_uses.py` lista os pontos conhecidos).

## Pulo, ataques no ar e XE nas classes novas (decisão do operador, 31/10/2026)
A Nexon removeu o pulo e o XE *move* (`ExtremeMode`) em determinado momento: as classes lançadas depois
(da Miri em diante) não têm pulo, ataques no ar, XE nem `_ex` próprio no NA (os `_ex` delas são cópias de
outra classe ou cascos de uma sequência). O laboratório roda com `ExtremeMode`, que lê só os arquivos
`_ex`. Regra: **migrar o que falta a partir de uma classe doadora do 1.69 de mesmo esqueleto** (feminino:
Delia) **e implementar para a classe um XE *move* como habilidade extra**. O que a Miri já faz
(`stage_na_class.py`, `apply_lab_content_additions.py`; `docs/future/miri-class-port.md` 3.2 e 4, M7d–M7k):
- `_ex` a partir dos arquivos completos da classe: modelo (`extreme_motion_sibling`), estados e eventos
  (`extreme_scripts`), com os *includes* trocados pela variante `_ex` do 1.69 (`with_extreme_includes`;
  o `common_state_movement.txt` do 1.69 tem os pulos comentados);
- modelo da doadora incluído para as sequências que faltam (`jump_donor`: `jump_*`, `jump_attack*`,
  `super_power`, busca de núcleos), com os blocos de evento dela (`with_included_events`, inclusive o
  `jump_town`, que o 1.69 guarda no roteiro `_ex` da classe) e as ativações `Jump*` nos blocos de mesmo nome
  (`with_jump_activations`);
- estados da doadora que a classe não tem (`DONOR_STATES`: `JumpAttack`, `JumpAttackStrong`);
- tabelas lidas no lugar das comuns com `ExtremeMode` recebem as linhas da classe (`EXTREME_MERGES`:
  `action_table_player_ex.txt`; sem ele os golpes ficam sem ação, dano mínimo e sem SP).
XE *move* (feito na Miri, M7t: `class_imports.donor_xe_rows`, `DONOR_STATES` com `donor_renames`, ativações em `with_jump_activations`): habilidade nova `skill_<classe>_xe_move` (linhas de `SkillInfo`/`SkillRankInfo`
no esquema 1.69, recarga `skill_xe_move_cooltime`), estado `SuperPower` copiado da doadora com
`required_skill` e `is_weapon` da classe, ativações `SuperPower` nos eventos e a ação do golpe (`super_power`
usa a da doadora). Os nomes e valores vêm da doadora do 1.69, não do NA, e ficam registrados no documento
da classe.

## Armadilhas (Delia e Miri)
Evidência e passo de cada uma: `references/armadilhas.md`.
- **Rastros de arma cabem em 12 alças** no cliente 1.69 (`C_PlaneTrail`, rastros × faixa de `attack_points`
  ≤ 12; a espada grande usa 4 × 3). Passar disso derruba o cliente ao montar a arma (Miri M7u,
  `class_costumes.fitting_trails`).
- **Máscara "todas as classes" do 1.69 é 2047** (classes 0–10): linhas de `ItemClassInfo`, `SkillInfo`,
  `StoryLineInfo` e `TitleInfo` com ela precisam do bit da classe nova, senão a classe fica sem histórias
  (Miri M7v, `class_imports.SHARED_TABLES`).
- **Texto de habilidade vem do id** (`HEROES_SKILL_NAME_<id>`, `HEROES_SKILL_DESC_<id>_ex`), não do
  `SkillName` da tabela: habilidade copiada de doadora precisa dos textos renomeados (Miri M7v).
- **`onepiece` no `player_costume.txt`** aponta para outra entrada da mesma seção (corpo inteiro); traga a
  entrada e o modelo junto, ou peitoral e calça se escondem (Miri M7u).
- **Limites de 10 classes** aparecem como `cmp reg, 9; ja`, `cmp reg, 0xa; jge`, `cmp R, 0xa; jl; xor R, R`
  (lê a linha 0), tabelas sem limite (lê a tabela seguinte) e o pet no caso 10. Procure todos antes de
  testar; cada um falha de um jeito diferente (queda, pose T, corpo invisível, lista vazia).
- **Nascimento no servidor local**: tabela de chaves `0x40CFA1B0` e corte do `SetClass` (`0x4055A190`);
  sem eles a classe da grade nasce como classe 0 (C3-6).
- **Byte de prólogo só na estalagem**: na customização ele troca mouse/teclado e quebra WASD e poses; sem
  ele, "Go to town" trava em "Loading" (C3-21 a C3-23).
- **Arquivo solto × pacote renomeado**: arquivos soltos de `scaleform/` têm prioridade e o pacote
  `RENOMEADo_9FB527B3…` não é lido; confira qual arquivo o cliente carrega antes de analisar (C3-12/13/19).
- **Registradores nos *stubs***: restaure `ecx` (e o que o original relê da pilha) antes de voltar ao
  código original (C3-15, D8c, passo 22).
- **Sem mudança de estado dentro de *callbacks***: não dispare eventos da máquina de UI dentro do *callback*
  de uma fala ou diálogo, nem reconstrua o modelo dentro do despacho de eventos de animação; adie para o
  *think* ou para o caminho original (C3-21, passo 46).
- **Memória antes de código**: depois de uma correção sem efeito, leia a memória do jogo
  (`lab_memory.py`) em vez de empilhar hipóteses estáticas (D8g, passo 43, C3-21 → C3-22).
- **Heredocs do Bash quebram escapes de *bytes*** (`\x..`, `\\`): grave *scripts* com a ferramenta de
  escrita em `.cache/` ou no *scratchpad* e rode o arquivo.
- **`ExtremeMode` lê só os `_ex`**: modelo, estados, eventos, *action table* e `include` `_ex` da classe precisam vir completos
  (no NA podem ser casco ou cópia de outra classe; Miri M7d, M7g, M7k).
- **`include` ausente no 1.69 invalida o arquivo de estados** (Miri M7b); **tabelas com corte em 11** também existem (vetor de
  palavras-chave, `characterImageLabel`; Miri MI4, M7p).
- **Arma**: o `CostumeType` escolhe também a entidade na lista por classe; use `10100 + n` (Miri M7a).
- ***Strings* nunca no meio do código de um *stub*** (queda `0x392204e4`, Miri M8h); **estado do jogo pelos predicados do
  `GameStateManager001`**, nunca pelo nome do nível (Miri M7f, M7p, M8m).
- **Nada trocado à mão na cópia**: preparador, aplicador e *scripts* de patch são a única fonte (`docs/client-dll-patches.md`).
- **Pele atravessando os conjuntos próprios da classe** (armaduras do NA com partes dentro do corpo em repouso): confirme pelo
  render no Blender e infle 0,3 ao longo da normal nos `.vvd` das peças de corpo, em lote no preparador
  (`stage_na_class.inflated_armor`), ou peça a peça com o **Push Mesh Out** do *add-on* `tools/blender/vindictus`, publicando o
  kit como modelo substituto no editor de itens (Miri MG4–MG6; `miri-class-port.md` 3.7).
- **Grade e customização**: cabelos por prefixo (`prefixo + NN.mdl`), preços negativos do NA, faixas de Height/Bust por *switch*
  de classe no GFx, modelos e textos citados só por *scripts* ou colunas (Miri MG2–MG3).
- **Teclas são do EU e do jogo**: não traga `cfg/` do NA; o jogo grava `config_<classe>_ex.cfg` a partir dos padrões EU (Miri MI45).
- **`git worktree remove` segue *junctions***: remova a *junction* de `.cache/play-client` antes (02/11/2026).

## Como registrar
Modelos prontos: `references/registro-e-aceite.md`.
- **`docs/history/session-log.md`** (acréscimo no fim, formato do `AGENTS.md`), uma entrada por passo verificado: relato, causa, mudança, verificação (comando e log
  em `.cache/verification/<etapa>/`), aplicação (hashes antes → depois), cliente (entrada do roteiro ou
  "cliente: não necessário") e próxima ação. Parada: motivo e ação retomável.
- **Roteiro de cliente por etapa** em `docs/client-tests/<etapa>-<classe>.md` (formato de
  `docs/playable-game-roadmap.md`; exemplos `C2-delia.md` e `C3-criacao.md`).
- **Documento da classe** `docs/future/<classe>-class-port.md`, na estrutura do da Miri (consolidado de conversões,
  mecânicas, armadilhas e pendências, mais o registro por passo) e com o inventário
  de arquivos do repositório, DLLs e *backups* com hashes, conteúdo da cópia de laboratório com antes/depois,
  e **o que veio do NA** (arquivo, origem, SHA-256 e a decisão do operador que autorizou).
- *Commit* local por passo, com código, testes, roteiro, linha do `MIGRATION.md` e entrada do histórico juntos; sem *push*.

## Checklist final de aceite
A classe só está portada quando o operador confirma no cliente, numa sessão pelo roteiro:
- [ ] **Seleção**: modelo, rosto, cabelo e roupa certos, sem decalques ou faixas (D17).
- [ ] **Cidade**: corpo, animações, estamina normal, sem queda ao entrar nem no emote (D8v, D19).
- [ ] **Arma**: uma arma, no lugar certo, ataque básico e combate de fase (D18).
- [ ] **Habilidades**: janela com as habilidades certas, posições distintas; ativas e SP pela barra (D13, D22).
- [ ] **Mecânicas próprias**: cada uma do escopo combinado, mais as aproximações declaradas (D22, D23).
- [ ] **Itens**: itens da classe sem fundo vermelho, dica sem "Cannot be equipped"; Fiona sem regressão (D22).
- [ ] **Malha**: conjuntos próprios e roupas de amostra sem a pele atravessando, sem peça inchada ou descolada (Miri MG4).
- [ ] **Teclas**: ENTER, t, `\`, TAB e q como nas classes antigas (Miri MI45).
- [ ] **Grade**: a classe aparece, o ator vai ao centro na pose de apresentação, vídeo certo (G1, G2).
- [ ] **Customização**: modelo da classe, abas, roupas de amostra, nome e "Check", `Back`, WASD travado,
      poses sem fila (G4, G6).
- [ ] **Criação até a cidade**: "Create" → estalagem → fala da Tieve → "Go to town" → cidade (G4).
- [ ] **Volta à seleção**: a personagem criada aparece certa e entra de novo; classe antiga pela grade (G3)
      e voltar da grade sem criar (G5) sem regressão.
