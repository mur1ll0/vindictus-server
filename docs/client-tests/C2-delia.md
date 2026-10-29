# Roteiro de testes no cliente — C2 Delia sem patch (classe 10)

Caminho C, etapa C2 (`docs/future/new-classes.md`, seção 8.5). O objetivo é **medir**, não fazer a Delia funcionar: com o cliente 1.69 sem nenhum patch, uma personagem com classe 10 mostra qual camada falha primeiro (tabela de classes, entidade, modelo, arma, estados de ação). Uma queda do cliente é um resultado esperado e útil.

## Preparação
1. Runtime com a liberação da classe 10 no servidor: depois do commit deste teste, `COMPOSE_PARALLEL_LIMIT=1 docker compose build` e `docker compose up -d --wait`; `docker compose ps` mostra `game` e `server` saudáveis. A mudança deixa a lista de personagens e o login aceitarem a classe 10; a criação continua limitada às 10 classes do cliente.
2. Cliente: abrir pelo `.cache/play-client/start-lab.bat`, com a conta `lab169_a`. O `client.dll` da cópia de laboratório não recebe nenhum patch novo neste teste.
3. Relatórios de queda, se houver: `Documents\Vindictus EU GM\Report Error` (anote o nome do arquivo mais novo e o horário).
4. Os comandos abaixo rodam no PowerShell 7, na pasta do projeto, com a personagem **fora do jogo** (cliente fechado ou na seleção de personagem).

## Ordem de gameplay (uma sessão)
1. **D0:** criar a personagem de teste (Fiona) e entrar uma vez na cidade.
2. **D1:** dar a espada e o inner da Delia, ainda como Fiona.
3. **D2:** trocar a classe para 10 e abrir a seleção de personagem.
4. **D3:** entrar na cidade com a classe 10 (só se a D2 não derrubar o cliente).
5. **D4:** equipar a espada e o inner da Delia e entrar numa missão curta (só se a D3 chegar à cidade).
6. **D5:** desfazer a troca e conferir que a personagem volta a ser uma Fiona normal.

## Como relatar
Para cada linha marcada: ID da entrada e número da linha (ex.: `D2-2`), o que apareceu na tela (texto, captura se possível), se o cliente fechou ou travou, o horário aproximado e o nome do relatório de queda, se houver. Na primeira falha, pare a entrada, faça a D5 e relate; não é preciso tentar as entradas seguintes.

### D0 — Personagem de teste
Pré-condição: preparação feita.
- [ ] Na seleção, criar uma **Fiona** chamada `DeliaLab` (qualquer aparência) → criada e listada.
- [ ] Entrar na cidade com ela e sair para a seleção → entra e sai normalmente.
- [ ] Fechar o cliente.

### D1 — Itens da Delia (ainda como Fiona)
Pré-condição: `DeliaLab` fora do jogo. O `cid` aparece no painel, na ficha da personagem, ou na saída da D2; se ainda não souber, faça a D2 primeiro, desfaça com a D5 e volte aqui.
- [ ] `./scripts/dev/lab-give-item.ps1 -ItemClass bastardsword_chieftain_restoration -Count 1 -Cid <cid>` → o comando termina sem erro.
- [ ] `./scripts/dev/lab-give-item.ps1 -ItemClass delia_china_inner -Count 1 -Cid <cid>` → o comando termina sem erro.
(Alternativa: dar os dois itens pelo painel, em Personagens → `DeliaLab` → Dar item.)

### D2 — Classe 10 na seleção de personagem
Pré-condição: D1 feita; cliente fechado.
- [ ] `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 10` → imprime `cid=<número> class 1 -> 10` e o comando para desfazer.
- [ ] Abrir o cliente e entrar até a seleção de personagem → anotar: a lista aparece? `DeliaLab` aparece? Com que nome de classe, ícone e modelo (Fiona, outro, invisível, pose T)? O cliente fecha?
- [ ] Clicar em `DeliaLab` sem entrar → o modelo muda? Algum texto vazio ou `#GameUI_…`?

### D3 — Cidade com a classe 10
Pré-condição: D2 sem queda.
- [ ] Entrar com `DeliaLab` → anotar: tela de carregamento, chegada na cidade ou queda (e em que ponto).
- [ ] Na cidade: modelo do corpo, cabeça e cabelo; andar, correr, pular; abrir o inventário e a janela de personagem (nome de classe, atributos, skills).

### D4 — Equipamento e missão curta
Pré-condição: D3 na cidade.
- [ ] Equipar `delia_china_inner` e a espada `bastardsword_chieftain_restoration` → aceita ou recusa (texto da mensagem)? O modelo muda?
- [ ] Entrar na primeira missão disponível → carrega? Atacar (botão esquerdo e direito), esquivar, usar uma skill → anotar a animação (da Delia, da Fiona, parada) e se o cliente fecha.
- [ ] Sair da missão e voltar à cidade.

### D5 — Desfazer
Pré-condição: personagem fora do jogo (feche o cliente se ele travou). Se a D4 equipou os itens da Delia, desequipe-os no jogo antes de sair; se não der (queda), relate e não rode o comando, que a limpeza é feita do lado do servidor.
- [ ] `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 1` → `class 10 -> 1`.
- [ ] Abrir o cliente: `DeliaLab` aparece como Fiona e entra na cidade. Os itens da Delia ficam no inventário (não equipáveis pela Fiona).

## Resultado da sessão de 27/10/2026 (operador, 20:55–21:01 local; servidor 23:55–00:01 UTC)
- **D0/D1:** Fiona `DeliaLab` criada pelo operador; itens dados e classe trocada por mim (`MIGRATION.md`). Ela não tinha entrado na cidade antes da troca.
- **D2 (seleção):** `DeliaLab` aparece com o retrato certo da Delia, mas o modelo 3D fica em pose T, sem rosto, e com a textura do inner errada. O cliente não caiu. Leitura: o retrato vem do conteúdo (classe 10 existe nos dados) e o modelo vem do código de classe do cliente, que não conhece a classe 10 (seção 8.2).
- **Troca de slot:** o operador moveu a ordem dos slots antes de entrar; o servidor recebeu a `SelectOrdersChangeMessage` (categoria 489) sem tratamento, não gravou a ordem, e a entrada pelo slot levou à personagem `Teste` (Hagie). Defeito do servidor, independente da Delia.
- **D3 (entrar):** "You have been disconnected from server". Causa no servidor, antes de o cliente receber a entrada: a seleção montava o cache de skills pelo *initializer* da classe (`initializer_for(10)`), que não existe, e respondia `Conflict` (log `laboratory frontend connection closed`, 00:00:47 UTC). Corrigido: o *initializer* passa a vir do marcador de inicialização da personagem (o da Fiona que a criou), na seleção e nos atributos de nível 1.
- **Pendente:** repetir a D3 com a correção; D4 e D5.

## Resultado da segunda sessão (27/10/2026, 21:48–21:49 local)
- **D3 (entrar), com a correção da seleção:** o servidor aceitou a entrada; o cliente carregou a cidade (`t01`), conectou `DeliaLab` e caiu logo após `joined team "Paladin"`, ao criar a entidade do jogador: `EXCEPTION_ACCESS_VIOLATION` em `server.dll` (`0x408b4700`, um *getter* `mov eax,[ecx+0x1c]` chamado com objeto inválido). O `server.dll` da cópia é idêntico ao original (`d7727e18…`).
- **Leitura:** é a camada nativa medida na seção 8.2 de `docs/future/new-classes.md`. A simulação local da cidade não tem tabela nem entidade para a classe 10 (`CHeroesPlayerDelia` não existe; tabela de 10 classes em `0x40d0cdf8`), e o objeto da classe não é criado. Sem patch do cliente, a classe 10 para aqui.
- **Ordem dos patches que o teste confirma:** (1) tabela de classes e entidade do jogador no `server.dll` e no `client.dll`; (2) modelo da seleção (pose T, sem rosto); (3) arma e condições de estado de ação.
- D4 não executada (sem cidade). D5 (desfazer) disponível: `scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 1`.

