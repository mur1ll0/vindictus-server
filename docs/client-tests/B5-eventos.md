# Roteiro de testes no cliente — B5 eventos (chaves no painel)

Chaves de evento do FeatureMatrix ligadas e desligadas pelo painel (`docs/content-tuning.md`, "Chaves de evento no painel"). É uma extensão do laboratório; drops de evento dos chefes (EV5), recompensas periódicas (EV6), histórias por período (EV7), lojas de cupom (EV8) e metas coletivas (EV10) funcionam; `EventTemplate` não foi migrado.

## Preparação
1. Runtime reconstruído com a migração `0057_content_event_switches` e o `item-economy.json` com a seção `events`.
2. Painel com papel admin; cliente pelo `.cache/play-client/start-lab.bat`.

## Ordem de gameplay
1. **EV1**: abrir a aba Eventos. 2. **EV2**: ligar um evento de loja. 3. **EV3**: desligar. 4. **EV4**: evento de história. 5. **EV5**: drop de evento de um chefe. 6. **EV6**: ficar 30 min online com BoostEvent. 7. **EV7**: bico no dia seguinte. 8. **EV8**: loja de cupom. 9. **EV9**: descrição no painel. 10. **EV10**: meta coletiva.

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

### EV8 — Loja de cupom (`CouponShopItemInfo`)
Pré-condição: cupom da loja na bolsa (por exemplo `coupon_todaymission` da loja 26, `TodayMission` ligado na região, ou `ticket_allusergoal_2015` da loja 40 com `Event_allusergoal_eneu_201507_shop` ligado no painel); dar o cupom pelo painel.
- [ ] Abrir a loja de cupom no cliente → a lista mostra os itens e, nos limitados, quantos restam.
- [ ] Trocar um item → o cupom sai, o item entra, o limite cai em 1; trocar acima do limite → recusa sem gastar o cupom.
- [ ] Relogar → item e limite preservados.
Limites conhecidos: onde o cliente abre a loja de cupom (menu, NPC ou janela de evento) ainda não foi confirmado.

### EV9 — Descrição dos eventos no painel
- [ ] Ajustes de jogo → Eventos → passar o mouse no ícone (i) de um evento → caixa com NPCs, itens das lojas e lojas de cupom (ícone, nome, preço e moeda), drops, quests, histórias com recompensas, títulos, recompensas periódicas e metas, cada seção marcada "funciona no servidor" ou "ainda não funciona".
- [ ] Em tela estreita (celular), tocar no (i) abre e tocar fora fecha.

### EV10 — Meta coletiva (`AllUserGoalInfo`, AllUserGoalEvent)
Pré-condição: no painel ligar `AllUserGoalEvent` e uma chave de meta da região (por exemplo `Event_AllUserGoal_enEU_201501`) e Salvar; as janelas das metas dessa chave passam a começar na hora da publicação (extensão, decisão de 03/11). Relogar.
- [ ] Ao entrar → a janela de evento coletivo mostra a meta do dia com contagem 0 (ou a atual) e as próximas como "em breve".
- [ ] Matar o monstro da meta → a contagem sobe para todos os jogadores conectados.
- [ ] Atingir a meta → aviso para todos (`GameUI_Heroes_GoalEventDialog_Success_Announce`) e a recompensa chega aos conectados e aos que participaram (inventário, ou correio GM se cheio), com a mensagem de recompensa da meta.
- [ ] Na virada da janela → a meta seguinte aparece (ou a janela some no fim do evento).
- [ ] Desligar a chave e relogar → nenhuma meta.
Limites conhecidos: o reset por cheat (`InformAllUserGoalEvent` IsReset) não foi migrado; quem participou e estava desconectado recebe na próxima entrada (QueuedItem).
