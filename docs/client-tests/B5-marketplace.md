# Roteiro de testes no cliente — B5 mercado

Mercado (leilão entre jogadores) do servidor Rust (`docs/marketplace.md`): registrar à venda com taxa, buscar por categoria e por nome com filtros e ordenação, comprar total ou parcial, cancelar, expirar e receber pelo correio (item e ouro). Precisa de **duas personagens em duas contas** (o ouro é da conta: personagens da mesma conta compartilham o ouro): **A** vende (conta `lab169_a`), **B** compra (conta `lab169_b`). Pode ser um cliente de cada vez (A sai, B entra) ou dois clientes.

## Preparação
1. Runtime com o mercado: depois do commit, `COMPOSE_PARALLEL_LIMIT=1 docker compose build` e `docker compose up -d --wait`; `docker compose ps` mostra `game` e `server` saudáveis. A migração `0055_marketplace` cria as tabelas na subida.
2. Conteúdo: `.cache/private-content/item-economy.json` tem a seção `marketplace` e `trade_category` nas classes, gerados por `scripts/content/export_item_economy.py` (já reexportado neste passo).
3. Cliente: `.cache/play-client/start-lab.bat`; contas `lab169_a` e `lab169_b`.
4. Itens e ouro, com a personagem **fora do jogo** (PowerShell 7, na pasta do projeto; o `cid` está na ficha do painel):
   - A: `./scripts/dev/lab-give-item.ps1 -ItemClass gold -Count 20000 -Cid <cidA>`
   - A: `./scripts/dev/lab-give-item.ps1 -ItemClass life_potion_lvl1 -Count 10 -Cid <cidA>` (limite de preço 1000) e `-ItemClass metal_solvent_lvl1 -Count 5` (limite 600)
   - A: um equipamento sem refino (`-ItemClass <equipamento> -Count 1`) e, para MK2, um item com validade e um `dye_` se houver.
   - B: `./scripts/dev/lab-give-item.ps1 -ItemClass gold -Count 5000 -Cid <cidB>`
5. Para MK8 (expirar sem esperar a duração): com A fora do jogo, `docker compose exec postgres psql -U <POSTGRES_USER> -d <POSTGRES_DB> -c "UPDATE game_trade_items SET close_ticks=open_ticks WHERE cid=<cidA>"` (usuário e banco do `.env`).
6. Relatórios de queda, se houver: `Documents\Vindictus EU GM\Report Error`.

## Ordem de gameplay
1. **MK1** (A): abrir o mercado.
2. **MK2** (A): registrar e ver as recusas.
3. **MK3** (A, depois B): meus registros; buscas.
4. **MK4** (B): comprar parcial, esgotado, ouro insuficiente.
5. **MK5** (B): comprar com o inventário cheio.
6. **MK6** (A): receber o ouro das vendas.
7. **MK7** (A): cancelar.
8. **MK8** (A): expirar.
9. **MK9** (A): limite de 20 registros.
10. **MK10**: relogar as duas e conferir.

## Como relatar
Para cada linha marcada: ID e número da linha (ex.: `MK4-2`), o que apareceu na tela (texto, captura se possível), ouro antes e depois quando a linha falar de taxa ou preço, se o cliente fechou ou travou, o horário aproximado (para comparar com `docker compose logs game` e `.cache/verification`) e o nome do relatório de queda, se houver.

### MK1 — Abrir o mercado
Pré-condição: preparação feita; A entra no jogo na cidade.
- [ ] Abrir o mercado (botão do NPC ou menu) → a tela abre (`SecuredOperation` OpenTradeDialog); abas de busca e de registro.
- [ ] Fechar a tela → nada aparece (sem vendas pendentes).

