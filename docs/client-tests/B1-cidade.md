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

### B1.3b — lista de NPCs e histórias abertas no login
Pré-condição: runtime com os assets privados atuais (`npc_scripts` = `.cache/private-content/npc-scripts.json`). A Fiona ainda sem progresso de histórias (primeiro login após este passo).
- [ ] Entrar na cidade → sem travar no carregamento; os NPCs aparecem e o cliente recebe a lista de NPCs (o servidor responde à QueryNpcList).
- [ ] Abrir o diário/lista de histórias (se o cliente mostrar) → a história principal inicial (GuidePlay_1_SkillTrain) aparece como acompanhada.
- [ ] Observar NPCs com marcador de história (ex.: o NPC da GuidePlay) → marcador visível.
- [ ] Relogar → as mesmas histórias e marcadores; nada duplicado.
Limites conhecidos: clicar no NPC ainda não abre diálogo (B1.3c); logs de história (StoryTelling) não migrados; FlexibleMapLoading não muda o mapa para as histórias do nível 1 (não há entrada para elas).

### B1.3c — conversar com NPCs e navegar nos menus
Pré-condição: a mesma do B1.3b.
- [ ] Clicar num NPC da cidade (ex.: Ferghus no ferreiro) → abre o diálogo de saudação com o retrato do NPC, as falas e os botões do menu.
- [ ] Escolher um botão de menu que só conversa (pergunta/assunto) → nova fala no mesmo diálogo, sem travar.
- [ ] Botão de despedida ("tchau"/endtalk) → fecha o diálogo; clicar de novo no NPC reabre normalmente.
- [ ] Clicar no NPC marcado pela história inicial (GuidePlay) → fala da história.
- [ ] Testar 3–4 NPCs diferentes (ferreiro, loja geral, pousada, guilda de aventureiros) e anotar qualquer diálogo que não abra.
Limites conhecidos: opções que executam ações do servidor (dar/aceitar missão, dar item, título, reparo, armazém, manufatura, vocação) param no aviso de comando do servidor e não executam (B1.4/B1.5); abrir a loja chega ao cliente, mas comprar/vender é o B1.4; StorySave e logs de história não migrados.

### B1.4a — comprar na loja do NPC
Pré-condição: Fiona com ouro (B1.1) e espaço livre na bolsa; runtime com os assets atuais (lojas no perfil `npc_scripts`).
- [ ] Falar com um NPC lojista (ex.: Ferghus, loja geral) e abrir a loja → lista de itens com preços.
- [ ] Comprar 1 unidade de um consumível barato → o item entra no inventário (somando à pilha existente, se houver), o ouro diminui exatamente o preço exibido e o cliente mostra a compra concluída.
- [ ] Comprar uma quantidade maior (ex.: 5) → total cobrado = preço × quantidade.
- [ ] Tentar comprar algo mais caro que o saldo → recusa de ouro insuficiente; ouro e inventário inalterados.
- [ ] Relogar → itens comprados e saldo preservados.
Limites conhecidos: itens com limite de compra (restrição por período), itens pagos com AP, itens com atributo na classe (ex.: `[ENHANCE:3]`) e as abas recompra/Buyback/cupom são recusados sem alterar nada (sem resposta); a consulta de limites (QueryShopItemInfo) não responde.

### B1.4b — vender e recomprar
Pré-condição: Fiona com itens vendáveis na bolsa (ex.: poções compradas no B1.4a).
- [ ] Na loja, vender parte de uma pilha (ex.: 2 de 5) → a pilha diminui, o ouro aumenta no preço de venda × quantidade e aparece a mensagem de venda no chat.
- [ ] Vender a pilha inteira → o item some do inventário.
- [ ] Tentar vender mais do que tem → diálogo de quantidade insuficiente; nada muda.
- [ ] Tentar vender um item equipado ou que está na barra de atalhos → diálogo de item equipado; nada muda.
- [ ] Abrir a aba de recompra → os itens vendidos aparecem (mais recente primeiro) com preço = venda × 2.
- [ ] Recomprar um item → volta ao inventário (equipamento com as mesmas cores) e o ouro diminui no preço de recompra; a lista de recompra é atualizada.
- [ ] Relogar → a lista de recompra fica vazia (como no original, ela vive só na sessão); itens e ouro preservados.
Limites conhecidos: venda cujo ouro não caiba é recusada inteira (o original removeria o item mesmo assim); restauração por AP (Buyback_byAP) não migrada.
