# Roteiro de testes no cliente — B1 cidade

Bloco B1 (dinheiro, NPCs, lojas, histórias e quests). Gate fechado em 21/10/2026 (resultado no `MIGRATION.md`).

## Preparação
1. Banco do laboratório: o smoke Docker do gate já aplicou as migrações até a 0037. Se o banco tiver sido recriado, rodar `docker compose run --rm --no-deps migrate` (idempotente).
2. Binário nativo: `cargo build --locked`.
3. Conteúdo privado (já regenerado no gate): `.cache/private-content/npc-scripts.json` (scripts de NPC, lojas, reparo e tabelas de quest) e `.cache/private-content/item-economy.json` (formato 3). O `minimum-creation-assets.json` aponta para os dois. Para regenerar: `py -3.13 scripts/content/export_npc_scripts.py --output .cache/private-content/npc-scripts.json` e `py -3.13 scripts/content/export_item_economy.py --output .cache/private-content/item-economy.json`.
4. Personagem: a mesma Fiona inicial (nível 1) aceita no B0. Com ela **deslogada**, conceder ouro: `pwsh scripts/dev/lab-give-item.ps1 -ItemClass gold -Count 50000`.
5. Runtime: `pwsh scripts/dev/start-lab-frontend.ps1 -ProfileFile <o mesmo .cache/private-frontend/runtime-*.json do B0> -MinimumCreationAssetsFile .cache/private-content/minimum-creation-assets.json`. O log do runtime fica em `.cache/verification/lab-frontend-*`. O script também sobe o HTTP de bootstrap (8081) que entrega a lista de servidores ao cliente; abrir o cliente só depois dele. Desde 22/10/2026 o caminho padrão é o Docker: `docker compose up -d --build --wait` (ver a preparação de `docs/client-tests/B2-fases.md`).

## Ordem de gameplay (uma sessão)
1. Antes de entrar: B1.1 (ouro concedido; conferir o saldo ao entrar).
2. Entrar na cidade: B1.3b (NPCs e histórias abertas) e B1.5b (lista de quests com a do Gnoll Watchman revelada).
3. Inventário: B1.6 (caixa Level Up recusada por nível; poção/pena/kit recusados na cidade; cápsula de AP, se houver, funcionando).
4. Conversas: B1.3c (saudações, menus, botões, comando de servidor).
5. História principal: B1.5a seguindo a GuidePlay (Marrec → Ceara → guia para o distrito de combate) e a entrada B1.5b do ramo "quest não concluída".
6. Lojas: B1.4a (comprar), B1.4b (vender e recomprar), B1.4c (reparo sem desgaste).
7. Skills: B1.2 (aprender skill nova pagando ouro).
8. Relogar uma vez no fim e conferir tudo o que cada entrada lista como preservado (ouro, itens, skill, fase/guia das histórias, quests).

## Como relatar
Para cada linha marcada: ID da entrada e número da linha (ex.: `B1.4b-3`), o que foi feito, o que apareceu na tela (texto do diálogo/mensagem; captura se possível) e o horário aproximado (hh:mm), para comparar com `.cache/verification/lab-frontend-*`. Marcar `[x]` no que passou e descrever o que divergiu; uma linha não testada fica `[ ]` com o motivo.

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
Limites conhecidos: skill que exige desbloqueio por história no rank 2 é recusada (serviço de histórias não migrado; nenhuma skill da Fiona está nesse caso); efeitos de atributo da skill nova aparecem após relogar (UpdateStat imediato não é enviado, como no treino do B0).

### B1.3b — lista de NPCs e histórias abertas no login
Pré-condição: runtime com os assets privados atuais (`npc_scripts` = `.cache/private-content/npc-scripts.json`). A Fiona ainda sem progresso de histórias (primeiro login após este passo).
- [ ] Entrar na cidade → sem travar no carregamento; os NPCs aparecem e o cliente recebe a lista de NPCs (o servidor responde à QueryNpcList; log "minimum NPC list sent"). Repetir num segundo login: os NPCs também devem aparecer (falhava em 22/10 com "in-game message rejected", categoria 533).
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
Limites conhecidos (estado no gate): comandos de servidor de história, itens, quests e reparo foram migrados no B1.4/B1.5; título, armazém, manufatura, vocação, skill dada por história e demais comandos de servidor ainda param no aviso de comando sem executar; logs de história não migrados.

### B1.4a — comprar na loja do NPC
Pré-condição: Fiona com ouro (B1.1) e espaço livre na bolsa; runtime com os assets atuais (lojas no perfil `npc_scripts`).
- [ ] Falar com um NPC lojista (ex.: Ferghus, loja geral) e abrir a loja → lista de itens com preços.
- [ ] Comprar 1 unidade de um consumível barato → o item entra no inventário (somando à pilha existente, se houver), o ouro diminui exatamente o preço exibido e o cliente mostra a compra concluída.
- [ ] Comprar uma quantidade maior (ex.: 5) → total cobrado = preço × quantidade.
- [ ] Tentar comprar algo mais caro que o saldo → recusa de ouro insuficiente; ouro e inventário inalterados.
- [ ] Relogar → itens comprados e saldo preservados.
Limites conhecidos: itens com limite de compra (restrição por período), itens pagos com AP, itens com atributo na classe (ex.: `[ENHANCE:3]`) e a aba de cupom são recusados (a recompra veio no B1.4b) sem alterar nada (sem resposta); a consulta de limites (QueryShopItemInfo) não responde.

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