### MK2 — Registrar à venda
Pré-condição: MK1; A com as poções, o solvente e o ouro da preparação.
- [ ] Registrar 5 `life_potion_lvl1` a 100 cada por 3 dias → sucesso; as 5 saem do inventário e o ouro cai **45** (3% × 100 × 5 × 3 dias).
- [ ] Registrar 2 `metal_solvent_lvl1` a 600 por 1 dia → sucesso; o ouro cai **36**.
- [ ] Registrar com duração de menos de um dia (se a tela oferecer) → taxa **0**.
- [ ] Preço acima do limite (`life_potion_lvl1` a 1001, ou acima de 2× o preço de loja) → erro de preço fora do limite (PriceOutOfRange); nada muda.
- [ ] Item vinculado, de cash ou com restrição de troca → erro (InternalError/NotTradable); nada muda.
- [ ] Equipamento com quantidade 2 → erro; equipamento com quantidade 1 → sucesso.
- [ ] Item com validade → erro de item por tempo (TimeLimitItem).
- [ ] `dye_…` com A abaixo do nível 11 → erro de nível (NotEnoughLevel).
- [ ] Registro premium sem `pre_trade_ticket` → erro (NoPremiumTicket).
Limites conhecidos: Tir coin não migrado (InternalError_Tir); peças de Combine Craft não são registradas.

### MK3 — Meus registros e buscas
Pré-condição: MK2.
- [ ] A: aba de meus registros → as poções, o solvente e o equipamento, com o tempo restante.
- [ ] B (outro cliente ou depois de A sair): busca por categoria GOODS → POTION → as poções de A; nível mínimo/máximo filtram.
- [ ] Ordenar por preço crescente e decrescente; por nível → a ordem muda (premium primeiro, se houver).
- [ ] Busca por nome do item (lista de classes) → o registro aparece.
- [ ] Filtro de preço máximo abaixo de 100 → lista vazia; acima → aparece.
- [ ] Muitas buscas seguidas (mais de 20 em 10 s) → erro de excesso de buscas (SearchOverhead).

### MK4 — Comprar
Pré-condição: MK3; B com 5000 de ouro.
- [ ] B compra 2 das 5 poções → sucesso; o ouro de B cai **200**; 2 poções entram no inventário de B (vinculadas: não podem ser registradas de novo); o registro mostra 3 restantes.
- [ ] B tenta comprar 4 → esgotado (AlreadySoldOut) com 3 restantes; o ouro não muda.
- [ ] B tenta comprar o equipamento com ouro insuficiente (preço acima do saldo) → ouro insuficiente (NotEnoughMoney); nada muda.

### MK5 — Comprar com inventário cheio
Pré-condição: MK4; encher as bolsas de B.
- [ ] B compra 1 poção → sucesso; mensagem de console de compra pelo correio; o correio de B recebe "Buy" do remetente AUCTION com a poção; retirar quando houver espaço.

### MK6 — Receber o ouro das vendas
Pré-condição: MK4/MK5 feitos; A sai e entra de novo (ou fecha a tela do mercado).
- [ ] Ao entrar (ou ao fechar o mercado) → mensagem de console com o número de vendas; o correio de A recebe uma carta "Sold" (remetente AUCTION) por compra, com o ouro **sem comissão** (200 pela compra de 2).
- [ ] Retirar o ouro → o ouro de A sobe o valor da carta.

### MK7 — Cancelar
Pré-condição: MK6; A com o solvente ainda registrado.
- [ ] Cancelar o registro do solvente → sucesso; as 2 voltam ao inventário (sem vínculo); mensagem de console de cancelamento; some de meus registros.
- [ ] Com o inventário cheio, cancelar outro registro → o item vai ao correio ("SellCanceled").
- [ ] A taxa do registro não é devolvida.

### MK8 — Expirar
Pré-condição: MK7; um registro de A ainda aberto; A fora do jogo; comando 5 da preparação.
- [ ] A entra → mensagem de console de registros expirados; o correio recebe "SellExpired" com o item; meus registros vazio para esse item; B não o encontra mais na busca por categoria.

### MK9 — Limite de registros
Pré-condição: A com 20 itens simples (por exemplo 20 poções avulsas).
- [ ] Registrar 20 → todos com sucesso; o 21º → erro (WebError) e o item fica no inventário.

### MK10 — Relogar
- [ ] A e B saem e entram → ouro, itens, registros abertos e cartas do correio preservados; nada duplicado nem perdido.
