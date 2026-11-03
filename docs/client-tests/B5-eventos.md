# Roteiro de testes no cliente — B5 eventos (chaves no painel)

Chaves de evento do FeatureMatrix ligadas e desligadas pelo painel (`docs/content-tuning.md`, "Chaves de evento no painel"). É uma extensão do laboratório; os sistemas de evento ainda não migrados (meta coletiva, cupons) não fazem efeito; drops de evento dos chefes (EV5), recompensas periódicas (EV6) e histórias por período (EV7) já funcionam.

## Preparação
1. Runtime reconstruído com a migração `0057_content_event_switches` e o `item-economy.json` com a seção `events`.
2. Painel com papel admin; cliente pelo `.cache/play-client/start-lab.bat`.

## Ordem de gameplay
1. **EV1**: abrir a aba Eventos. 2. **EV2**: ligar um evento de loja. 3. **EV3**: desligar. 4. **EV4**: evento de história. 5. **EV5**: drop de evento de um chefe. 6. **EV6**: ficar 30 min online com BoostEvent. 7. **EV7**: bico no dia seguinte.

## Como relatar
ID da linha, o que apareceu (captura), horário (para `docker compose logs game`) e o nome do relatório de queda, se houver.

### EV1 — Aba Eventos
- [ ] Ajustes de jogo → Eventos → lista de chaves com busca, filtros (Natal, Páscoa, Halloween, SAO…) e, em cada uma, as tabelas que usam a chave (riscadas as ainda não migradas); nenhuma chave `_kokr`, `_jajp` ou `_zhcn`.

### EV2 — Ligar um evento de loja
Pré-condição: escolher uma chave com `ShopInfo` (por exemplo `Event_eneu_christmas_2012_shop` ou outra de Natal com ShopInfo).
- [ ] Marcar e Salvar → mensagem de aplicado na hora; a auditoria mostra `content.tuning`.
- [ ] Entrar no jogo (novo login) → a loja do NPC do evento mostra os itens do evento; comprar um → entra no inventário.
- [ ] NPC ou decoração do evento aparece na cidade (confirma a hipótese do `SyncFeatureMatrix`); anotar se não aparecer.

### EV3 — Desligar
- [ ] Desmarcar e Salvar; relogar → os itens do evento saem da loja; o item comprado continua no inventário.

### EV4 — Evento de história
Pré-condição: uma chave com `StoryLineInfo`/`QuestInfo`.
- [ ] Ligar, relogar → a história/quest do evento aparece no NPC; desligar, relogar → some.
Limites conhecidos (EV4): `EventTemplate`, `PeriodicEventInfo`, `AllUserGoalInfo` (SAO) e lojas de cupom não migrados.

### EV5 — Drop de evento dos chefes (`EventDropItemInfo`)
Pré-condição: ligar no painel uma chave com `EventDropItemInfo` sem janela de data vencida (por exemplo `Event_Vindictus_OBT`, linhas sem `StartTime`) e relogar; quest com chefe que solta itens (DropItem do chefe > 0).
- [ ] Matar o chefe → além dos núcleos normais aparece um núcleo de evento (EventCore); chutar → o item do evento entra no inventário (ou no correio, se cheio).
- [ ] Desligar a chave, relogar e repetir → nenhum núcleo de evento.
Limites conhecidos: em party, a condição `TodayQuest` usa o estado do próprio jogador (o original soma o de todos); linhas com `StartTime`/`Period` só valem dentro da janela (as do conteúdo são de 2010–2015).

### EV6 — Recompensa periódica (`PeriodicEventInfo`, BoostEvent)
Pré-condição: no painel, ligar `BoostEvent` e uma chave com `PeriodicEventInfo` da região (por exemplo `Event_boost_eneu`); relogar. O `BoostEventPeriod` da região é 1800 s (30 min).
- [ ] Ficar 30 min online com a personagem → mensagem de item recebido (ouro, AP, EXP ou item do evento); o item entra no inventário (ou no correio, se cheio).
- [ ] Prêmio com anúncio (`GameUI_Heroes_GetitemEvent_Telepathy`) → aviso para todos os jogadores conectados.
- [ ] Desligar `BoostEvent` e relogar → nenhuma recompensa no período seguinte.
Limites conhecidos: o diário e o razão de itens da recompensa não são gravados.

### EV7 — Histórias por período (`StoryLinePeriodInfo`)
Pré-condição: personagem com um bico (Arbeit, por exemplo `Arbeit_get_fish`) concluído ou avançado num dia anterior (UTC).
- [ ] Entrar no jogo no dia seguinte → o bico volta para a fase inicial (A) no NPC, disponível de novo.
- [ ] Relogar no mesmo dia → nada muda.
Limites conhecidos: a troca no meio da sessão (virada do dia) não avisa o cliente, como no original; ele vê na próxima consulta da história.