### B1.4c — reparo pago
Pré-condição: na cidade os equipamentos da Fiona estão com durabilidade cheia (desgaste só acontece em combate, B2). Este teste confere o caminho sem desgaste; o reparo com custo real fica para depois do primeiro combate.
- [ ] No ferreiro, escolher a opção de reparo → o NPC informa o preço (0 com tudo inteiro) e conclui o reparo sem descontar ouro e sem travar o diálogo.
- [ ] Usar o botão de reparo da janela de inventário/equipamento (se o cliente oferecer) → nada muda e nenhuma mensagem de erro aparece.
Limites conhecidos: com itens desgastados (B2) o preço segue Item.RepairPrice (RepairEnhance/Quality/Enchant/EquipClass, RepairPriceRate); bônus de guilda não migrado (sem guilda = 0); efeitos de equipamento quebrado (ApplyBadStatus) não migrados.

### B1.5a — histórias da cidade (guia, fases, tokens, itens de história)
Pré-condição: Fiona nível 1 após o login com as histórias automáticas abertas (B1.3b); runtime com a migração 0035 aplicada (`migrate`).
- [ ] Abrir o diário de histórias/guia → aparece a história inicial (GuidePlay) e o marcador de guia aponta para o NPC/landmark indicado.
- [ ] Falar com o NPC marcado na história e seguir o diálogo até o fim → o diálogo não trava; se a fase avançar, o diário mostra o novo objetivo e o marcador de guia muda de destino/mensagem.
- [ ] Se o diálogo entregar ou pedir item de história, o inventário muda na mesma hora (entrega/remoção) e o NPC confirma.
- [ ] Repetir o diálogo do NPC seguinte indicado pelo guia até a história pedir combate ou missão.
- [ ] Relogar → fase, guia e itens entregues pela história preservados (o NPC continua no ponto em que parou).
- [ ] Pegar a missão secundária do Marrec: o diálogo passa da fala "Then I'll tell you a story related to it." (comando `notice_title`), o título aparece na lista de títulos como conhecido e a quest é entregue. No log: "minimum NPC talk processed" (falhava em 22/10 com `Unsupported("notice_title")`).
Limites conhecidos: conclusões de história com recompensa de EXP/AP/título/skill/item (ficam com o XP do B2), `set_timer`/`time_passed` e itens de história com atributo (`[...]`/`_lvl`) não respondem (o diálogo para sem mudar nada); entrega de item sem espaço é recusada inteira (o original manda por correio); recompensa de ouro de história usa GoldRate_Story.

### B1.5b — registro de quests
Pré-condição: runtime com a migração 0036 aplicada (`migrate`) e o perfil `npc_scripts` reexportado com as tabelas de quest (`scripts/content/export_npc_scripts.py`).
- [ ] Entrar com a Fiona → nenhum erro; a lista de quests do cliente (diário/quadro de quests, onde o cliente a mostrar na cidade) traz as quests iniciais de nível 1, com a do Gnoll Watchman revelada.
- [ ] Abrir/fechar a janela de quests algumas vezes → a lista se mantém igual (o cliente pede QueryQuestProgress e recebe o mesmo estado).
- [ ] Na história GuidePlay, ao chegar no passo que manda ir ao distrito de combate, o diálogo segue pelo ramo "quest ainda não concluída" (guia aponta o distrito de combate).
- [ ] Relogar → as mesmas quests, sem duplicar.
Limites conhecidos: concluir quests (objetivos de combate, recompensas, contagem de jogadas, tempos de conclusão) fica para o B2; bônus de cafe/VIP não migrado (sem bônus = limite de jogadas normal); a quest do dia segue o sorteio original pelo perfil de arquitetura do runtime.

### B1.6 — usáveis na cidade
Pré-condição: perfil `item-economy` reexportado no formato 3 (`scripts/content/export_item_economy.py`) e migração 0037 aplicada (`migrate`).
- [ ] Usar (clique direito/uso) a caixa Level Up (`randomitem_levelup_v2_10`) com a Fiona abaixo do nível 10 → diálogo de nível insuficiente; a caixa continua no inventário.
- [ ] Usar uma poção de vida, pena de fênix, kit de reparo, dardo ou bomba na cidade → diálogo de item não utilizável aqui; nada é consumido.
- [ ] Usar uma cápsula de AP (se houver) → continua funcionando como no B0 (AP aumenta, cápsula consumida).
Limites conhecidos: abrir caixas (sorteio RandomItemV2, itens raros e sequências) fica para o B2, quando a Fiona alcança o nível da caixa; demais usáveis (comida, pets, megafones, bônus de cafe/VIP) seguem recusados sem resposta.
