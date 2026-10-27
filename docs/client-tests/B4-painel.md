# Roteiro de testes no cliente — B4 painel de administração

Bloco B4 (painel de administração e análise de extensões). Plano em `docs/b4-painel-plano.md`. Gate fechado em 27/10/2026 (resultado no `MIGRATION.md`). Algumas entradas já foram testadas pelo operador (marcadas com [x]); as demais seguem a ordem abaixo.

## Preparação
1. Runtime no Docker recompilado no gate (27/10/2026): `docker compose ps` mostra `game` e `server` saudáveis. Se o código mudou depois disso: `COMPOSE_PARALLEL_LIMIT=1 docker compose build` e `docker compose up -d --wait` (o serviço `migrate` aplica as migrações até a 0048).
2. Painel: `http://127.0.0.1:27090` (só no próprio computador). Conta de administração `admin` (papel admin), ligada ao usuário de jogo `lab169_a` (Fiona Murael, nível 20).
3. Cliente: abrir sempre pelo `.cache/play-client/start-lab.bat` (atualiza a cópia do cliente com a versão de conteúdo publicada e abre o jogo como o `start.bat`). No ID, digite o login da conta; a senha fica em branco e a segunda senha continua a mesma.
4. Conteúdo privado exportado: `.cache/private-content/item-names.json` e `panel-catalog.json` com `icons/` (comandos nas pré-condições do B4.3a e do B4.4e).
5. Antes de começar, anote no painel, em Conteúdo, a última versão publicada; edições de teste antigas ficam pendentes até serem publicadas ou retiradas.

## Ordem de gameplay (uma sessão)
1. **Painel, contas e sessão:** B4.2 e B4.4f (entrar, navegar pelo menu inteiro sem voltar ao login, cadastro de `teste_b4`, troca de senha).
2. **Login no jogo e personagens novos:** B4.2-L (se quiser repetir), B4.N1 e B4.N2 (com `teste_b4`: Check do nome, criar uma Evy ou outra classe e entrar na cidade). Sair para a seleção: FX-relist.
3. **Leitura do personagem:** com a Murael fora do jogo, B4.3a e B4.4e (ficha, inventário, tooltip).
4. **Mudanças no personagem:** B4.3b, B4.3c e B4.4d (dar e remover itens, ouro, cash, progressão, arrastar e organizar); entrar no jogo e conferir.
5. **Mapas e canais com alguém no jogo:** com a Murael na cidade, B4.6 (lista do canal, conexões); entrar numa missão e voltar.
6. **Ajustes ao vivo:** B4.5b (mudar a EXP das missões, terminar uma missão sem sair do jogo).
7. **Lojas, cash shop e versão publicada:** B4.4b, B4.4 e B4.5 (editar uma loja de NPC, um preço do cash shop e uma taxa pendente), depois B4.4c (publicar com manutenção, deixar o jogo reiniciar, abrir pelo `start-lab.bat` e conferir a loja e o preço novos).
8. **Fim:** relogar com a Murael e o personagem novo e conferir o que as entradas listam como preservado; Auditoria deve ter uma linha para cada mudança feita.
9. **Editor de itens (depois do B4, opcional na mesma sessão):** I2 com a Fiona fora do jogo: criar, clonar com kit, editar, ocultar e remover; publicar; abrir pelo `start-lab.bat`; dar os itens e conferir no jogo; relogar.