### D6 — Criação S3 (grade de classes) ligada pela FeatureMatrix
Pré-condição: `game` recompilado com o commit deste passo (o servidor envia `S3_CharacterCreation = 3.09.00`). Não entre com a `DeliaLab` (voltou a Fiona; não troque a classe de novo).
- [ ] Na seleção de personagem, clicar em criar personagem → abre a grade de classes (`NewCharacterDialogS3`) no lugar da barra? Anotar quantas classes aparecem, se a Delia está entre elas e se o modelo e a prévia aparecem ao clicar em cada uma.
- [ ] Clicar na Delia → o que aparece (modelo, vídeo, descrição, arma)?
- [ ] Criar uma personagem de uma classe já suportada (por exemplo, Fiona `GradeLab`) → criada e entra na cidade como antes.
- [ ] Tentar criar uma Delia → esperado: recusa do servidor (a criação aceita só 0–9). Anotar a mensagem; se o cliente cair, anotar o horário e o relatório.

### D7 — Não regressão com as tabelas de classe estendidas (`.hcls`)
Pré-condição: `client.dll` e `server.dll` da cópia de laboratório com o patch `scripts/client/patch_class_table.py` aplicado (hashes `a9591d3a…` e `af43431e…`; o `start-lab.bat` não troca esses arquivos). A `DeliaLab` é Fiona; não troque a classe.
- [ ] Abrir pelo `start-lab.bat`, entrar até a seleção → a lista e os modelos aparecem como antes.
- [ ] Criar uma personagem de qualquer classe (barra V4) → criada; entrar na cidade com ela e com a Murael.
- [ ] Entrar numa missão curta e voltar à cidade → sem queda.
- [ ] Se o cliente cair, anotar o horário e o relatório de `Report Error`. Para desfazer o patch: copiar `bin/client.dll.314272ec54054ed0.pre-hcls.bak` sobre `bin/client.dll` e `bin/server.dll.d7727e182f6cdec1.pre-hcls.bak` sobre `bin/server.dll`.
- **Resultado D7 (operador, 28/10/2026):** com os DLLs `.hcls`, a criação abre a barra V4 de 10 classes, sem a Delia (esperado: a grade S3 foi desfeita e a barra é fixa em 10), e selecionar uma classe carrega o tutorial normalmente. Sem regressão observada; a missão curta não foi relatada. O patch fica aplicado.

### D8 — Classe 10 roteada para os arquivos da Delia (patch `.hrte`, sem a entidade própria)
Pré-condição: `client.dll` e `server.dll` da cópia de laboratório com `scripts/client/patch_class_routing.py` aplicado sobre o `.hcls` (hashes `0e1b1800…` e `eda9ac59…`). O servidor local cria `player_delia` pelo nome; como a fábrica não existe no 1.69, ele usa a entidade genérica `player` (`0x403712f7`), sem a guarda nem a arma próprias da Delia (E1–E5). Runtime com `docker compose up -d --wait` saudável. Use a `DeliaLab` (Fiona) fora do jogo.
- [ ] `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 10` → imprime `class 1 -> 10`.
- [ ] Abrir pelo `start-lab.bat` e ir até a seleção → a `DeliaLab` aparece? Anotar nome da classe, modelo e se o cliente fecha.
- [ ] Entrar na cidade com a `DeliaLab` → esperado: a cidade carrega **sem** a queda do D3 (`0x408b4700` em `server.dll`). Anotar o modelo (corpo da Delia, da Fiona ou pose T), as animações ao andar e correr e se há texto `#GameUI_…` vazio.
- [ ] Na cidade: abrir inventário e habilidades, andar, correr, usar um emote → anotar o que falha.
- [ ] Voltar à seleção e sair → sem queda.
- [ ] Desfazer: `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 1`.
- [ ] Se o cliente cair, anotar o horário e o relatório de `Report Error`. Para desfazer o patch: copiar `bin/client.dll.a9591d3a44a2b538.pre-routing.bak` sobre `bin/client.dll` e `bin/server.dll.af43431e78666783.pre-routing.bak` sobre `bin/server.dll` (volta ao `.hcls`, que o D7 aceitou).
Limites conhecidos: a barra V4 não cria a Delia (a classe 10 é aceita pelo servidor, mas só a grade ou um botão novo a envia); a Delia continua com os itens da Fiona; sem guarda, ataque forte, *hold skill* e arma `BASTARDSWORD` própria (E2–E5); os demais `switch` com limite 9 (eventos, HUD, figura) caem no caso padrão.

Resultado (28/10/2026): **falhou.** Seleção: modelo da Fiona sem rosto, em pose T. Entrada: `t01` carregou e o cliente caiu cerca de 2 s depois (`client.dll` `0x387da6b0`, `this` nulo). Causa: a preparação do jogador local (`0x387033b0`) recusa classes ≥ 10 em `0x387033d7` e não cria o objeto de comandos `+0x437c` que o movimento lê (`docs/future/new-classes.md` 8.14). Patch revertido (`client.dll` `a9591d3a…`, `server.dll` `af43431e…`); `DeliaLab` na classe 1.

### D8b — Repetir o D8 com a preparação do jogador local liberada
Pré-condição: o operador autoriza reaplicar o patch. `python -s -E scripts/client/patch_class_routing.py --apply` sobre a cópia `.hcls` (o patch agora inclui `GATES`, `0x387033d7` com limite 28); `docker compose up -d --wait` saudável; `DeliaLab` fora do jogo. Aplicado em 28/10/2026: `client.dll` `a9591d3a…` → `560f5e27…`, `server.dll` `af43431e…` → `eda9ac59…`.
- [ ] `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 10` → `class 1 -> 10`.
- [ ] Seleção: anotar o modelo (a Fiona sem rosto em pose T é esperada até o E1).
- [ ] Entrar na cidade → esperado: sem a queda em `0x387da6b0`. Andar, correr, abrir inventário e habilidades, usar um emote; anotar o modelo e as animações.
- [ ] Voltar à seleção e sair → sem queda.
- [ ] Desfazer: `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 1`.
- [ ] Se cair: anotar o horário e o endereço do `Report Error`. Para reverter, copiar os `*.pre-routing.bak` sobre `bin/client.dll` e `bin/server.dll`.

Execução de 28/10/2026 12:21: **não testou o D8b.** Os DLLs estavam sem o patch de roteamento (`a9591d3a…`/`af43431e…`, o `--apply` não foi rodado), então a classe 10 caiu de novo no caminho do pet: queda do D3 em `server.dll` `0x408b4700` (retorno `0x40804b5f`, `switch` de expressões), antes de entrar no canal. Servidor sem erro (`.cache/verification/d8b-game.log`). `DeliaLab` na classe 1; DLLs continuam revertidos. Sem o passo do `--apply`, trocar a classe para 10 sempre reproduz o D3.

