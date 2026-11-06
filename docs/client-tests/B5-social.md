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

### G5 — Guilda nos outros sistemas
Pré-condição: A membro regular de uma guilda; C membro aguardando aprovação; níveis maiores de guilda pedem o nível ajustado no banco do laboratório (pedir ao Claude: `game_guild.level`). Bônus por nível (EN-EU, o valor da última linha até o nível): EXP de batalha 1/3/5 % (níveis 1/7/13), taxa do mercado −1/2/3 % (3/8/15), reparo −1/2/3 % (4/11/19), refino −1/2/3 % (6/12/18), AP de batalha +1 (16, só com o objetivo principal).
- [ ] A, no jogo, abre a própria ficha ou é visto por B → o nome da guilda aparece; C (aguardando) aparece sem guilda.
- [ ] A conclui uma missão com a guilda nível 1 ou mais → a EXP da recompensa traz o bônus de guilda (comparar com a mesma missão sem guilda).
- [ ] Com a guilda nível 3+, A registra um item no mercado → a taxa cai a porcentagem do nível; nível 4+, reparo mais barato; nível 6+, refino mais barato.
- [ ] Sem guilda (ou aguardando), A tenta usar o kit de fogueira de guilda (`guild_campfire_kit_lvl1`) → recusado por nível; com guilda nível 10+ → permitido.
- [ ] A tenta excluir um personagem que está numa guilda (inclusive aguardando) → recusa `FailInGuild`; depois de sair da guilda, a exclusão funciona.

### RK1–RK2 — Ranking
Pré-condição: A e B com missões concluídas em dias diferentes; o ranking publica às 07:00 UTC (e na subida do servidor, se a publicação estiver atrasada). Para testar no mesmo dia, peça ao Claude para forçar uma publicação.
- [ ] Depois da publicação, A abre a janela de ranking → aparecem os primeiros de cada ranking (batalhas concluídas, tempos por missão).
- [ ] A abre um ranking (ex.: batalhas concluídas) → lista com até 100 nomes, posição e pontuação; troca diário/semanal/mensal.
- [ ] A consulta o próprio ranking e o de B (pelo nome) → posições de cada um.
- [ ] A marca um ranking como favorito → aparece na aba de favoritos; desmarca → some.
- [ ] Ao entrar pela primeira vez no top 100 → aviso de ranking (alerta) uma única vez.
- [ ] Anotar se o banner de ranking da cidade fica vazio (o sorteio do banner não está migrado).

### RK3 — Títulos de rank e seleção de título
Pré-condição: os títulos de rank do EU (`RankTitle`, 12 linhas) são só dos rankings de PvP (DeathMatch 60001–60003, Arena 60005): ninguém os recebe até o PvP alimentar o ranking (PV2–PV4). Para testar antes, peça ao Claude para gravar uma linha semanal de ranking (período 2, posição 1–3) para o personagem e reiniciar o servidor.
- [ ] A abre a janela de títulos e seleciona um título adquirido (ex.: de nível) → o título fica selecionado; ao sair e voltar continua selecionado.
- [ ] A seleciona "nenhum título" → volta a ficar sem título.
- [ ] (Com a linha de ranking gravada) A entra no jogo → o título de rank aparece na lista como adquirido e pode ser selecionado.
- [ ] (Depois de apagar a linha e reiniciar o servidor) A entra → o título some da lista; se estava selecionado, volta a nenhum título com aviso.
- [ ] Anotar se outro jogador vê o título selecionado de A ao encontrá-lo na cidade (o resumo do personagem leva o título).

### PV1 — Duelo livre
Pré-condição: A e B no mesmo grupo, numa partida (MicroPlay) com A como anfitrião, onde o cliente permita o duelo livre entre jogadores.
- [ ] A e B duelam até um vencer → nada muda na tela na hora (o servidor não responde ao relatório).
- [ ] Depois da partida, A e B abrem as informações do personagem (ou a lista de personagens) → o vencedor tem uma vitória a mais e o perdedor uma derrota a mais em duelo livre.
- [ ] Anotar se o cliente não oferece o duelo em nenhuma partida (o PvP do EU depende do servidor dedicado, D-S2).

### G6b — Armazém da guilda: abrir e configurar (confirma a I-S2)
Pré-condição: no painel, **Ajustes de jogo → Guilda**, "Membros para usar o armazém" (`GuildStorageRequiredMember`, 20 no EU) = 2 e Salvar (vale na hora). Guilda com A (mestre) e B (membro). Na mesma aba ficam o armazém ligado/desligado, o nível de guilda, o GP por AP e a hora do reset diário de GP.
- [ ] B abre o armazém da guilda → a janela abre vazia, com 0 slots (a compra de abas é o G6e), e as abas de registro vazias. **Este é o teste da I-S2**: se nada abrir, anote; o log do servidor mostra a categoria recebida como "unhandled".
- [ ] B tenta mudar a configuração (limite de ouro/acesso por aba) → aviso de posto insuficiente.
- [ ] A muda a configuração com B com o armazém aberto → a janela de B mostra a configuração nova.
- [ ] Com o mínimo em 3 no painel (só A e B na guilda) → A tenta configurar e recebe o aviso de armazém parado; volte para 2.
- [ ] Depositar, retirar, arrumar ou comprar abas ainda não funciona (G6c–G6e): anote se o cliente trava.