### P2 — Busca por categoria, refino e scrolls de um item pelo painel
Pré-condição: imagens recompiladas e catálogo reexportado depois dos commits da busca por categoria e do editor de refino (`py -3.13 scripts/content/refresh_item_content.py`, depois `COMPOSE_PARALLEL_LIMIT=1 docker compose build` e `docker compose up -d --wait`); login no painel como `gm` ou `admin`; uma Fiona de laboratório offline com uma espada longa (ex.: `longsword_swordwind`) na bolsa. Cliente fechado.
- [ ] Ficha → **Dar item** → **Catálogo** → Weapon → Longsword → a lista mostra só espadas longas, paginada, com o filtro de classe já na Fiona; escolher uma → a classe vai para o campo.
- [ ] Ainda em Dar item, digitar `longsword_swordwind[ENHANCE:20]` → recusa "Nível de refino fora dos níveis do tipo do item"; `longsword_swordwind[PREFIX:protected]` → recusa "não serve para este item".
- [ ] Clicar na espada da bolsa → **Refino e scrolls** → refino +10, um prefixo e um sufixo de rank A (filtro "a") → a prévia mostra o tooltip antes → depois (ATK maior, "+10", nomes do prefixo e do sufixo) → Aplicar.
- [ ] Entrar no jogo com a Fiona → a espada aparece como "+10 <prefixo> <nome> <sufixo>"; o tooltip do jogo mostra o mesmo ATK, rank e linhas da prévia do painel; o brilho de refino aparece ao equipar; equipar e desequipar sem erro.
- [ ] Sair; no painel, mudar só o sufixo para um de rank 7–9 → aplicar → no jogo o item aparece como intransferível (como um enchant real desse rank).
- [ ] Repetir com uma peça de armadura de placas (ex.: elmo): os scrolls oferecidos respeitam a restrição (`(HEAVY_ARMOR,PLATE_ARMOR)&(HELM,PANTS)` aparece; os de arma não).
- [ ] Lojas → uma loja → **Adicionar linha** → **Catálogo** para o item e para a moeda; Modelos 3D → **Escolher no catálogo**; Editor de itens → seletores de categoria e **Catálogo** do item-modelo.
Limites conhecidos: encantamentos temporários (Duration) não aparecem; a escolha do `Arg` só existe para scrolls com faixa; a durabilidade não muda.

## Como relatar
Para cada linha marcada: ID da entrada e número da linha (ex.: `B4.3b-2`), o que foi feito, o que apareceu na tela (texto da mensagem do painel ou do jogo; captura se possível) e o horário aproximado (hh:mm). Os horários são comparados com `docker compose logs game` e com a Auditoria do painel. Linhas não testadas ficam desmarcadas com o motivo (ex.: "missão não alcançada").

### B4.2 — Contas no painel (cartões, modal, cadastro, conta própria)
Pré-condição: containers no ar.
- [ ] Abrir `http://127.0.0.1:27090`, aba Entrar, `admin` e a senha → abre em Contas, com barra lateral (Administração: Contas, Auditoria; Conta: Minha conta) e "admin · Admin" no topo.
- [ ] Contas mostra cartões (avatar, login, UID e personagens, crachás de papel e situação); o cartão `admin` diz "jogo: lab169_a", "UID 952894723 · 1 personagem(ns)".
- [ ] Clicar num cartão → modal com os dados (papel, situação, criada, nome no jogo, UID, personagens) e os botões Papel, Senha e Desativar/Ativar; cada botão abre o formulário; Voltar retorna aos dados.
- [ ] Botão de tema (sol/lua) alterna claro/escuro e a escolha continua ao recarregar.
- [ ] Sair → aba "Criar conta": criar `teste_b4` (senha de 12+ caracteres, repetida) → entra direto em Minha conta, com o papel "Jogador" e só o item "Minha conta" na barra lateral; abrir `http://127.0.0.1:27090/contas` → volta para Minha conta.
- [ ] Minha conta → Trocar senha (senha atual errada → recusa; certa → volta para o login com "Senha trocada"); entrar com a senha nova.
- [ ] Entrar de novo como `admin` → Auditoria em cartões: o cadastro de `teste_b4`, a troca de senha ("troca pelo próprio usuário") e as entradas no painel, com quem, quando, IP e antes → depois.
Limites conhecidos: personagens e itens da conta ainda não aparecem (B4.3); entrar no jogo com a conta depende do diagnóstico abaixo.

### B4.2-D — Diagnóstico da senha da tela de login (feito em 26/10/2026)
Resultado: com ID `admin` e senha em branco ou `admin`, a mensagem de login chegou idêntica (passport de 5 caracteres; NexonID 0, HwID 0, MachineID 16, UpToDateInfo 0). O cliente não envia a senha. Decisão do operador: login sem senha no jogo.

### B4.2-L — Entrar no jogo com o login da conta
Pré-condição: containers recompilados depois de 26/10/2026; cliente fechado.
- [x] Abrir o cliente pelo `start.bat` da `.cache/play-client`, Start Game, digitar `admin` no ID (senha em branco ou qualquer uma) → pede a segunda senha de sempre e lista a Fiona Murael; entrar na cidade → personagem, inventário e ouro como antes.
- [x] Sair e tentar `lab169_a` no ID → recusado ("Premium Open"): o nome pertence à conta `admin`.
- [x] No painel, criar ou usar `teste_b4`, entrar no jogo digitando `teste_b4` → lobby vazio; criar uma Fiona; no painel, Minha conta de `teste_b4` mostra a personagem.
- [x] Desativar `teste_b4` no painel (como `admin`) e tentar entrar no jogo com `teste_b4` → recusado.
Resultado do operador (26/10/2026): as quatro linhas passaram (admin entrou na Murael; lab169_a recusado; teste_b4 criado, entrou e criou personagem; desativado não entrou).
Limites conhecidos: a senha da conta não protege o jogo (o cliente não a envia); a proteção do jogo é a segunda senha. Um launcher futuro pode mudar isso.