Resultado do D8b (28/10/2026, cliente, patch com `GATES`): **avançou, com duas quedas.**
- A cidade carrega e a personagem anda, mas **invisível** (nenhum modelo 3D); a `Bastard Sword` e a *inner* aparecem como "Cannot be equipped"; nenhuma habilidade na janela.
- 12:54: `F7` (emote) → queda em `client.dll` `0x38a5df98` (leitura de `0x20`).
- 12:57: entrar no Mercenary Outpost → queda em `server.dll` `0x40994727` (leitura de `0x20`).
- Causa das duas (minidumps em `.cache/d8/d8b-*.mdmp`): `SkillActionStateMatchTable::GetSkillNamesByActionStateName` (cliente `0x3885fb60`, servidor `0x407ccb8b`) lê o mapa da classe em `this + classe*0x1c + 0x20`. A tabela tem 10 mapas (`+0x20` a `+0x11c`, construtor `0x38861640`); a classe 10 lê `+0x138`, que é outro campo. O arquivo `scripts/skill/skill_actionstate_match_table.txt` do 1.69 já tem 13 linhas `skill_delia_*`.
- Servidor Rust sem aviso nem erro (`.cache/verification/d8b-run-game.log`).

### D8c — Classe 10 com o 11º mapa da `SkillActionStateMatchTable`
Pré-condição: patch de roteamento com `MATCH` aplicado sobre a `.hcls` (28/10/2026, 2ª versão: `client.dll` `a9591d3a…` → `75f08dcc…`, `server.dll` `af43431e…` → `d2f93765…`); `docker compose up -d --wait` saudável; `DeliaLab` fora do jogo. Abrir pelo `start-lab.bat`.
- [ ] `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 10` → `class 1 -> 10`.
- [ ] Entrar na cidade → esperado como no D8b (personagem invisível, itens "Cannot be equipped" e janela de habilidades vazia continuam abertos).
- [ ] `F7` (emote) → esperado: **sem** a queda em `client.dll` `0x38a5df98`.
- [ ] Entrar no Mercenary Outpost → esperado: **sem** a queda em `server.dll` `0x40994727`. Anotar se a área carrega.
- [ ] Voltar à seleção e sair → sem queda.
- [ ] Desfazer: `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 1`.
- [ ] Se cair: anotar horário e endereço do `Report Error`. Reverter: copiar os `*.pre-routing.bak` sobre `bin/client.dll` e `bin/server.dll`.

Execução de 28/10/2026 13:11 (1ª versão, `75874fe4…`/`8265556f…`): **falhou antes do menu.** Queda em `server.dll` `0x407cda68` ao montar a tabela (`MatchingActionStateSkill` no construtor). O laço lê o mapa também por `[esp+0x10]` depois de inserir (`0x407cda5a`; cliente `0x38860b4a`), e o *stub* só trocava `ebp`. Corrigido na 2ª versão (o *stub* da classe 10 aponta `[esp+0x10]` para o mapa da Delia + 4).