### Carruagem (rank_basis_carriage_move)
Pré-condição: personagem em Rochester, perto da carruagem (CARRIAGE_ROCHEST).
- [ ] Falar com a carruagem e escolher a ida para Colhen → depois da contagem, o personagem entra em Colhen (o servidor soma 1 no ranking "GamePlay_take_carriage" e segue com `cc_enter_colhen_from_rochest`).
- [ ] Na variante paga, o ouro cai 2000 antes da viagem; sem ouro, aparece a mensagem de falta de ouro e não há viagem.

### G6e — Armazém da guilda: comprar slots
Pré-condição: roteiro G6b (o armazém abre; confirmado em 05/11). No EU original toda compra usa o cupom `guildstorage_ticket` do cash shop, que o catálogo do laboratório não tem. Para testar, em Ajustes de jogo → Guilda ligue **Slots do armazém grátis** (1), ou desligue **Slots do armazém só por cash** (0) para pagar a primeira compra em ouro (**Preço em ouro da primeira compra**, original 30.000.000; ponha um valor que o personagem tenha).
- [ ] A abre o armazém e clica em Purchase → a primeira bolsa mostra 24 slots; com ouro, o ouro de A cai pelo preço.
- [ ] B com o armazém aberto ao mesmo tempo → a janela de B mostra os 24 slots sem reabrir.
- [ ] Comprar de novo → +24 slots por compra (grátis, ou falha "compra falhou" sem o cupom).
- [ ] Com os valores originais (só por cash ligado, grátis desligado) → aviso de compra falhou, nada muda.
- [ ] Fechar e reabrir o armazém, e relogar → os slots continuam.

### G6c/G6d — Armazém da guilda: depositar, retirar e arrumar
Pré-condição: G6e (a guilda já comprou slots). A é o mestre, B um membro, ambos com o armazém aberto. A configuração inicial (898779) limita cada aba ao posto de membro ou acima.
- [ ] A arrasta um item negociável do inventário para um slot do armazém → o item sai do inventário de A e aparece no slot escolhido; a janela de B mostra o item sem reabrir.
- [ ] A deposita mais do mesmo material empilhável → junta na pilha do armazém.
- [ ] A deposita ouro (Deposit) → o ouro de A cai e o "Available Balance" do armazém sobe; B vê o novo saldo.
- [ ] B retira o item (arrasta para o inventário) → o item vai para o inventário de B e some do armazém de A.
- [ ] B retira ouro (Withdraw) → o ouro de B sobe; acima do limite de retirada da configuração ou de 2.500.000 de uma vez → aviso de limite.
- [ ] A arrasta um item de um slot para outro do armazém → o item muda de slot (se o destino estiver ocupado, os dois trocam).
- [ ] Item quebrado (durabilidade abaixo do máximo), não negociável ou de cash → aviso e nada muda.
- [ ] Relogar e reabrir o armazém → itens e ouro continuam.

### G6f — Armazém da guilda: histórico
Pré-condição: G6c/G6d feitos no mesmo dia (UTC). A e B com o armazém aberto.
- [ ] Aba "Storage Usage Records" → uma linha por personagem e tipo do dia (ouro, itens, slots) com os totais depositados e retirados; B vê a linha de A logo depois do depósito, sem reabrir.
- [ ] Aba "Gold Transaction History"/itens → cada depósito e retirada de item com nome, quantidade e cores (até 10 por personagem por dia).
- [ ] Comprar slot → linha de slot no histórico.
- [ ] Retirar ouro: o limite diário de 2.500.000 soma as retiradas do dia menos os depósitos do dia (com mais depósitos, dá para retirar mais).
- [ ] No dia seguinte (UTC), reabrir o armazém → as linhas de ontem aparecem no histórico antigo.

