# Roteiro de testes no cliente — B5 social (sussurro, bloqueio, guilda)

Sistemas sociais do servidor Rust (`docs/social-systems.md`). Este roteiro cresce a cada etapa (SO1 sussurro, SO2 bloqueio, G1–G6 guildas); a preparação, a ordem e o relato finais são completados no fechamento do bloco. Precisa de **duas contas e dois clientes** (A e B no jogo ao mesmo tempo).

## Preparação
1. Runtime: depois do commit, `COMPOSE_PARALLEL_LIMIT=1 docker compose build` e `docker compose up -d --wait`; `docker compose ps` mostra `game` e `server` saudáveis.
2. Clientes: `.cache/play-client/start-lab.bat` com `lab169_a` (personagem A) e um segundo cliente com `lab169_b` (personagem B).
3. Relatórios de queda, se houver: `Documents\Vindictus EU GM\Report Error`.

## Como relatar
Para cada linha marcada: ID e número da linha (ex.: `SO1-2`), o que apareceu na tela (texto, captura se possível), se o cliente fechou ou travou, o horário aproximado (para comparar com `docker compose logs game`) e o nome do relatório de queda, se houver.

### SO1 — Sussurro
Pré-condição: A e B no jogo (cidade), cada um num cliente.
- [ ] A sussurra para B (modo Sussurro do chat ou comando de sussurro com o nome de B) → B recebe a mensagem com o nome de A como remetente; A vê a própria mensagem no chat; nenhum erro.
- [ ] B responde a A → A recebe.
- [ ] A sussurra para B escrevendo o nome com maiúsculas/minúsculas diferentes → B recebe.
- [ ] Texto com acentos (ex.: `olá, ação`) → chega igual.
- [ ] A sussurra para um nome que não existe → aviso de nome de personagem inválido (`GameUI_Heroes_Chat_Whisper_Invalid_Character_Name`).
- [ ] B sai para a seleção de personagem (ou fecha o cliente); A sussurra para B → aviso de que B não está online (`GameUI_Heroes_Chat_Whisper_Not_Online`).
- [ ] B volta ao jogo; A sussurra de novo → B recebe.
- [ ] Com A numa missão (navio/dungeon) e B na cidade, um sussurra para o outro → chega nos dois sentidos.

### SO2 — Bloqueio
Pré-condição: SO1; A e B no jogo; A tem uma segunda personagem A2 na mesma conta (para a linha do próprio nome).
- [ ] A abre a Comunidade → aba Bloqueados vazia.
- [ ] A bloqueia B (menu do personagem/chat ou a aba Bloqueados, pelo nome) → aviso de bloqueio com o nome de B (`GameUI_Heroes_WhisperFilter_Success_Add`); B aparece na aba Bloqueados.
- [ ] A bloqueia B de novo → aviso de que não foi possível bloquear (`GameUI_Heroes_WhisperFilter_Cannot_Add`); a lista não muda.
- [ ] A bloqueia um nome que não existe → aviso de nome inválido (`GameUI_Heroes_Chat_Whisper_Invalid_Character_Name`).
- [ ] A tenta bloquear o próprio nome ou o de A2 → aviso `GameUI_Heroes_WhisperFilter_Cannot_Filter_Yourself`.
- [ ] A sai para a seleção e volta → B continua na aba Bloqueados.
- [ ] B sussurra para A com o bloqueio ativo → anotar se a mensagem aparece para A (o servidor original entrega o sussurro e deixa o filtro para o cliente).
- [ ] A desbloqueia B → aviso `GameUI_Heroes_WhisperFilter_Success_Remove`; B sai da lista. Desbloquear de novo → `GameUI_Heroes_WhisperFilter_Cannot_Remove`.