Execução de 28/10/2026 (2ª versão, `75f08dcc…`/`d2f93765…`): **passou nas quedas.** A cidade carrega; `F7` não derruba o cliente, mas a personagem trava (sem animação); o Mercenary Outpost carrega. Continuam: personagem invisível, `Bastard Sword`/*inner* "Cannot be equipped", nenhuma habilidade na janela.

### D8d — Modelos de animação da Delia no servidor local
Pré-condição: patch com as tabelas de animação (`server.dll` `af43431e…` → `f83dd892…`; `client.dll` `75f08dcc…`, sem mudança); `docker compose up -d --wait` saudável; `DeliaLab` fora do jogo; abrir pelo `start-lab.bat`.
- [ ] `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 10`.
- [ ] Entrar na cidade → anotar se a personagem aparece (corpo da Delia, pose T ou invisível) e se anda com animação.
- [ ] `F7` (emote) → esperado: a animação roda e a personagem não trava. Anotar o emote.
- [ ] Entrar no Mercenary Outpost (animação de batalha) → anotar se anda/ataca com animação.
- [ ] Desfazer: `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 1`.
- [ ] Se cair: horário e endereço do `Report Error`. Reverter: copiar os `*.pre-routing.bak`.

Resultado do D8d (28/10/2026): **sem mudança visível.** A `DeliaLab` continua invisível na cidade e trava no `F7`.

### D8e — Itens com a classe 10 no `engine.dll`
Pré-condição: `scripts/client/patch_engine_class_flags.py --client-copy .cache/play-client --apply` (`engine.dll` `a3add974…` → `44281475…`, cópia `engine.dll.a3add9742f7fa03c.pre-classflags.bak`); `client.dll` `75f08dcc…` e `server.dll` `f83dd892…` como no D8d; `docker compose up -d --wait` saudável; abrir pelo `start-lab.bat`.
- [ ] Antes: entrar com uma personagem de outra classe e equipar/desequipar um item → sem regressão (o *setter* foi reescrito para as classes 8 e 9 também).
- [ ] `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 10`.
- [ ] Inventário: a `Bastard Sword` e a *inner* da Delia **não** mostram mais "Cannot be equipped". Equipar as duas.
- [ ] Anotar se a personagem aparece na cidade depois de equipar a *inner* (o corpo vem da *inner armor*).
- [ ] `F7` → anotar se a animação roda.
- [ ] Desfazer: `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 1`.
- [ ] Se cair: horário e endereço do `Report Error`. Reverter o `engine.dll`: copiar `engine.dll.a3add9742f7fa03c.pre-classflags.bak` sobre `bin/engine.dll`.

Resultado do D8e (28/10/2026): **não regressão ok** (Hagie desequipou e equipou um item). `DeliaLab` continua invisível e a `Bastard Sword` e a *inner* continuam "Cannot be equipped". Os DLLs carregados eram os certos (`bin/engine.dll` `44281475…`; o minidump lista `bin\engine.dll`, `bin\client.dll` e `bin\server.dll`; o `start-lab.bat` não troca DLLs). A recusa estava em outro ponto: o `client.dll` lê o vetor por `switch` de 0 a 9.

### D8f — Acessores de classe dos itens no `client.dll` e no `server.dll`
Pré-condição: patch de roteamento com `FLAG_CHECKS` (`client.dll` `a9591d3a…` → `d1a1571c…`, `server.dll` `af43431e…` → `d094ee11…`) e o `engine.dll` do D8e (`44281475…`); `docker compose up -d --wait` saudável; abrir pelo `start-lab.bat` (nada mais a rodar).
- [ ] Não regressão: entrar com a Hagie, desequipar e equipar um item.
- [ ] `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 10`.
- [ ] Inventário: `Bastard Sword` e *inner* da Delia sem "Cannot be equipped"; equipar as duas.
- [ ] Anotar se a personagem aparece depois de equipar a *inner* e se o `F7` anima.
- [ ] Desfazer: `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 1`.
- [ ] Se cair: horário e endereço do `Report Error`. Reverter: copiar os `*.pre-routing.bak` e o `engine.dll.a3add9742f7fa03c.pre-classflags.bak`.

Resultado do D8f (28/10/2026): Hagie sem regressão. `DeliaLab` continua invisível; a `Bastard Sword` ainda mostra "Cannot be equipped", mas o botão direito a equipa (com a personagem "bugada"); a calça também equipa, e nada aparece.

### D8g — Pré-carga dos modelos de animação da Delia e dica do item
Pré-condição: patch de roteamento com `PRECACHE` e `TOOLTIPS` (`client.dll` `a9591d3a…` → `b75ddeb2…`, `server.dll` `af43431e…` → `a25f68f1…`) e `engine.dll` `44281475…`; `docker compose up -d --wait` saudável; abrir pelo `start-lab.bat` (nada mais a rodar).
- [ ] Não regressão: Hagie desequipa e equipa um item; a dica de um item comum a todas as classes antigas continua sem lista de classes.
- [ ] `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 10`.
- [ ] Entrar na cidade → esperado: **a personagem aparece** (esqueleto e animação da Delia) e anda com animação.
- [ ] Dica da `Bastard Sword` e da *inner* da Delia → esperado: "Only for Delia" (não mais "Cannot be equipped").
- [ ] Equipar espada, armadura e *inner* → esperado: aparecem no corpo.
- [ ] `F7` → esperado: a animação roda e a personagem não trava.
- [ ] Desfazer: `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 1`.
- [ ] Se cair: horário e endereço do `Report Error`. Reverter: copiar os `*.pre-routing.bak` e o `engine.dll.a3add9742f7fa03c.pre-classflags.bak`.

Resultado do D8g (28/10/2026, 14:23, DLLs certos: `client.dll` gravado às 14:20, `server.dll` às 14:21): **sem mudança.** Personagem invisível; dica "Cannot be equipped"; os itens são aceitos, mas nada aparece no corpo; só a arma aparece (duas espadas, paradas como em pose T).

### D8h — Coleta do console do motor
Pré-condição: `start-lab.bat` reinstalado com `-condebug +developer 1` (`scripts/client/install_lab_launcher.py`); DLLs do D8g, sem mudança.
- [ ] `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 10`; abrir pelo `start-lab.bat`.
- [ ] Entrar na cidade com a `DeliaLab`, abrir o inventário, passar o mouse na `Bastard Sword`, equipar a *inner* e apertar `F7`.
- [ ] Sair do jogo normalmente.
- [ ] Avisar o horário; o `console.log` fica na cópia (`.cache/play-client`, pasta do jogo ou `bip/`).
- [ ] Desfazer: `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 1`.

Resultado do D8h (28/10/2026, 14:31): sem mudança no jogo; **o `console.log` não foi gerado** (o `vindictus.exe` não repassa `-condebug` ao jogo).

### D8i — Coluna de roupa da Delia (`player_costume.txt`) e ferramentas de inspeção
Pré-condição: patch com `COSTUME` (`client.dll` `a9591d3a…` → `97cacae0…`, `server.dll` `af43431e…` → `8458f959…`, relatório `bin/class-routing.json`); `engine.dll` `44281475…`; `start-lab.bat` reinstalado (sem `-condebug`; cria `cfg/config_pendingkey_concmd.cfg` com `developer 1` e `con_logfile lab_console.log`); `docker compose up -d --wait` saudável.
- [ ] Não regressão: Hagie entra na cidade com a roupa de sempre, desequipa e equipa um item.
- [ ] `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 10`; abrir pelo `start-lab.bat`.
- [ ] Entrar na cidade → esperado: **corpo da Delia visível** (base `delia_base.mdl`, rosto e animação de cidade dela).
- [ ] Equipar espada, armadura e *inner* → anotar o que aparece no corpo; anotar a dica dos itens.
- [ ] `F7` → anotar se anima.
- [ ] **Com o jogo aberto, na cidade**, rodar e copiar a saída (só leitura, não altera o jogo):
  - `python -s -E scripts/dev/lab_memory.py modules`
  - `python -s -E scripts/dev/lab_memory.py costume 10`
  - `python -s -E scripts/dev/lab_memory.py costume 9`
- [ ] Sair do jogo; anotar se apareceu `lab_console.log` na pasta `.cache/play-client` (ou `bip/`).
- [ ] Desfazer: `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 1`.
- [ ] Se cair: horário e endereço do `Report Error`. Reverter: copiar os `*.pre-routing.bak` e o `engine.dll.a3add9742f7fa03c.pre-classflags.bak`.

Resultado do D8i (28/10/2026): **avanço na seleção.** A seleção mostra o modelo 3D da Delia com a animação dela sentada; a *inner* padrão tem defeito nas pernas (falta de malha ou textura) e muda ao trocar a *inner*, com o mesmo defeito. Na cidade a personagem continua invisível e a arma continua "Cannot be equipped". O `lab_memory.py` não anexou: o `Vindictus.exe` roda como administrador e o terminal não (erro 5 do Windows); a ferramenta agora diz isso.

### D8j — Leitura de memória com o jogo aberto (terminal de administrador)
Pré-condição: DLLs do D8i, sem mudança.
- [ ] Abrir pelo `start-lab.bat`, entrar na cidade com a `DeliaLab` (classe 10).
- [ ] Num **terminal aberto como administrador**, na pasta do projeto, rodar e copiar a saída:
  - `python -s -E scripts/dev/lab_memory.py modules`
  - `python -s -E scripts/dev/lab_memory.py costume 10`
  - `python -s -E scripts/dev/lab_memory.py costume 9`
- [ ] Anotar se apareceu `lab_console.log` em `.cache/play-client` ou `bip/` depois de sair.

Resultado do D8j (28/10/2026, terminal de administrador): os três módulos nos endereços preferidos (`client.dll` `0x38000000`, `server.dll` `0x40000000`, `engine.dll` `0x42000000`). A coluna da Delia foi carregada igual no cliente e no servidor, no mesmo padrão da Hagie: base 10 (Hagie 9), rosto 100 (90), animação 100/101 e cidade 102 (90/91 e 92); as outras partes -1 nas duas classes. O carregamento da roupa está certo; o que falta na cidade não é dado de roupa.

### D8k — Diagnóstico: entidade da Fiona para a classe sem entidade (temporário)
Escolha do operador (28/10/2026): teste de diagnóstico, não o desenho da entidade da Delia (seção 8.9). Pré-condição: `patch_class_routing.py --apply --fallback-entity player_fiona` (`client.dll` `97cacae0…`, sem mudança; `server.dll` `af43431e…` → `45c7a082…`; `0x40371314` empurra `"player_fiona"` em vez de `"player"`); `engine.dll` `44281475…`; abrir pelo `start-lab.bat`.
- [ ] Não regressão: Hagie entra na cidade normalmente (tem entidade própria, não passa pelo *fallback*).
- [ ] `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 10`.
- [ ] Entrar na cidade → anotar: o corpo da Delia aparece? Com animação? As pernas da *inner* continuam com defeito?
- [ ] Equipar espada, armadura e *inner* → anotar o que aparece no corpo.
- [ ] `F7` → anotar se anima.
- [ ] Desfazer a classe: `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 1`.
- [ ] Ao terminar o diagnóstico, voltar ao patch sem a opção: copiar os `*.pre-routing.bak` sobre `bin/client.dll` e `bin/server.dll` e rodar `python -s -E scripts/client/patch_class_routing.py --client-copy .cache/play-client --apply`.
- Leitura do resultado: se a Delia aparece, o que falta é a entidade da classe (E1, com os *slots* que diferem da base); se continua invisível, a causa está fora da entidade.

Resultado do D8k (28/10/2026): **sem mudança.** Seleção: pernas da *inner* brancas, muito iluminadas (malha ou textura errada). Cidade: invisível, mesmos problemas. A entidade da classe não é a causa.
- Memória (leitor elevado `lab_memory.py serve`, aprovado pelo operador): o jogador local do cliente está em `[[0x38c8f3f4]+0x14]` (`C_HeroesPlayerFiona`, classe 10) e o do servidor em `[[0x40cb3740]+0x14]` (`CHeroesPlayerFiona`, classe 10). Cópias salvas em `.cache/lab-memory/delia_town_client.bin` e `delia_town_server.bin` para comparar com uma Fiona visível.
- Aparência guardada da `DeliaLab` (`game_costumes`): rosto 10 e cabelo 2001 (os da Fiona com que foi criada), *inner* 0.

### D8l — Comparação de memória: a mesma personagem como Fiona
Pré-condição: jogo aberto e o leitor elevado (`lab_memory.py serve`) rodando; DLLs do D8k.
- [ ] Voltar à seleção de personagens (sem fechar o jogo).
- [ ] Avisar; eu rodo `lab-set-class.ps1 -Name DeliaLab -Class 1`.
- [ ] Entrar na cidade com a `DeliaLab` (agora Fiona) e avisar; eu salvo as mesmas cópias e comparo.

Resultado do D8l (28/10/2026): a mesma `DeliaLab` como Fiona aparece corretamente na cidade. Comparação de memória (`lab_memory.py diff`), mesma classe de entidade (`C_HeroesPlayerFiona`):
- índice de modelo da entidade (cliente `+0x84`, 16 bits): Fiona 13, Delia **-1** (sem modelo); os `-1` em `+0xecc..+0xed4` (Fiona 0, 17, 18) acompanham;
- os oito códigos de roupa equipada (servidor `+0x3418`, cliente `+0x488c`, `parte*100000 + número`, por exemplo `100130`, `700353`): Fiona com as oito peças, Delia todas `-1`;
- a coluna de roupa já estava certa (D8j). Hipótese: o corpo-base `delia_base.mdl` e os rostos não estão na tabela de pré-carga de modelos do servidor; a seleção é só do cliente e não precisa dela.

### D8m — Pré-carga do corpo-base e dos rostos da Delia
Pré-condição: **jogo fechado** antes de aplicar (os DLLs carregados não podem ser trocados). Patch de roteamento **sem** `--fallback-entity` (o diagnóstico da Fiona sai), com `PRECACHE` incluindo `delia_base.mdl` e `pc_delia_face01..03.mdl`; `engine.dll` `44281475…`; abrir pelo `start-lab.bat`.
- [ ] Não regressão: Hagie entra na cidade normalmente.
- [ ] `./scripts/dev/lab-set-class.ps1 -Name DeliaLab -Class 10`.
- [ ] Entrar na cidade → anotar se o corpo aparece.
- [ ] Com o jogo aberto, eu rodo `lab_memory.py` (o leitor elevado precisa ser iniciado de novo; aprovar o UAC) e confiro o índice de modelo da entidade.

Resultado do D8m (28/10/2026): **sem mudança** (invisível na cidade; coxa da *inner* com defeito na seleção). Memória:
- os modelos da Delia foram pré-carregados (vários `pc_delia_face01/02.mdl` no *heap*), então a pré-carga não era a falta;
- os campos do cliente pelo `RecvProp`: `m_nModelIndex` em `+0xb0` (16 bits; o `+0x84` usado antes era outro campo), `m_fEffects` `+0x7c`, `m_nRenderMode` `+0x80`, `m_nRenderFX` `+0x5c`, `m_clrRender` `+0x60`. A Delia tem modelo válido (290 na cópia do D8k, 294 agora; a Fiona 491) e os mesmos valores de desenho que a Fiona (efeitos 0, modo 0, cor branca opaca);
- o que continua diferente: os oito códigos de roupa do servidor (`+0x3418`, `parte*100000 + índice`) ficam `-1` para a Delia, inclusive cabelo e *inner*, que a Fiona resolve.

### D8n — Arquivo de eventos da animação da Delia no servidor local
Causa achada (rastreio estático, seção 8.14): o método do servidor que grava o arquivo de eventos da animação no objeto de roupa (`0x4081bae0`) começa com `cmp esi, 9; ja` e lê a coluna do gerenciador por endereço fixo (`classe*16 + 0x40f64698`, ou `0x40f64738` em *ExtremeMode*). A classe 10 ficava sem arquivo de eventos; o caminho em rede (`+0x3f0`, cliente `+0x410`) ficava vazio e o corpo da cidade não era montado.
Pré-condição: **jogo fechado** antes de aplicar. Patch de roteamento com `EVENT_FILE` (a classe 10 lê os caminhos da coluna da sombra); `engine.dll` `44281475…`; abrir pelo `start-lab.bat`.
- [ ] Não regressão: Hagie entra na cidade normalmente.
- [ ] `DeliaLab` na classe 10 (já está); entrar na cidade → anotar se o corpo aparece e se anda com animação.
- [ ] `F7` → anotar se o emote roda.
- [ ] Equipar espada, armadura e *inner* → anotar o que aparece.
- [ ] Com o jogo aberto, eu confiro na memória o caminho do arquivo de eventos (precisa aprovar o UAC do leitor).

Resultado do D8n (28/10/2026): **avanço parcial.** A *inner* da Delia agora mostra "Only for Delia" na dica; a `Bastard Sword` continua "Cannot be equipped" (as armas usam outra verificação). Tudo equipa, mas nada aparece; a personagem continua invisível. Memória: o caminho do arquivo de eventos agora está certo no servidor e no cliente (`…\delia\pc_delia_motion_ex.txt`) e as 12 partes também; os oito códigos resolvidos (`+0x3418`) continuam `-1`, inclusive para *inner* 10 e cabelo 2001, que a Fiona resolve.

### D8v — Corpo da Delia na cidade (montagem da roupa liberada no servidor local)
Pré-condição: patch de roteamento com `COSTUME_GATE` (`server.dll` `a444b183…`, `client.dll` `09bbd443…`, `engine.dll` `44281475…`); `DeliaLab` na classe 10.
Resultado (28/10/2026): **passou no corpo.** O modelo da Delia aparece na cidade, sem defeito no corpo, e as animações funcionam. Pendentes:
- seleção de personagens: defeito da barriga até a coxa (provável `csf/bodyshape_delia_default.fig`, aplicado pela entidade própria da classe; E1);
- a `Bastard Sword` aparece como duas espadas em vez de uma espada grande nas costas (o 1.69 não tem o tipo `BASTARDSWORD`; E2);
- a janela de habilidades fica vazia.


### D9 — E1: figura do corpo e constantes da Delia (tabela de classes extras)
Mudança: `patch_class_routing.py` passou a ler a tabela `EXTRA_CLASSES` (Delia = 10; a Miri será mais uma linha). Novo `ENTITY_SLOTS`: na vtable de `C_HeroesPlayer` (a entidade que a Delia usa), o *slot* 264 devolve `csf/bodyshape_delia_default.fig` pela mesma trava da Hagie (`0x3870d1d0`) e os *slots* 177/178 devolvem 0,88/0,96 (valores do NA); as outras classes rodam o original.
Pré-condição: **jogo fechado** antes de aplicar (já aplicado: `client.dll` `a47c4aac…`, `server.dll` `e6f1b67b…`, `engine.dll` `44281475…`); `DeliaLab` na classe 10; abrir pelo `start-lab.bat`.
- [ ] Seleção de personagens: olhar a `DeliaLab` → anotar se o defeito da barriga até a coxa sumiu.
- [ ] Não regressão: Hagie na seleção e na cidade sem mudança.
- [ ] Entrar na cidade com a `DeliaLab` → corpo e animações como no D8v.
- [ ] Abrir a janela de habilidades (SP) → anotar se aparece alguma habilidade (o `CLASS_NAME` ainda não foi conferido no cliente).
Resultado esperado: seleção sem o defeito; cidade igual ao D8v.

Resultado do D9 (28/10/2026): Hagie sem regressão (seleção e cidade). `DeliaLab`: cidade sem defeito; **a seleção continua com o defeito da barriga à coxa**; janela de habilidades vazia; a criação não oferece a Delia (barra V4 de 10 classes).

### D10 — Roupa-base e habilidades iniciais da Delia na `DeliaLab`
Causa provável (dados, não DLL): a `DeliaLab` foi criada como Fiona e guardava a roupa-base da Fiona (*inner* 10, cabelo 2001, sobrancelha 0; a Delia, pelo `csf/create_delia.csf`, usa 1002, 6036, 8000) e as habilidades iniciais da Fiona (`skill_fiona_shield_mastery`, `skill_fiona_guard`), que a janela da classe 10 não mostra. A seleção monta o modelo pela roupa-base da lista de personagens; a cidade, pela montagem do servidor local.
Mudança: `lab-set-class.ps1` agora troca também as habilidades iniciais (pela `DefaultSkillInfo`; a da Delia, que o 1.69 não tem, é `skill_delia_bastard_mastery` + `skill_delia_guard`, escolha de laboratório) e a roupa-base que ainda está no padrão da classe antiga. Já aplicado na `DeliaLab` (classe 10; 2 habilidades e 3 partes trocadas). DLLs iguais ao D9.
Pré-condição: servidor no ar; abrir pelo `start-lab.bat`.
- [ ] Seleção: a `DeliaLab` → anotar se o defeito da barriga à coxa (pernas brancas da *inner*) sumiu e se cabelo/sobrancelha são os da Delia.
- [ ] Cidade com a `DeliaLab` → corpo e animações como antes.
- [ ] Janela de habilidades → anotar se aparecem `Bastard Sword Mastery` e `Guard` da Delia.
- [ ] (Opcional) com a `Bastard Sword` equipada, usar o *guard* → anotar o que acontece.
Resultado esperado: seleção sem defeito; duas habilidades da Delia na janela.

Resultado do D10 (28/10/2026): o cabelo da `DeliaLab` mudou para o da Delia na seleção, mas o defeito da barriga e da perna continua. Ao entrar no jogo: **desconectado**. Log do `game` (`.cache/verification/d10-compose.log`): `laboratory frontend connection closed` logo depois da seleção; a falha é do `minimum_selection::select`, que exige que as primeiras linhas de habilidade sejam as do inicializador que criou a personagem (a Fiona). Renomear essas linhas para as da Delia fez a carga recusar a personagem.

### D11 — Entrada com as habilidades da Delia acrescentadas; seleção sem forma de corpo da Fiona
Mudanças:
- `lab-set-class.ps1` não renomeia mais as habilidades: as linhas da Fiona voltaram, e as iniciais da Delia (`skill_delia_bastard_mastery`, `skill_delia_guard`, rank 1) foram **acrescentadas** como habilidades aprendidas (estão no perfil de habilidades do servidor). A roupa-base continua a da Delia.
- Experimento da seleção: as 12 linhas de forma do corpo (tipo 3, chaves da Fiona 0, 15–20, 22–25, 40) foram retiradas da `DeliaLab`; cópia em `.cache/verification/d11-delialab-bodyshape-backup.txt` para devolver. A figura (D9) não é a causa: o defeito existia antes e depois dela.
Pré-condição: servidor no ar; DLLs iguais ao D9; abrir pelo `start-lab.bat`.
- [ ] Seleção: a `DeliaLab` → anotar se o defeito da barriga e da perna sumiu.
- [ ] Entrar no jogo → anotar se entra (sem desconexão) e se o corpo está certo na cidade.
- [ ] Janela de habilidades → anotar se aparecem `Bastard Sword Mastery` e `Guard` da Delia.
Resultado esperado: entra na cidade; duas habilidades da Delia; seleção sem defeito se a forma do corpo era a causa.

Resultado do D11 (28/10/2026): entrou na cidade sem desconexão; cidade com *inner* e modelo certos; a seleção continua com o defeito da barriga e da perna (cabelo da Delia certo); **nenhuma habilidade** na janela. Sem as linhas de forma do corpo o defeito ficou igual: a forma do corpo não é a causa (linhas devolvidas da cópia).

### D12 — Janela de habilidades da classe 10; comparação de memória na seleção
Causa da janela vazia (estática): a lista da janela (`client.dll` `0x38372c00`) e o `IsAvailableClass` (`0x3835df60`) testam `cmp ecx, 9; ja` antes de `1 << classe`; toda habilidade de classe acima de 9 era pulada. Mudança: `CLASS_BITS` no patch (limite → 10). Aplicado com o jogo fechado: `client.dll` `29ad8b1f…`, `server.dll` `e6f1b67b…` (igual).
Seleção: a entidade da seleção é `C_NPC_PlayerForCharacterSelection` (o *slot* 264 dela é o da base, sem figura para nenhuma classe). A causa do defeito ainda não foi achada estaticamente; o passo abaixo compara a memória da entidade da seleção com a Delia e com uma Hagie.
Pré-condição: servidor no ar; abrir pelo `start-lab.bat`.
- [ ] Na seleção, com a `DeliaLab` selecionada: avisar; eu abro o leitor elevado (aprovar o UAC) e salvo a entidade da seleção.
- [ ] Selecionar uma Hagie da mesma conta: avisar; eu salvo e comparo.
- [ ] Entrar na cidade com a `DeliaLab` → abrir a janela de habilidades → anotar se aparecem `Bastard Sword Mastery` e `Guard`.
Resultado esperado: as duas habilidades da Delia na janela; a comparação aponta o campo ou modelo que difere na seleção.

Resultado do D12 (28/10/2026): as habilidades da Delia aparecem na janela, mas todas no mesmo lugar (uma sobre a outra). Seleção ainda com o defeito.
Memória (leitor elevado; `heapdword` novo no `lab_memory.py`, entidades só do cliente): a entidade da seleção da `DeliaLab` (`C_NPC_PlayerForCharacterSelection`) e a da cidade (`C_HeroesPlayer`) têm o **mesmo** modelo, `CoStUmE_10/0_1002/0_100/0_6036/0_-1/…/0_102/0_-1/0_.mdl`, e o mesmo mapa de forma do corpo (chaves 15 e 24 da Fiona). A Hagie da seleção tem a chave 105 = 1000.

### D13 — Figura padrão da Delia na seleção; posições das habilidades
Causas:
- **seleção:** a seleção aplica a forma do corpo guardada; a Delia criada no jogo guarda a figura padrão dela na chave 106 = 1000 (`csf/bodyshape_fig_list.txt` 106 = `bodyshape_delia_default.fig`; `csf/default_delia.bsh` marca essa figura `isDefaultFigure`, razão 1,0), como a Hagie guarda a 105. A `DeliaLab` (criada como Fiona) tinha as chaves da Fiona e não a 106. Na cidade a forma não é aplicada, por isso a cidade está certa. Mudança: `lab-set-class.ps1` troca o conjunto de chaves da classe antiga pelo da nova (Delia: 18, 19, 20, 24, 25, 40, 71–75 em 0 e 106 = 1000);
- **janela de habilidades:** `client.dll` `0x388e6ac0`/`0x388e6ae0` devolvem o nome/inicial da classe por tabelas de 10 (`0x38cb6d24`/`0x38cb6d4c`) e texto vazio acima de 9; o `SkillDialog.setClassInfo` recebia vazio para a Delia (o filme `SkillDialog.gfx` do 1.69 tem os quadros `Weapon_1st_Delia`). Mudança: `TABLES['client.dll']` + `CLASS_STRING_BOUNDS` (tabelas de 32 com "Delia"/"D", limite → 10).
Pré-condição: já aplicado (`client.dll` `4fb46646…`, `server.dll` `e6f1b67b…`; `DeliaLab` 10 → 1 → 10 com 12 linhas de forma do corpo); abrir pelo `start-lab.bat`.
- [ ] Seleção: a `DeliaLab` → anotar se o defeito da barriga e da perna sumiu.
- [ ] Cidade → janela de habilidades → anotar se as habilidades ficam em posições separadas.
- [ ] Não regressão: Hagie na seleção, na cidade e na janela de habilidades.
Resultado esperado: seleção sem defeito; habilidades da Delia organizadas na janela.

Comparação de arquivos da Delia, NA (`F:\Games\vindictus\hfsExtracted`) × cliente do laboratório (`.cache/verification/d13-delia-na-vs-lab.txt`): 485 arquivos com "delia" no laboratório, 3.156 no NA; todos os 485 existem no NA e 107 diferem. As malhas principais (`delia.vvd`, rostos `.vvd`) são iguais; mudaram cabeçalhos `.mdl` (`delia.mdl`, `delia_base.mdl`, rostos, *inners*), as animações (`pc_delia_battle_motion*.mdl`, `pc_delia_town_motion.mdl` e os `.txt` de movimento, `pc_delia_town.txt` 26 KB → 117 KB), 3 *inners* (malhas novas), texturas de 2 *inners*, scripts `create/prologue_delia.csf`, `default_delia.bsh` (o NA acrescenta a barra "waist2"), 17 vídeos de habilidade e 13 efeitos. Só no NA: 2.387 vozes de NPC, 39 vozes da Delia, roupas (`yukata_delia` e outras), 8 arquivos de `bastardsword`, `bodyshape_spine2_*_delia.fig` e figurinos `.csf`. Conclusão: a Delia do 1.69 é uma versão anterior, completa para jogar; o NA atualizou animações e modelos depois.

Resultado do D13 (28/10/2026): **habilidades da Delia certas na janela.** Hagie sem regressão. A seleção continua com o defeito da barriga e da perna (mesmo com a figura padrão 106 = 1000). A `Bastard Sword` continua com o modelo errado e "Cannot be equipped"; as armaduras-base aparecem vermelhas no inventário mas equipam.
- Armaduras vermelhas: `premium_newbie_boots/tunic/pants` têm `ClassRestriction` 2047 (inclui a classe 10); a marca vermelha vem de outra verificação, ainda não achada (as varreduras de `cmp reg, 9` antes de `1 << classe` e das escritas nos *flags* `+0x198..+0x1a2` não a mostram).
- Mais uma verificação de habilidade por classe (`client.dll` `0x385a5ed0`, `cmp eax, 9`) entrou no `CLASS_BITS`. Aplicado com o jogo fechado: `client.dll` `b3774da7…`.

### D14 — Capturas de tela para os três defeitos restantes
Pré-condição: DLLs `b3774da7…`/`e6f1b67b…`; abrir pelo `start-lab.bat`.
- [ ] Seleção: captura da `DeliaLab` (defeito da barriga/perna) e, se der, da mesma tela com uma Hagie.
- [ ] Cidade, inventário: captura da dica (*tooltip*) da `Bastard Sword` e da dica de uma armadura-base vermelha (`premium_newbie_tunic`).
- [ ] Cidade: captura da `DeliaLab` com a `Bastard Sword` equipada (modelo errado).
Resultado esperado: as capturas mostram o texto exato da dica e a forma do defeito, para escolher onde procurar.

Resultado do D14 (28/10/2026), pelas capturas do operador:
- **Seleção:** a Delia sentada tem as coxas brancas e brilhantes; a Evy na mesma pose não. Com outra *inner* (`delia_inner_cutecat`), tudo da barriga para baixo fica branco, com o desenho da *inner* por cima; braços, tronco e rosto normais. Na cidade as mesmas pernas têm pele normal. Não é forma do corpo nem modelo: é o sombreamento da pele da parte de baixo só na seleção. As *inners* usam o mesmo material de pele da Evy (`fiona_skin_body01`). O `delia_base.mdl` (laboratório e NA) cita `fiona_skin_body01_lod` numa pasta sem esse material, como o `hagie_base.mdl` (`lethita_skin_body01`), sem efeito na Hagie. O código do sombreamento SSS (`$sss_color`/`$sss_enable`, `client.dll` `0x386bb050`, `0x386bccc0`, `0x386c2370`) é por material; nenhuma tabela por classe achada ainda.
- **Arma:** com a `Bastard Sword` equipada aparecem duas espadas, uma na mão e uma nas costas; o certo é só nas costas (o 1.69 não tem o tipo de arma `BASTARDSWORD`; E2).
- **Inventário:** dica "Bastard Sword, Weapon", **"Cannot be equipped"** em vermelho; as armaduras-base aparecem vermelhas.

### D15 — Pele branca da seleção: experimentos
Descartados (o operador viu a pele ainda branca em cada um, 28/10/2026):
- cores de pele/rosto (roupa-base chave 2) trocadas pelas do `create_delia.csf` (devolvidas depois);
- figura padrão desligada (`csf/default_delia.bsh` solto no laboratório com `bodyshape_null.fig` e chave 106 = 0; desfeito);
- material `fiona_skin_body01_lod` copiado para a pasta da Delia (o `delia_base.mdl` o procura lá; desfeito);
- a pintura do corpo (tipo 2) da Tsuko (Evy, normal) é igual à da `DeliaLab`.
Achado: o gerenciador de forma do corpo do cliente (`0x387602b0` carrega, `0x38760650`/`0x387606b0` são as entradas por entidade) **pula a classe acima de 9** (`cmp reg, 9; ja`) e guarda 4 grupos de 10 mapas por classe e *flags* em `+0x460`. A seleção é a única tela que mistura formas do corpo. Experimento: `SHAPE_CLASS` faz a classe 10 usar a vaga da Lynn (7) nessas duas entradas, sem mudar o armazenamento. Aplicado: `client.dll` `ae521dec…`.
- [ ] Seleção: a `DeliaLab` → anotar se a barriga e as pernas deixam de ficar brancas.
- [ ] Não regressão: Hagie e Tsuko (Evy) na seleção.
Resultado esperado: se a pele ficar normal, a causa é o gerenciador pular a classe 10, e o próximo passo é dar à Delia mapas próprios.

Resultado do D15 (28/10/2026): **sem mudança** com o `SHAPE_CLASS` (Delia → vaga da Lynn); Evy (Tsuko) e Fiona (Murael) certas na mesma tela. Nova captura com a *inner* 90035 (`fiona_innerarmor_jpn_15.mdl`, biquíni): a faixa branca vai da cintura até o meio da coxa, com borda recortada, por baixo do biquíni. O operador confirma: na cidade o modelo fica perfeito; só a seleção mostra a faixa.
Memória (seleção): as entidades da Delia, Evy e Fiona diferem só na sequência de animação (`+0x6a0`); o modelo composto é `CoStUmE_10/0_90035/0_100/0_6036/…/0_102/…` (o mesmo nome que a cidade monta). Descartados também: parte padrão de calça (a Delia não tem), `delia_base` (sem `.vvd`, não desenha), `.cloth` (só esferas de colisão), ossos de `.jiggle` (iguais aos da Lynn), materiais do rosto e da animação (todos presentes).

### D16 — Rastreio ao vivo da faixa branca (seleção)
Memória (leitor elevado; comandos novos `studios FILTRO` e `studio ENDEREÇO [bones|seq N,…]` no `lab_memory.py`):
- não há modelo composto único: a roupa é desenhada pelas partes; `delia_base.mdl` não tem malhas; a *inner* 90035 é `fiona_innerarmor_jpn_15.mdl` (malhas: pele `fiona_skin_body01`, metal, tecido). A faixa branca e os braços normais são **a mesma malha e o mesmo material**, então a diferença é por vértice (ossos/animação), não por material;
- as bases mescladas da Evy e da Fiona mostram a mesma tabela de ossos estendida: não é diferença da Delia;
- o *proxy* de pele (`0x386bb050`, `$sss_color`/`$composite_map`, decalque da pintura do corpo) recebe da entidade os mesmos valores da Evy;
- a seleção toca a família "descanso" da cidade: na Delia a sequência 43 é `town_idle2restb` (`ACT_TOWN_IDLE_TO_REST_B`) de `pc_delia_town_motion.mdl`; o NA refez esse modelo (4,6 MB → 6,7 MB) e o `pc_delia_town.txt` (26 KB → 117 KB).
Hipótese: as animações de descanso do 1.69 da Delia deformam pelve e coxas; a cidade normalmente toca parada/caminhada, por isso parece certa.
- [ ] Na cidade, com a `DeliaLab` parada (sem mexer) até ela entrar na pose de descanso (sentar/relaxar): anotar se a faixa branca aparece.
Resultado esperado: se aparecer na cidade, a causa é a animação de descanso (dados), não o código da seleção.

Resultado do D16 (28/10/2026): na cidade, parada até a pose de descanso, sentando e com emotes: **nenhuma faixa branca**. A animação não é a causa; o defeito é só da seleção.
Mais rastreio ao vivo na seleção (jogo aberto, `lab_memory.py`; `poke` novo, só para experimento de laboratório):
- a `.bsh` padrão carregada pelo cliente é a original (o `.bsh` solto do D15 nunca foi lido; aquele teste não vale);
- classe da entidade da seleção trocada de 10 para 7 na memória: o jogo regravou 10 em seguida (experimento inconclusivo);
- matrizes dos ossos calculadas na seleção (`+0x7b0`/`+0x82c`, 81 ossos, matrizes 3×4): escalas da Delia (0,92 geral; pelve 0,92/0,98/0,95; coxas 0,92/0,92/0,95) comparáveis às da Evy (0,95; coxas 0,95/1,05/1,05). Posição e normal dos vértices estão certas: a faixa vem da cor/textura por pixel;
- modelos de movimento não têm malha; a pele e o *proxy* de decalque/composição (`$composite_map`, `$decal_*`, `client.dll` `0x386bb880`) são o caminho restante.
Próximo passo: ponto de parada no *proxy* de composição (`0x386bb880`, argumento = entidade) para ler a textura composta e o fator da Delia e da Evy.
- Primeira tentativa do ponto de parada (28/10/2026): o jogo caiu em `client.dll` `0x386bb890`. Causa: o `lab_watch.py --exec` contava com `EFlags.RF` para seguir depois do ponto de parada, mas nas *threads* WOW64 o RF não vale; a mesma instrução disparou de novo até o limite, a *thread* de desenho ficou parada ~30 s e o objeto ficou inválido. Correção na ferramenta: desarma a *thread*, executa um passo (TF) e rearma; antes de desanexar, espera os passos pendentes. Dado colhido antes da queda: estado do *proxy* `0x4fbc34cc` (contagem 1, textura composta `0x4f977408`, matriz identidade/escala).
- Segunda tentativa (com o passo TF): o jogo caiu com `EXCEPTION_SINGLE_STEP` em `0x386bb881`. O ponto de parada disparou 40 vezes na mesma instrução: nem `RF` nem `Dr7 = 0` + `TF` gravados por `Wow64SetThreadContext` no evento de exceção valem para as *threads* do jogo; ao desanexar, o passo pendente chegou ao jogo. **Pontos de parada de execução do `lab_watch.py` não são confiáveis neste jogo**; os de escrita (D8o–D8r) continuam válidos. Dado colhido: todos os acertos vêm de um só objeto (`esi` = `ecx - 0x64`), com contagem 1 e textura composta (`+8`); o próximo passo lê esses objetos pela vtable com o leitor, sem depurador.

### D17 — Causa da faixa branca da seleção e correção
Rastreio ao vivo (leitor elevado; ponto de parada de escrita):
- cada personagem da seleção tem um `SkinMaterialParamModifier`; só o da Delia tinha um decalque ativo (tipo 1), vindo de `CHeroesCostumeData` `+0x240` = `m_bodyLayerList[0]` (camada de pintura do corpo). Fiona, Evy e Hagie: -1. Delia: `0x4d205cb0`, o mesmo valor a cada reconstrução;
- os personagens da seleção são NPCs do `server.dll` local (`CNPC_BaseCostumeLoadable::m_curCostumeData`, enviado em rede); o valor nasce no servidor: `0x40411e60` (montagem do NPC da seleção) usa sempre o padrão `0x40815490(slot, classe)` = `tabela[slot*10 + classe]` sobre `0x40f6494c`, **sem limite**. A tabela tem 10 vezes -1; a classe 10 lia o dword seguinte (um ponteiro). O cliente desenhava esse "número de pintura" como decalque branco na barriga e nas coxas. Na cidade o jogador só usa o padrão quando não tem valor, por isso a cidade estava certa.
Mudança: `CLASS_DEFAULTS` no patch: os quatro acessores `[slot][10]` (servidor `0x40815390`/`0x40815490`, cliente `0x3887ce30`/`0x3887cf30`) leem a coluna da classe copiada (`clone_of`, Hagie) para uma classe extra. `python -m unittest test_patch_class_routing` (21, 0 falhas; `d16-routing-tests.log`).
Pré-condição: aplicado (`client.dll` `8e766ed3…`, `server.dll` `46cd2c50…`); abrir pelo `start-lab.bat`.
- [ ] Seleção: a `DeliaLab` → a faixa branca da barriga e das coxas deve sumir.
- [ ] Não regressão: Hagie, Evy e Fiona na seleção; Delia na cidade.
Resultado do D17 (28/10/2026): **passou.** A faixa branca sumiu da Delia na seleção (operador: "Funcionou, problema resolvido").

### D18 — E2: arma da Delia
Mudança (passo 31): a entidade da arma vem de uma lista por classe nos dois DLLs, que mandava a classe 10 para a lista da Lethita (`weapon_dualsword`: duas espadas). A Delia agora tem lista própria com `weapon_greatsword` (o 1.69 não tem a `weapon_bastardsword` do NA; é a espada grande nas costas mais próxima). O servidor Rust passa a aplicar a maestria `skill_delia_bastard_mastery` à `Bastard Sword` (imagem Docker reconstruída).
Pré-condição: aplicado (`client.dll` `67bcca2f…`, `server.dll` `27e437b7…`; a primeira aplicação, `d78bf797…`/`62d3aed6…`, derrubou o `server.dll` em `0x40825900` ao entrar na cidade: a linha tinha 1 nome e o leitor `0x40825890` percorre 5); servidor reiniciado; abrir pelo `start-lab.bat`.
- [ ] Cidade com a `DeliaLab` e a `Bastard Sword` equipada: anotar se aparece **uma** espada (nas costas, ou na mão ao atacar) em vez de duas.
- [ ] Entrar numa missão (Mercenary Outpost → qualquer batalha): ataque básico e um *smash* com a espada; anotar animação, acerto e dano.
- [ ] Inventário: dica da `Bastard Sword` (ainda "Cannot be equipped"?) e cor das armaduras-base (ainda vermelhas?). Se continuarem, deixar o jogo aberto na cidade e avisar: eu leio os *flags* do item na memória.
- [ ] Não regressão: Hagie com a adaga, Fiona/Evy com as armas delas.
Resultado esperado: uma espada só; ataque funciona.
