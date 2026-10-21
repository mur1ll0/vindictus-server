# Roteiro de testes no cliente — B1 cidade

Bloco B1 (dinheiro, NPCs, lojas e quests). A preparação, a ordem de gameplay e o formato de relato são completados no gate do bloco.

### B1.1 — carteira de ouro (gold na AccountStorage) e GiveItem geral
Pré-condição: Fiona deslogada (fora da cidade e da seleção). Com o binário compilado (`cargo build --locked`), conceder ouro pelo comando de laboratório:
`pwsh scripts/dev/lab-give-item.ps1 -ItemClass gold -Count 10000`
(o comando recusa se a Fiona estiver conectada; `-Cid` só é necessário se houver mais de uma Fiona inicializada).
- [ ] Entrar na cidade e abrir o inventário → o saldo mostra 10.000 de ouro.
- [ ] Repetir o comando com `-Count 500` (deslogada) e entrar de novo → saldo 10.500 (somado ao mesmo item, sem segundo item de ouro).
- [ ] Relogar → saldo preservado.
Limites conhecidos: o envio por correio do que não couber (sobra do GiveItem) não foi migrado; nesse caso a entrega inteira é recusada sem alterar nada. Classes com validade (ExpireIn), TirCoin e aba Story são recusadas.