### B4.3a — Personagens no painel (só leitura)
Pré-condição: containers recompilados com `item-names.json` exportado (`py -3.13 scripts/content/export_item_names.py --output .cache/private-content/item-names.json`).
- [ ] Como `admin`, Contas → cartão `admin` → o modal mostra "Personagens" com o cartão da Murael (Fiona · nível 20).
- [ ] Clicar na Murael → ficha com classe, nível, ouro igual ao do jogo, EXP, AP, atributos e tempo de jogo.
- [ ] Aba Inventário → itens agrupados por Bolsa 1, Bolsa 2…, Cash, Conta (ouro), com nomes em inglês iguais aos do tooltip do jogo; o equipamento vestido tem o crachá "vestido"; um item melhorado/encantado mostra "atributo(s)" e, no modal, `classe[ENHANCE:…]`.
- [ ] Filtrar por "sword" (ou outra parte do nome) → só os itens que batem.
- [ ] Abas Skills (nomes em inglês e rank), Efeitos (VIP/termas, se houver, com validade) e Títulos (IDs, o selecionado marcado).
- [ ] Como `teste_b4` (sem papel): Minha conta → "Meus personagens" mostra só os dele; abrir no navegador `http://127.0.0.1:27090/personagens/<CID da Murael>` → "Não encontrado".
Limites conhecidos: somente leitura (dar/remover itens é o B4.3b); títulos aparecem pelo ID; nomes coreanos não traduzidos no cliente EU aparecem pela classe.

### B4.3b — Dar e remover itens, ouro e cash pelo painel
Pré-condição: containers recompilados depois de 26/10/2026; a Murael **fora do jogo** (no lobby ou com o cliente fechado) para itens e ouro.
- [x] Como `admin`, ficha da Murael → Ouro → Dar 10000 com motivo → o ouro do cabeçalho sobe 10.000; Auditoria mostra "Item dado ao personagem" com antes → depois.
- [x] Dar item → buscar "elixir" (ou outro nome) no catálogo, escolher, quantidade 2, motivo → aparece no inventário da ficha.
- [ ] Dar item com atributo, digitando `enchant_scroll[PREFIX:bitter]` → aparece com o crachá "1 atributo(s)".
- [ ] Abrir um item no inventário → Remover → quantidade 1 → some (ou diminui) na ficha.
- [ ] Ouro → Remover mais do que existe → "O personagem não tem essa quantidade do item."; nada muda.
- [ ] Entrar no jogo com `admin` e conferir o ouro, o elixir e o pergaminho no inventário; com a Murael dentro do jogo, tentar dar ouro no painel → "O personagem está no jogo…".
- [ ] Cash → Creditar 1000 (pode ser com a Murael no jogo) → abrir o Depot no jogo → saldo maior; Debitar mais do que o saldo → "Saldo insuficiente".
- [ ] Como `teste_b4` (sem papel): a ficha dos próprios personagens não mostra os botões Dar item/Ouro/Cash/Remover.
Resultado do operador (26/10/2026): dar 2 itens e dar ouro pelo painel funcionaram; as demais linhas não foram relatadas.
Limites conhecidos: itens e ouro só com o personagem fora do jogo; remover tira a quantidade da classe na ordem do jogo (DestroyItem), não um item escolhido; itens com atributos ou que expiram podem ser recusados na remoção; só Fiona de laboratório inicializada é editável; mover itens entre bolsas ainda não existe.

