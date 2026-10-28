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