### PV2c — Arena: canais, entrada e anfitrião (experimento D-S2; confirma se o cliente hospeda o p05)
Pré-condição: Docker reconstruído; contas A e B com personagens na cidade, fora de party e de missão. No EU, abaixo do nível 40 a Arena aceita o personagem como espectador (ArenaSpecatorLevelLimitMin 1), então qualquer nível entra. Ainda não há lutas na Arena (as rodadas, os comandos e as recompensas são o PV3).
1. [ ] A abre o PvP/Arena → lista de 3 canais "0/32". Anote se o botão de PvP aparece e se a janela abre.
2. [ ] A entra no canal 0 → mensagens de espera ("aguardando o anfitrião"/consulta do personagem) e, em seguida, **o cliente de A carrega o mapa da Arena como anfitrião**. **Este é o teste do D-S2:** anote se o mapa carrega, se fica preso numa mensagem ou se o cliente fecha (com o horário).
3. [ ] B abre a lista → canal 0 com "1/32"; B entra no canal 0 → cerca de 10 s depois de A carregar o mapa, B entra na Arena de A (os dois se veem).
4. [ ] Dentro da Arena: anote o que cada um vê (placar, botões de entrar na fila de luta); nenhuma luta começa ainda.
5. [ ] B sai da Arena pela interface → B volta à cidade; A continua. Depois A sai → A volta à cidade e, se B ainda estiver dentro, B também sai (a saída do anfitrião fecha a Arena).
Relato: o resultado de cada passo (ok/falhou), o horário e um print do que aparecer. Os logs do servidor eu leio.

### PV3a — Arena: lutas, rodadas e recompensas (dois clientes)
Pré-condição: PV2c confirmado (A hospedou a Arena em 06/11). A e B com nível 40 ou mais (abaixo disso a fila de luta recusa com "nível mínimo"); para ganhar selos, cada um precisa ter `honor_stamp_limit` no inventário (a recompensa troca um `honor_stamp_limit` por um `honor_stamp`, como no original; sem ele a luta conta mas não dá selo).
1. [ ] A e B entram no mesmo canal (A anfitrião). Os dois clicam em "Match Request" do mesmo quadro (1 dos 4) → mensagens de fila/escalação no quadro; ~5 s depois "aguardando início", contagem de 10 s e a luta começa (90 s).
2. [ ] Um derrota o outro → fim da luta: o vencedor recebe "selo de honra recebido" e o resultado de Arena (vitórias/derrotas/sequência), o perdedor vê "você perdeu" e sai do quadro; 15 s depois o ciclo volta a "aguardando partida".
3. [ ] Repetir a mesma dupla em até 10 min → a luta acontece mas sem selos (SamePlayerRewardCoolDown).
4. [ ] Deixar o tempo acabar com os dois vivos → empate: os dois saem do quadro, sem selos.
5. [ ] Durante uma luta, o perdedor (B) sai da Arena → A vence por W.O.
6. [ ] "Observe" e o histórico de Arena no personagem não fazem parte deste passo; anote o que acontecer.
Relato: o resultado de cada passo, o horário e prints da contagem, do fim da luta e das mensagens de selo.

### PV3b — DeathMatch e GodMatch (dois clientes)
Pré-condição: nos dados do EU os times são de 8 (DeathMatch p03, só guilda; p03_free sem guilda, 6) e de 6 (GodMatch p04), então com dois clientes é preciso baixar "Jogadores por time" para 1 no painel (Ajustes de jogo → PvP). O registro de party inteira não está migrado: cada jogador entra sozinho.
1. [ ] A e B entram em GodMatch (ou DeathMatch sem guilda) → A vê "aguardando oponente"; com B, os dois veem "oponente encontrado" e o pedido de confirmação; os dois confirmam.
2. [ ] A partida abre com o primeiro jogador como anfitrião (o cliente carrega o mapa p04/p03), o outro entra ~10 s depois; ~10 s com os dois prontos começa a contagem (60 s) e a partida.
3. [ ] GodMatch: derrubar o deus do outro time → fim (se for antes de 7 min, aviso de "partida rápida demais" e o fim espera); DeathMatch: vencer 2 rodadas.
4. [ ] Fim: selos para vencedor e perdedor; cada um confirma a saída (ou todos saem em 32 s).
5. [ ] O anfitrião sai no meio → o outro vira anfitrião (recarrega como host).

### G7 — Guilda recomendada para novatos
Pré-condição: A mestre de uma guilda, B sem guilda (duas contas).
1. [ ] A liga "recomendar a novatos" na janela da guilda (configuração de mestre/oficial).
2. [ ] B (sem guilda) abre a busca de guilda recomendada → aparece a guilda de A (precisa ter alguém online nela).
3. [ ] B pede para entrar pela recomendação → A recebe o aviso do pedido do novato; B fica como candidato na guilda (A aprova como sempre).

### G8 — Compra direta de vagas na guilda
Pré-condição: A mestre; o cash shop do laboratório precisa ter um produto `guild_extension_member_N` (cadastre pelo painel se não houver) e saldo de cash.
1. [ ] A compra a extensão de membros → o limite da guilda sobe N (até 200); B (não mestre) recebe "não é mestre".
2. [ ] Sem saldo ou sem o produto → falha sem mudar o limite.

### PV — Registro de party no PvP
1. [ ] A (mestre de party com B) entra no GodMatch/DeathMatch com "jogadores por time" 2 no painel → os dois veem "aguardando oponente" juntos; na Arena a party é recusada (só solo).
2. [ ] Enquanto registrados, convites, saída e expulsão de party são ignorados.
