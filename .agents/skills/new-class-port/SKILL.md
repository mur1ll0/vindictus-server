---
name: new-class-port
description: >
  Procedimento para portar uma classe do cliente NA oficial (Miri, classe 11, e as seguintes) para o
  laboratório Vindictus 1.69, a partir da experiência da Delia (classe 10). Use quando o operador pedir
  para "trazer", "portar" ou "habilitar" uma classe que o 1.69 não tem, ou para continuar um porte já
  iniciado (conteúdo, heroes.db3, dados de criação, engine.dll, EXTRA_CLASSES, arma, mecânicas, criação
  pela grade, testes de cliente). Não use para variantes de classes existentes, itens ou modelos novos
  das 10 classes antigas, nem para trocar a base do projeto pelo cliente NA.
metadata:
  author: vindictus-server (escrita a partir do porte da Delia)
  version: "1.0.0"
  date: 2026-10-30
---

# Porte de uma classe nova do cliente NA para o 1.69

O exemplo completo é o porte da Delia: `docs/future/delia-class-port.md`. Esta skill não o repete; ela
diz em que ordem trabalhar, onde a Delia errou e o que muda para a próxima classe. Cite a seção do
documento da Delia sempre que precisar do endereço, do grupo ou do passo exato.

## Quando usar
- O operador pede uma classe que existe no NA e não no 1.69 (próxima: **Miri, classe 11**, arma `DRAKER`).
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
2. `docs/future/delia-class-port.md` inteiro (sobretudo 4.4, 4.5, 4.5.1, 7, 8 e 9).
3. `docs/future/new-classes.md` 8.13 (limites de 10 classes), 8.15 (mecânicas) e 9.1 (Miri).
4. `references/fontes-e-ferramentas.md` (clientes, cópia de laboratório, ferramentas).

Confirme com o operador, antes de qualquer patch: a classe, a decisão de cópia de assets do NA e o
escopo das mecânicas próprias (o que portar e o que aproximar). Sem isso, pare (regra 3 do `AGENTS.md`).

## Procedimento (ordem de trabalho)
Detalhes, comandos e o que muda da Delia para a Miri: `references/procedimento.md`.

0. **Medir** a classe no NA e no 1.69 (conteúdo, arquivos, scripts, chaves e comandos novos); nada é patch.
1. **Conteúdo** do cliente: arquivos da classe na cópia `.cache/play-client` (só com decisão do operador
   quando vierem do NA), com origem e SHA-256 registrados.
2. **`heroes.db3` e conteúdo do servidor**: linhas da classe no esquema 1.69 (`ClassRestriction` em
   máscara, bit `1 << índice`), tipo de arma, FeatureMatrix.
3. **Dados de criação** no servidor Rust: `class_additions.json`, exportação, `minimum-creation-assets.json`,
   `lab-set-class.ps1`, maestria em `equipped_skills.rs`.
4. **`engine.dll`**: *flag* de classe do item para o índice novo (o `bool[11]` acabou na Delia).
5. **`EXTRA_CLASSES`** e **revisão dos grupos** de `patch_class_routing.py`, mais `ACTIVE_CLASSES` em
   `patch_class_table.py`. Os grupos escritos para a Delia hoje valem para *toda* classe extra.
6. **Arma**: entidade 1.69 mais próxima (5 nomes por linha) ou porte da entidade do NA.
7. **Mecânicas próprias**: entidade NA x64 → *slots* 1.69, condições novas, efeitos e comandos ausentes.
8. **Criação pela grade**: filme, ator `npc_<classe>`, roupas de amostra; estalagem, byte de prólogo e
   "Go to town" já são genéricos.
9. **Aplicação** com o jogo fechado, a partir dos *backups*, na ordem `.hcls` → `engine.dll` → roteamento.
10. **Testes** focados e **roteiro de cliente** por etapa; teste no cliente na ordem seleção → cidade →
    arma → habilidades → mecânicas → itens → grade → customização → criação → seleção.

Antes de cada teste de cliente, procure `cmp reg, 9`/`0xa` antes de tabelas por classe nos caminhos que
a classe nova toca (`scripts/reverse/v169_class_uses.py` lista os pontos conhecidos).

## Armadilhas que a Delia ensinou
Evidência e passo de cada uma: `references/armadilhas.md`.
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
- **Grupos da Delia valem para toda classe extra** (`class_dispatch`, `cmp classe, 10; jl`): `GUARD`,
  `CLIENT_GUARD`, `CLASS_CONDITIONS`, `DELIA_EFFECTS`. Restrinja ou parametrize antes de pôr a Miri.

## Como registrar
Modelos prontos: `references/registro-e-aceite.md`.
- **`docs/history/session-log.md`** (acréscimo no fim, formato do `AGENTS.md`), uma entrada por passo verificado: relato, causa, mudança, verificação (comando e log
  em `.cache/verification/<etapa>/`), aplicação (hashes antes → depois), cliente (entrada do roteiro ou
  "cliente: não necessário") e próxima ação. Parada: motivo e ação retomável.
- **Roteiro de cliente por etapa** em `docs/client-tests/<etapa>-<classe>.md` (formato de
  `docs/playable-game-roadmap.md`; exemplos `C2-delia.md` e `C3-criacao.md`).
- **Documento da classe** `docs/future/<classe>-class-port.md`, na estrutura do da Delia, com o inventário
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
- [ ] **Grade**: a classe aparece, o ator vai ao centro na pose de apresentação, vídeo certo (G1, G2).
- [ ] **Customização**: modelo da classe, abas, roupas de amostra, nome e "Check", `Back`, WASD travado,
      poses sem fila (G4, G6).
- [ ] **Criação até a cidade**: "Create" → estalagem → fala da Tieve → "Go to town" → cidade (G4).
- [ ] **Volta à seleção**: a personagem criada aparece certa e entra de novo; classe antiga pela grade (G3)
      e voltar da grade sem criar (G5) sem regressão.
