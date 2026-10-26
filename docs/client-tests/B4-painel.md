# Roteiro de testes no cliente — B4 painel de administração

Bloco B4 (painel de administração e análise de extensões). Plano em `docs/b4-painel-plano.md`. Entradas acrescentadas a cada passo com efeito no cliente; o topo é completado no gate do B4.

## Preparação
1. Runtime no Docker recompilado (`docker compose ps` mostra `game` e `server` saudáveis). Se o código mudou depois disso: `COMPOSE_PARALLEL_LIMIT=1 docker compose build` e `docker compose up -d --wait` (o serviço `migrate` aplica as migrações).
2. Painel: `http://127.0.0.1:27090` (só no próprio computador).
3. Conta de administração criada a pedido do operador: login `admin`, papel admin, ligada ao usuário de jogo `lab169_a` (Fiona Murael, nível 20).

### B4.2 — Contas no painel (cartões, modal, cadastro, conta própria)
Pré-condição: containers no ar.
- [ ] Abrir `http://127.0.0.1:27090`, aba Entrar, `admin` e a senha → abre em Contas, com barra lateral (Administração: Contas, Auditoria; Conta: Minha conta) e "admin · Admin" no topo.
- [ ] Contas mostra cartões (avatar, login, UID e personagens, crachás de papel e situação); o cartão `admin` diz "jogo: lab169_a", "UID 952894723 · 1 personagem(ns)".
- [ ] Clicar num cartão → modal com os dados (papel, situação, criada, nome no jogo, UID, personagens) e os botões Papel, Senha e Desativar/Ativar; cada botão abre o formulário com motivo; Voltar retorna aos dados.
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
- [ ] Clicar no Enhancement Elixir → mudar o preço para 3.000 com motivo → Conteúdo mostra 1 pendente.
- [ ] Na mesma loja, Adicionar linha → buscar um item (ex.: "potion") → preço 100 → Conteúdo mostra 2 pendentes.
- [ ] Cash shop → escolher uma categoria (ex.: HAIR) → preço novo → 3 pendentes.
- [ ] Conteúdo → Publicar versão (motivo) → v1 "a aplicar".
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
- [ ] Organizar → Todas as bolsas → motivo → "N item(ns) mudaram de lugar"; de novo → "O inventário já está organizado.".
- [ ] Entrar no jogo com a Murael → os itens estão nos mesmos espaços que o painel mostrou.
- [ ] Com a Murael dentro do jogo, arrastar ou Organizar → "O personagem está no jogo…" e nada muda.
Limites conhecidos: itens vestidos ocupam espaços da bolsa (ícone de camisa), não são arrastados e não recebem itens soltos; só bolsas (Cash e Quest só pelo Organizar); arrastar usa o motivo fixo "Movido arrastando no painel".

### B4.4e — Ícone com a cor do item e tooltip como no jogo
Pré-condição: containers recompilados depois de 26/10/2026 com `py -3.13 scripts/content/export_panel_catalog.py --output .cache/private-content/panel-catalog.json` (catálogo com os dados do tooltip e os ícones de fundo). Só painel; para comparar, abrir o mesmo item no jogo.
- [ ] Ficha da Murael → Inventário: a espada, o escudo e as peças de armadura aparecem com as cores que têm no jogo (não o vermelho/verde/azul da máscara).
- [ ] O inner armor (`fiona_noluck_inner_devilhunter`) mostra o nome e a descrição em inglês ("Crescent Moon Inner Armor") e o ícone sobre o corpo (IconBG).
- [ ] Passar o mouse sobre a espada → caixa com ícone, nome com +refino/encantamentos, categoria e raridade, ATT e demais estatísticas, encantamentos com as linhas ativas, durabilidade, requisitos (nível, classe, skill; em vermelho os não atendidos), descrição, conjunto, cores, troca/vínculo, peso e preço de venda. Comparar com o tooltip do jogo e anotar o que difere.
- [ ] Passar o mouse sobre uma poção → nome, categoria "Consumable", descrição completa, quantidade.
- [ ] Clicar num item → o modal mostra a mesma caixa.
Limites conhecidos: a mistura das cores da máscara é uma aproximação do cliente; as cores de raridade são aproximadas; o efeito de uso de consumíveis aparece só pela descrição do item; durabilidade mostrada como valor do jogo / 100.

## Como relatar
Para cada linha marcada: ID da entrada e número da linha (ex.: `B4.2-4`), o que foi feito, o que apareceu na tela (texto do diálogo/mensagem; captura se possível) e o horário aproximado (hh:mm), para comparar com `docker compose logs game` e a tela Auditoria do painel. Linhas não testadas ficam desmarcadas com o motivo.