### B4.3c — Progressão pelo painel (nível/EXP, AP, título, efeitos)
Pré-condição: containers recompilados depois de 26/10/2026; a Murael **fora do jogo**.
- [x] Como `admin`, ficha da Murael → Progressão → Nível → subir até o nível atual + 1, com motivo → a ficha mostra o nível novo; Auditoria mostra "Progressão do personagem" com nível/EXP antes → depois.
- [ ] Progressão → AP → dar 10 → o AP da ficha sobe (ou o painel avisa que o jogo não mudou nada, se já estiver no limite).
- [ ] Progressão → Efeito → buscar "vip" → escolher um VIP/VVIP → duração 7 (dias) → aparece na aba Efeitos com a validade.
- [ ] Aba Efeitos → Remover o efeito → some.
- [ ] Progressão → Título → um ID conhecido (aba Títulos) ainda não conquistado → aparece como conquistado.
- [x] Entrar no jogo com `admin` → nível novo na seleção e na cidade; histórias do nível novo no diário; AP, título e efeito (ícone VIP) presentes.
- [ ] Com a Murael dentro do jogo, tentar Progressão → "O personagem está no jogo…".
Resultado do operador (26/10/2026): subir até o nível 20 e dar 50.000 de EXP pelo painel funcionaram e apareceram no jogo; AP, efeito, título e recusa online não relatados.
Limites conhecidos: só com a personagem fora do jogo; skills não mudam pelo painel ainda; títulos pelo ID (sem nome).

### B4.4 — Lojas de NPC e cash shop pelo painel
Pré-condição: containers recompilados depois de 26/10/2026; binário nativo compilado (`cargo build --locked`) para o script do host; cliente fechado na hora de aplicar.
- [ ] Como `admin`, Lojas de NPC → filtrar "ferghus" → abrir a loja → as linhas aparecem com o nome em inglês e o preço que o jogo cobra (Enhancement Elixir por 4.000).
- [ ] Clicar no Enhancement Elixir → mudar o preço para 3.000 → Conteúdo mostra 1 pendente.
- [ ] Na mesma loja, Adicionar linha → buscar um item (ex.: "potion") → preço 100 → Conteúdo mostra 2 pendentes.
- [ ] Cash shop → escolher uma categoria (ex.: HAIR) → preço novo → 3 pendentes.
- [ ] Conteúdo → Publicar versão → v1 "a aplicar".
- [ ] No host: `pwsh scripts/content/apply-content-edits.ps1` → termina com "Content edits applied up to version 1"; Conteúdo mostra v1 "aplicada".
- [ ] Abrir o cliente, entrar com `admin`, ir ao Ferghus → Enhancement Elixir por 3.000 e o item novo por 100; comprar um de cada → o ouro cai certo e os itens entram no inventário.
- [ ] Abrir o Depot → a categoria editada com o preço novo.
- [ ] (Desfazer) Clicar no item novo → Remover esta linha → publicar → aplicar de novo → o item some da loja do Ferghus.
Limites conhecidos: edições só chegavam ao jogo pelo script do host (substituído pelo fluxo automático do B4.4c, que continua aceitando o script como alternativa manual); só lojas existentes; linhas com limite de compra aparecem "limitado" e continuam fora do que o laboratório vende; os produtos VIP por duração seguem a própria regra de preço.

### FX-relist — Voltar à seleção de personagens e entrar de novo
Pré-condição: containers recompilados depois de 26/10/2026.
- [x] Entrar com `admin`, entrar na cidade com a Murael, voltar à tela de seleção (Home) → a lista aparece.
- [x] Selecionar a Murael e apertar Start → entra na cidade de novo, sem precisar de Go back e novo login.
- [x] Repetir uma segunda vez → também entra.
Resultado do operador (26/10/2026): testado e resolvido (voltar à seleção e entrar de novo funcionou).
Limites conhecidos: nenhum.

### B4.4b — Lojas por local, NPC, ícones e abas do inventário
Pré-condição: containers recompilados depois de 26/10/2026 com `.cache/private-content/panel-catalog.json` e `icons/` exportados (`py -3.13 scripts/content/export_panel_catalog.py`). Só painel; cliente: não necessário.
- [ ] Lojas de NPC → abas por local (Colhen, Rocheste, Malina, Campos, Navios, Eventos…) com a contagem; cada cartão mostra o nome do NPC, a loja e quantas linhas estão à venda.
- [ ] Buscar "ferghus" → abrir → título "Ferghus", subtítulo com a loja e o local; linhas com ícone e nome oficial; busca dentro da loja filtra por nome ou classe.
- [ ] As linhas NotSell/TestServer/eventos de outras regiões ficam ocultas; marcar "Mostrar N linha(s)…" → aparecem esmaecidas com "fora da região".
- [ ] Ficha da Murael → Inventário → abas Bolsa 1…, Quest, Cash… com a contagem; buscar um item → as abas com resultado ficam destacadas com `n/total` e a lista diz em quais abas está.
- [ ] Dar item → a busca do catálogo mostra os ícones.
Limites conhecidos: reordenar e arrastar itens pelo painel ainda não existem; ícones só de itens (não de skills/efeitos).