### G1 — Criar guilda, dados e busca
Pré-condição: A sem guilda, nível 20 ou mais e com pelo menos 300.000 de ouro (painel ou `./scripts/dev/lab-give-item.ps1 -ItemClass gold -Count 300000 -Cid <cidA>` com A fora do jogo); B sem guilda.
- [ ] A entra no jogo → nenhuma guilda na Comunidade (aba Guilda vazia), sem erro.
- [ ] A pede a criação de guilda (botão de criar guilda da Comunidade) → a história de criação de guilda aparece na lista de histórias; anotar a NPC indicada.
- [ ] A fala com a NPC da história (Ceara/soldado em Rochest) → diálogo de criação; ao aceitar, o ouro cai **300.000** e a história avança.
- [ ] A abre a janela de criação e cria com nome inválido (`abc` ou com espaço) → erro de regra de nome; com nome de mais de 20 letras → erro de tamanho.
- [ ] A cria a guilda `Teste_Guilda` com um ID (ex.: `testeguilda`) → sucesso; a aba Guilda mostra a guilda, nível 1, A como mestre, 1 membro, máximo 100, A online na lista; a história de criação some.
- [ ] B tenta criar outra guilda com o mesmo nome (com a história e o ouro) → erro de nome duplicado; com outro nome e o mesmo ID → erro de ID duplicado.
- [ ] A sai para a seleção e volta → a guilda continua na aba Guilda, com A online.
- [ ] B busca guildas por `Teste` → `Teste_Guilda` aparece na lista; busca com 1 letra → falha de busca; busca pelo nome do mestre (A) → a guilda aparece.

### G2 — Membros, cargos, mestre, notícia, sair e dissolver
Pré-condição: G1 (A mestre de `Teste_Guilda`); B sem guilda; A e B no jogo ao mesmo tempo; C (terceira personagem, outra conta) para a recusa.
- [ ] B pede para entrar em `Teste_Guilda` (busca → entrar) → sucesso; para B a guilda aparece como pendente (sem a lista de membros); para A, B aparece como aguardando na lista.
- [ ] A aprova B → aviso de operação com sucesso; os dois veem 2 membros; B passa a ver a lista completa.
- [ ] A promove B a operador → B aparece como operador. A tenta promover B de novo → aviso de falha da operação.
- [ ] C pede para entrar e B (operador) recusa → C some da lista e C fica sem guilda.
- [ ] B (operador) tenta passar a liderança → aviso `GameUI_Heroes_ChangeMaster_NotMatchOldMaster`. A passa a liderança a B → diálogo `GameUI_Heroes_ChangeMaster_Success`; B vira mestre e A operador para os dois.
- [ ] A edita a notícia da guilda → os dois veem a notícia nova.
- [ ] A sai para a seleção → B vê A offline na lista na hora.
- [ ] A volta; B expulsa A → A recebe o aviso de saída da guilda e fica sem guilda; B vê 1 membro.
- [ ] A pede para entrar de novo e sai por conta própria antes da aprovação → aviso de saída; A sem guilda.
- [ ] B dissolve a guilda com outro membro aguardando → falha `HasMember`; sem outros membros → sucesso; B sem guilda; a busca não mostra mais a guilda e o nome pode ser usado de novo (o ID não).

### G3 — Chat de guilda
Pré-condição: G2 até a troca de mestre (A e B membros regulares, os dois no jogo); C aguardando aprovação na guilda, no jogo.
- [ ] A abre o chat de guilda (aba de guilda do chat) → entra sem erro. B abre o chat → A vê o aviso de que B entrou.
- [ ] A escreve no chat de guilda (com acentos) → A e B veem a mensagem com o nome de A; C não vê.
- [ ] C (aguardando) tenta escrever no chat de guilda → aviso `GameUI_Heroes_InGameGuildChat_Fail_NotEnoughPermission`.
- [ ] B sai do jogo → A vê o aviso de que B saiu do chat e B offline na lista.

### G4 — GP e nível de guilda
Pré-condição: A mestre de uma guilda nível 1, no jogo; B membro regular, no jogo. Anotar o AP de recompensa de cada missão. Os números vêm do conteúdo EN-EU: nível 1 → 2 exige 1.000 GP; teto diário de 2.000 GP (PvE); GP = AP da recompensa; reset diário às 07:00 UTC.
- [ ] A conclui uma missão com recompensa de AP → a janela da guilda mostra o GP ganho hoje e o GP da guilda subindo; B vê o mesmo.
- [ ] Ao passar de 1.000 GP → aviso de nível de guilda para A e B; a guilda mostra nível 2.
- [ ] A e B recebem pelo correio a cápsula de AP da subida de nível (título `GameUI_Heroes_GuildLevelUpItemMail_Title`); um membro offline recebe ao entrar.
- [ ] Depois de 2.000 GP no dia, novas missões não somam GP até o reset das 07:00 UTC.
- [ ] Um membro aguardando aprovação conclui uma missão → a guilda não ganha GP.
