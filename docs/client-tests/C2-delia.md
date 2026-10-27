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