### B4.4c — Publicar com manutenção agendada e aplicação automática
Pré-condição: containers recompilados depois de 26/10/2026; abrir o cliente pelo `.cache/play-client/start-lab.bat` (atualiza a cópia do cliente com a versão publicada e abre o jogo como o `start.bat`).
- [ ] Entrar no jogo com `admin` e ficar na cidade com a Murael.
- [ ] No painel, Lojas de NPC → Ferghus → mudar o preço de um item (ex.: Enhancement Elixir) → Conteúdo → Publicar versão → o modal pede os minutos (5 por padrão); colocar 1 e publicar.
- [ ] Conteúdo mostra "Manutenção agendada: reinício em 0:59…" com a contagem; Auditoria registra a publicação com `restart_minutes`.
- [ ] No jogo: aparece o aviso de desligamento do servidor (60 s) e depois 45, 30, 15, 10 e 5 s (o texto é do próprio cliente; anotar como aparece).
- [ ] No fim do prazo o jogo desconecta (anotar a mensagem). O painel fica fora por alguns segundos e volta; a versão aparece "aplicada".
- [ ] Fechar o cliente, abrir pelo `start-lab.bat`, entrar → o Ferghus vende pelo preço novo (no diálogo da loja e na cobrança).
- [ ] (Cancelar) Publicar outra edição com 5 minutos → Cancelar manutenção → a contagem some, ninguém é desconectado; a versão fica "aplicada no próximo reinício".
- [ ] (Imediato) Publicar com 0 minutos → aviso de 0 s e desconexão imediata; o servidor volta com a versão.
Limites conhecidos: o aviso é só o NoticeShutDownMessage do original (sem anúncio de texto, que o original só mandava em esperas de 10 minutos ou mais); a cópia do cliente só muda ao abrir pelo `start-lab.bat` (o `start.bat` original abre sem a atualização); cancelar não desfaz a versão publicada.

### B4.4d — Organizar e arrastar itens no inventário pelo painel
Pré-condição: containers recompilados depois de 26/10/2026; a Murael fora do jogo (na tela de seleção ou com o cliente fechado).
- [ ] Ficha da Murael → Inventário → aba Bolsa 1: grade de 48 espaços com ícones e quantidades, como no jogo.
- [ ] Arrastar um item para um espaço vazio → ele vai para lá; Auditoria registra "Item movido no inventário".
- [ ] Arrastar um item sobre outro diferente → trocam de lugar.
- [ ] Arrastar uma pilha sobre outra do mesmo item (ex.: duas pilhas de poção) → juntam até o limite da pilha.
- [ ] Arrastar um item para a aba de outra bolsa (se houver) → vai para o primeiro espaço livre dela.
- [ ] Organizar → Todas as bolsas → "N item(ns) mudaram de lugar"; de novo → "O inventário já está organizado.".
- [ ] Entrar no jogo com a Murael → os itens estão nos mesmos espaços que o painel mostrou.
- [ ] Com a Murael dentro do jogo, arrastar ou Organizar → "O personagem está no jogo…" e nada muda.
Limites conhecidos: itens vestidos ocupam espaços da bolsa (ícone de camisa), não são arrastados e não recebem itens soltos; só bolsas (Cash e Quest só pelo Organizar).

### B4.4e — Ícone com a cor do item e tooltip como no jogo
Pré-condição: containers recompilados depois de 26/10/2026 com `py -3.13 scripts/content/export_panel_catalog.py --output .cache/private-content/panel-catalog.json` (catálogo com os dados do tooltip e os ícones de fundo). Só painel; para comparar, abrir o mesmo item no jogo.
- [ ] Ficha da Murael → Inventário: a espada, o escudo e as peças de armadura aparecem com as cores que têm no jogo (não o vermelho/verde/azul da máscara).
- [ ] O inner armor (`fiona_noluck_inner_devilhunter`) mostra o nome e a descrição em inglês ("Crescent Moon Inner Armor") e o ícone sobre o corpo (IconBG).
- [ ] Passar o mouse sobre a espada → caixa com ícone, nome com +refino/encantamentos, categoria e raridade, ATT e demais estatísticas, encantamentos com as linhas ativas, durabilidade, requisitos (nível, classe, skill; em vermelho os não atendidos), descrição, conjunto, cores, troca/vínculo, peso e preço de venda. Comparar com o tooltip do jogo e anotar o que difere.
- [ ] Passar o mouse sobre uma poção → nome, categoria "Consumable", descrição completa, quantidade.
- [ ] Clicar num item → o modal mostra a mesma caixa.
Limites conhecidos: a mistura das cores da máscara é uma aproximação do cliente; as cores de raridade são aproximadas; o efeito de uso de consumíveis aparece só pela descrição do item; durabilidade mostrada como valor do jogo / 100.

