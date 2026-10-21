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

### B1.2 — aprender skill nova pagando ouro (LearnNewSkill)
Pré-condição: Fiona com ouro (entrada B1.1; por exemplo `-Count 50000`). Runtime iniciado com os assets privados atuais (perfil de skills v4 com preços).
- [ ] Na tela de habilidades (ou no instrutor, se o cliente só oferecer a compra por NPC), escolher uma skill nova da Fiona com preço em ouro e confirmar → a skill aparece no rank 1, o ouro diminui exatamente no preço e o cliente mostra o resultado de sucesso.
- [ ] Tentar comprar uma skill com ouro insuficiente (ou com nível insuficiente) → mensagem de falha correspondente (ouro insuficiente / nível) e nenhum ouro descontado.
- [ ] Se a skill for ativa, ela ocupa o primeiro slot SP vazio.
- [ ] Relogar → a skill continua aprendida e o saldo de ouro preservado.
Limites conhecidos: skill que exige desbloqueio por história no rank 2 é recusada (serviço de histórias não migrado; nenhuma skill da Fiona está nesse caso); efeitos de atributo da skill nova aparecem após relogar (UpdateStat imediato não é enviado, como no treino do B0); log de personagem (AddSkillLog) não migrado.
