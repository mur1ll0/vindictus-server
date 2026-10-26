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
- [ ] Como `admin`, ficha da Murael → Ouro → Dar 10000 com motivo → o ouro do cabeçalho sobe 10.000; Auditoria mostra "Item dado ao personagem" com antes → depois.
- [ ] Dar item → buscar "elixir" (ou outro nome) no catálogo, escolher, quantidade 2, motivo → aparece no inventário da ficha.
- [ ] Dar item com atributo, digitando `enchant_scroll[PREFIX:bitter]` → aparece com o crachá "1 atributo(s)".
- [ ] Abrir um item no inventário → Remover → quantidade 1 → some (ou diminui) na ficha.
- [ ] Ouro → Remover mais do que existe → "O personagem não tem essa quantidade do item."; nada muda.
- [ ] Entrar no jogo com `admin` e conferir o ouro, o elixir e o pergaminho no inventário; com a Murael dentro do jogo, tentar dar ouro no painel → "O personagem está no jogo…".
- [ ] Cash → Creditar 1000 (pode ser com a Murael no jogo) → abrir o Depot no jogo → saldo maior; Debitar mais do que o saldo → "Saldo insuficiente".
- [ ] Como `teste_b4` (sem papel): a ficha dos próprios personagens não mostra os botões Dar item/Ouro/Cash/Remover.
Limites conhecidos: itens e ouro só com o personagem fora do jogo; remover tira a quantidade da classe na ordem do jogo (DestroyItem), não um item escolhido; itens com atributos ou que expiram podem ser recusados na remoção; só Fiona de laboratório inicializada é editável; mover itens entre bolsas ainda não existe.

## Como relatar
Para cada linha marcada: ID da entrada e número da linha (ex.: `B4.2-4`), o que foi feito, o que apareceu na tela (texto do diálogo/mensagem; captura se possível) e o horário aproximado (hh:mm), para comparar com `docker compose logs game` e a tela Auditoria do painel. Linhas não testadas ficam desmarcadas com o motivo.