### B4.5 — Taxas, drops por fase e chances de Enhance/Enchant pelo painel
Pré-condição: containers recompilados depois de 26/10/2026 (migração 0047); cliente aberto pelo `.cache/play-client/start-lab.bat`.
- [ ] Ajustes de jogo → Taxas: as 7 taxas aparecem com o valor carregado (100% = original). Mudar "EXP das missões (combate)" para 200 → Conteúdo mostra 1 pendente "Taxa do FeatureMatrix".
- [ ] Drops por fase: buscar uma fase que você joga (nome em inglês) → mudar mínimo/máximo (ex.: 3–5) → pendente.
- [ ] Enhance: a chance de cada nível (+1…). Mudar o +5 para 100% → pendente.
- [ ] Enchant: buscar um encantamento que você tem (ex.: o nome do pergaminho) → mudar a chance mínima/máxima → pendente.
- [ ] Conteúdo → Publicar versão com 1 minuto → aviso, reinício; Ajustes de jogo mostra os valores novos.
- [ ] Fechar o cliente e abrir pelo `start-lab.bat`. Jogar a fase ajustada: a EXP do resultado dobra e mais monstros deixam drop; Enhance +5 numa peça +4 → sucesso e a janela mostra a chance nova; o minijogo do encantamento mostra a chance nova.
Limites conhecidos: valores fora dos limites são recusados (taxas 0–10000%, drops 0–100, chances 0–100%); o Enhance é por nível (vale para todos os equipamentos naquele nível, como o EnhanceServerInfo do servidor); o cliente só mostra os valores novos depois de abrir pelo `start-lab.bat`.

### B4.6 — Mapas e canais (somente leitura)
Pré-condição: containers recompilados depois de 27/10/2026.
- [ ] Painel → Mapas e canais sem ninguém no jogo → "Ninguém no canal agora", portas do login, canal, relay e loja web, mapas do canal e versão de conteúdo carregada igual à última publicada.
- [ ] Entrar na cidade com um personagem → em até 5 s ele aparece no canal 1 com a conta, o mapa e a partição; "Conexões no jogo" sobe.
- [ ] Entrar numa missão → "Missões em andamento" mostra 1 e o relay mostra o grupo; voltar para a cidade → o grupo some.
- [ ] Sair do jogo → o personagem sai da lista.
Limites conhecidos: o laboratório tem um canal só, então não há controle de canal; a página atualiza a cada 5 segundos.

### B4.5b — Ajustes de jogo editados na própria tela, aplicados sem reiniciar
Pré-condição: containers recompilados depois de 26/10/2026; personagem no jogo, de preferência.
- [ ] Ajustes de jogo → Taxas: mudar "EXP das missões (combate)" para 200 na própria caixa → a caixa fica destacada com "Atual 100% → novo 200%" e aparece a barra "1 alteração(ões)" com Salvar/Descartar.
- [ ] Salvar → aviso verde "Aplicado no jogo agora (versão N), sem reiniciar"; a caixa mostra 200%. Conteúdo → versões mostra a versão N "aplicada"; Auditoria mostra "content.tuning".
- [ ] Sem sair do jogo: terminar uma missão → a EXP do resultado dobra (comparar com antes).
- [ ] Enhance: mudar o +5 para 100% e salvar → aviso diz que o servidor já usa a chance nova e que a janela do cliente mostra o valor depois de abrir pelo `start-lab.bat`. Tentar o +5 sem reabrir o cliente → sucesso.
- [ ] Mudar um mínimo para mais que o máximo (drops ou enchant) → a barra mostra o erro e Salvar fica desabilitado; Descartar volta os valores.
- [ ] Se houver ajustes pendentes do ciclo antigo: aviso no topo com o número de pendentes e, na caixa, "Pendente: 100% → 200%; publique a versão em Conteúdo para valer".
- [ ] Salvar aqui um valor que tinha pendente → a pendente some (retirada) e o valor salvo vale na hora.
Limites conhecidos: só admin salva; recarregar o conteúdo leva ~2 s e a memória do jogo sobe de ~270 MiB para ~750 MiB nas primeiras recargas e estabiliza; mensagens que já estavam em andamento terminam com os valores antigos.

### B4.4f — Sessão estável e mudanças sem motivo
Pré-condição: containers recompilados depois de 26/10/2026 (migração 0048).
- [ ] Entrar no painel, abrir a ficha da Murael e clicar em cada item do menu (Contas, Lojas de NPC, Cash shop, Ajustes de jogo, Conteúdo, Auditoria, Minha conta) → nenhum volta ao login.
- [ ] Deixar o painel parado mais de 30 minutos (a sessão expira), clicar num item do menu → volta ao login; entrar de novo e repetir o clique em outras telas → continua logado.
- [ ] Dar ouro, mudar um preço, organizar o inventário e publicar uma versão → nenhum formulário pede motivo.
- [ ] Auditoria → as entradas mostram quem fez, o quê (antes → depois) e quando, sem motivo. Conteúdo → edições e versões sem motivo.
Limites conhecidos: entradas antigas da auditoria perderam o motivo (a coluna foi removida).

### B4.N1 — Nome livre na criação
Pré-condição: containers recompilados depois do commit do nome livre (27/10/2026); cliente aberto pelo `start.bat` da `.cache/play-client`.
- [ ] Entrar com uma conta que tenha espaço livre (ex.: `teste_b4`), Create, digitar um nome novo só com letras (ex.: `Lucanor`) e clicar Check → diálogo "That name is available and can be used".
- [ ] Digitar o nome de uma personagem existente com outra caixa (ex.: `MURAEL`) e clicar Check → diálogo de nome já em uso.
- [ ] Digitar `A1` (fora da regra: só letras e um `_`) e clicar Check → diálogo de nome inválido.
- [ ] Criar uma Fiona com o nome novo → aparece na lista; sair e entrar de novo → continua lá.
- [ ] Tentar criar outra personagem com o mesmo nome (em qualquer caixa) → "Failed to create a character" com o erro de nome duplicado, sem 0xAA04.
Limites conhecidos: não há mais filtro de nomes proibidos nem lista de nomes reservados medida; só a regra de formato e os nomes já gravados bloqueiam. 

### B4.N2 — Criar personagem de qualquer classe
Pré-condição: `game` recompilado depois do commit da Hagie (27/10/2026). A conta pode já ter personagens (ex.: `admin` com a Murael): a lista mostra todos.
- [ ] Create → escolher **Evy**, nome novo (ex.: `Lucanor`), Check → disponível; Create → a Evy aparece na lista com a roupa interna dela, sem 0xAA04.
- [ ] Entrar no jogo com a Evy → cidade carrega; inventário com os itens iniciais da Evy (arma de mago, não o escudo da Fiona); janela de skills com as skills iniciais da Evy e a skill ativa no slot de SP.
- [ ] Sair, voltar ao lobby → a Evy continua na lista (relogin).
- [ ] Repetir com **Hurk**: criação, cidade, inventário e skills iniciais do Hurk.
- [ ] Create → **Hagie**, nome novo → aparece na lista sem 0xAA04; no jogo, 10000 de ouro na carteira, além dos itens iniciais dela.
- [ ] Opcional: uma classe de cada (Lann, Karok, Kai, Vella, Lynn, Arisha) → cada uma cria e entra na cidade.
Limites conhecidos: títulos de preset e stats por nível usam a classe certa, mas só a Fiona foi exercitada em combate/missão até agora.

### I2 — Editor de itens no painel (criar, clonar, editar, ocultar, remover)
Pré-condição: imagens recompiladas depois do commit do editor de itens (`COMPOSE_PARALLEL_LIMIT=1 docker compose build`, `docker compose up -d --wait`); login no painel com papel `admin` (publicar exige admin; criar e editar, `gm`); uma Fiona de laboratório offline. Para o modelo novo, um kit exportado pelo add-on do Blender com nomes de modelo e materiais novos (ex.: `fiona_inner_lab2.mdl`). Cliente fechado.
- [ ] Painel → **Editor de itens** → buscar `fiona_inner` → a lista mostra os itens base e as adições (`fiona_inner_fringe`, `fiona_inner_zero`, `fiona_inner_lab` com a marca "adição"); escolher um → ficha com campos, quem tem o item e a prévia 3D do modelo.
- [ ] **Novo item** → ItemClass `fiona_inner_ed1`, item-modelo `fiona_inner_fringe_socks` → Carregar → nome "Editor Inner One", DEF 77, só Fiona marcada → Criar item → aviso de edição pendente; a lista (filtro Pendentes) mostra o item "ainda não exportado".
- [ ] Escolher `fiona_inner_lab` → **Clonar** → ItemClass `fiona_inner_ed2`, nome "Editor Inner Two", anexar o kit novo (.zip) → a prévia mostra o modelo do kit → Criar item → resposta com o CostumeType novo (9102 ou o próximo livre).
- [ ] Escolher `fiona_inner_fringe` → **Editar** → DEF 55 e descrição nova → Salvar edição.
- [ ] Escolher um item base qualquer de loja (ex.: o Enhancement Elixir da loja do Ferghus) → **Ocultar** → o aviso diz que os itens dos personagens continuam funcionando.
- [ ] Escolher `fiona_inner_zero` → a prévia 3D mostra o modelo do inner_zero (antes: "modelo não encontrado").
- [ ] Escolher uma espada (ex.: `longsword_swordwind`) e um escudo (ex.: `largeshield_kite`) → a prévia 3D mostra o modelo; em Modelos 3D, "Exportar kit" baixa o zip; importar no Blender (File → Import → Vindictus Kit), exportar com nome novo (ex.: `models/weapon/longsword/longsword_lab.mdl`) → o zip exportado é aceito ao **Clonar** a espada com o kit (CostumeType seguinte ao último das espadas, 10007 ou mais).
- [ ] **Novo item** para a Evy: item-modelo um inner da Evy (ex.: um `evy_inner_*` base), só Evy marcada, kit `evy_inner_zero.zip` (pasta `inner_zero_kits` da Área de Trabalho), corpo female → Criar item.
- [ ] Dar à Fiona um item adicionado de teste (ex.: `fiona_inner_ed1`), vesti-lo, sair do jogo; no painel, **Remover** esse item → a confirmação avisa que ele será desequipado e apagado de quem o tem.
- [ ] Conteúdo → **Publicar** (0 minutos) → o jogo reinicia; o Editor de itens mostra o aviso "o servidor exportou até ..." enquanto a reexportação não rodar.
- [ ] Fechar o cliente e abrir pelo `start-lab.bat` → ele reexporta os itens do servidor e reinicia o `game` (1 a 3 minutos; log em `start-lab-items.log`), depois atualiza a cópia do cliente → o aviso some do Editor de itens.
- [ ] Painel → dar `fiona_inner_ed1`, `fiona_inner_ed2` e `fiona_inner_fringe` à Fiona → no jogo, cada um aparece na bolsa com o nome em inglês e o ícone do modelo; o tooltip mostra a DEF editada (77, a do clone e 55); vestir: `ed1` com o modelo do `fiona_inner_fringe_socks`, `ed2` com o modelo do kit.
- [ ] Loja do Ferghus → o item ocultado não aparece mais; um que a Fiona já tinha continua na bolsa e funciona.
- [ ] Relogar → os itens novos continuam na bolsa e vestidos, com nome, ícone, modelo e estatísticas; a espada clonada aparece na mão com o modelo do kit; a Evy veste o inner novo.
- [ ] O item removido sumiu da bolsa e do corpo da Fiona, sem erro ao entrar; o log do `game` registra "removed item taken from its owners".
- [ ] Painel → `fiona_inner_fringe` → **Restaurar original** → publicar → `start-lab.bat` → o tooltip volta à DEF original.
Limites conhecidos: o ícone é escolhido entre os ícones existentes (ícone novo depende do launcher); um item base não muda de conjunto nem ganha modelo novo (só pode apontar outro CostumeType existente); a remoção apaga o item de todos os personagens quando o jogo carrega a publicação (não confere lojas, receitas e drops); a distribuição a outros jogadores é o launcher (fora deste roteiro).

## Como relatar
Para cada linha marcada: ID da entrada e número da linha (ex.: `B4.2-4`), o que foi feito, o que apareceu na tela (texto do diálogo/mensagem; captura se possível) e o horário aproximado (hh:mm), para comparar com `docker compose logs game` e a tela Auditoria do painel. Linhas não testadas ficam desmarcadas com o motivo.
