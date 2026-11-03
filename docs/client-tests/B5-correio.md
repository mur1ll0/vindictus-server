# Roteiro de testes no cliente — B5 correio

Correio do servidor Rust (`docs/mail-reference.md`): listar, ler, enviar carta e item entre personagens, retirar item/ouro, apagar, devolver, avisos de correio novo, chegada (60 min ou expresso) e expiração, e o envio automático ao correio quando o inventário está cheio (GiveItem ForceMail). Precisa de **duas personagens** (A envia, B recebe); podem ser da mesma conta (uma de cada vez no jogo) ou de duas contas e dois clientes (aviso imediato com as duas conectadas).

## Preparação
1. Runtime com o correio: depois do commit, `COMPOSE_PARALLEL_LIMIT=1 docker compose build` e `docker compose up -d --wait`; `docker compose ps` mostra `game` e `server` saudáveis. A migração `0054_mail_lifecycle` cria as tabelas na subida.
2. Conteúdo: `.cache/private-content/item-economy.json` tem a seção `mail_types` (13 tipos do `MailTypeInfo`), gerada por `scripts/content/export_item_economy.py`.
3. Cliente: `.cache/play-client/start-lab.bat`, conta `lab169_a` (e `lab169_b` para o segundo cliente, se usar duas contas).
4. Personagem **A** com nível 10 ou mais (enviar exige nível 10; abaixo disso aparece o diálogo `Cannot_Send_Mail_Under_Level_5`). Subir de nível pelo painel (Personagens → ficha → Progressão) se preciso. Ouro e um material negociável, com A **fora do jogo** (PowerShell 7, na pasta do projeto; o `cid` está na ficha do painel):
   - `./scripts/dev/lab-give-item.ps1 -ItemClass gold -Count 20000 -Cid <cidA>`
   - `./scripts/dev/lab-give-item.ps1 -ItemClass <material sem restrição de troca> -Count 5 -Cid <cidA>`
5. Para o inventário cheio (CO8): encher as bolsas de A e então `./scripts/dev/lab-give-item.ps1 -ItemClass <equipamento> -Count 5 -Cid <cidA> -ForceMail` (a saída mostra `mails` > 0).
6. Relatórios de queda, se houver: `Documents\Vindictus EU GM\Report Error`.

## Ordem de gameplay (uma sessão)
1. **CO1** (A): abrir o correio vazio.
2. **CO2** (A): carta de texto para B (normal e expressa).
3. **CO3** (A): item + ouro para B.
4. **CO4** (B): receber, ler e retirar.
5. **CO5** (B): devolver uma carta com item.
6. **CO6** (B): apagar.
7. **CO7** (A): conferir enviadas, devolvida e taxas.
8. **CO8** (A): inventário cheio → correio do sistema; **CO9** retirar com e sem espaço.
9. **CO10** (A): recompensas e caixas com inventário cheio.
10. **CO11**: relogar ambas e conferir tudo.

## Como relatar
Para cada linha marcada: ID e número da linha (ex.: `CO3-2`), o que apareceu na tela (texto, captura se possível), ouro antes/depois quando a linha falar de taxa, se o cliente fechou ou travou, o horário aproximado (para comparar com os logs em `.cache/verification` e `docker compose logs game`) e o nome do relatório de queda, se houver.

### CO1 — Correio vazio na entrada
Pré-condição: preparação feita; A entra no jogo.
- [ ] Abrir o correio (NPC/caixa de correio ou menu) → listas de recebidas e enviadas vazias, sem aviso de correio novo.

### CO2 — Carta de texto
Pré-condição: CO1; B existe (nome conhecido).
- [ ] Enviar carta de texto para B, **normal** → mensagem de envio com sucesso; o ouro de A cai **100**.
- [ ] Enviar outra para B, **expressa** → sucesso; o ouro cai **200**.
- [ ] Enviar para um nome que não existe → erro de destinatário (CIDQueryFailed); o ouro não muda.
- [ ] Enviar para o próprio nome de A → erro (SelfCIDFailed); o ouro não muda.
- [ ] Aba de enviadas → as duas cartas aparecem.

### CO3 — Item e ouro
Pré-condição: CO2; A com o material da preparação.
- [ ] Enviar a B uma carta com 2 do material e 1000 de ouro, normal → sucesso; o material sai do inventário (2) e o ouro cai 1000 + 100 + 2% do preço (do material e do ouro anexado).
- [ ] Repetir expressa com 1 material → taxa 200 + 5%.
- [ ] Tentar anexar um item vinculado, de cash ou com restrição de troca → erro (ImproperTradeLevel) e nada muda.
- [ ] Tentar anexar um equipamento vestido → erro (EquippedItem).

### CO4 — Receber, ler e retirar
Pré-condição: CO3; B entra no jogo (ou já está conectada em outro cliente).
- [ ] Com B conectada durante o envio expresso → o correio de B atualiza sozinho (lista nova com o contador de não lidas).
- [ ] Ao entrar → recebidas mostram as cartas expressas; as normais aparecem só 60 minutos depois do envio (a lista atualiza sozinha até 30 s depois da hora).
- [ ] Abrir uma carta → título, texto e anexos; ela deixa de contar como nova.
- [ ] Retirar os anexos da carta com item e ouro → os itens entram no inventário de B, o ouro sobe 1000; a carta fica com o anexo recebido.
- [ ] Retirar de novo → nada acontece (sem item).

### CO5 — Devolver
Pré-condição: CO4; uma carta com item ainda não retirada em B.
- [ ] Devolver → mensagem de devolução com sucesso; a carta some das recebidas de B.
- [ ] A (agora ou ao entrar) recebe a carta devolvida (título `…MakeReturnMail_Title,<título>`) com o mesmo item, sem custo.

### CO6 — Apagar
Pré-condição: CO4.
- [ ] B apaga uma carta de texto lida → some da lista.
- [ ] B apaga uma carta com item não retirado de uma conta normal → ela volta para A como devolvida (o original devolve em vez de apagar).

### CO7 — Visão do remetente
Pré-condição: CO2–CO6.
- [ ] A vê as enviadas; retira o item da carta devolvida → o material volta ao inventário.
- [ ] Somar as taxas cobradas em CO2/CO3 e conferir com o ouro atual.

### CO8 — Inventário cheio vai para o correio
Pré-condição: A com as bolsas cheias; passo 5 da preparação feito com A fora do jogo.
- [ ] A entra → recebidas mostram cartas do sistema (`Heroes_GameUI_MailSender_SYSTEM`, título `GiveFailMail`) uma por item que não coube, com o anexo.
- [ ] Com A no jogo e as bolsas cheias, pegar um item de núcleo/drop numa missão curta → aparece a mensagem `GiveFailMail_Success_Console` e a carta do sistema chega logo (lista atualiza).

### CO9 — Retirar o correio do sistema
Pré-condição: CO8.
- [ ] Com as bolsas cheias, retirar → aviso de sem espaço (NoEmptySlot); a carta continua com o anexo.
- [ ] Abrir espaço e retirar → o item entra; a carta fica como recebida.

### CO10 — Outras origens com inventário cheio
Pré-condição: A com as bolsas cheias.
- [ ] Abrir uma caixa aleatória (RandomItem) → a caixa é usada; o que não coube vai ao correio (caixa V2: mensagem `RandomItem_UseItem_SendMail`).
- [ ] Concluir uma história com recompensa de item ou um `give_item_failmail` → recompensa no correio.
- [ ] Comprar numa loja com o inventário cheio → **recusado** (o original não usa correio na compra).

### CO11 — Relogin
Pré-condição: CO1–CO10.
- [ ] Relogar A e B → listas, lidas/não lidas, anexos recebidos e ouro iguais ao fim da sessão.

Limites conhecidos: a segunda senha não é pedida no laboratório; o aviso imediato entre personagens exige as duas conectadas ao mesmo servidor; cartas cobradas (tipo 3) exigem nível 10 para retirar; itens reservados de quem sai da missão vão para QueuedItem (não migrado), não para o correio; leilão e eventos que mandam correio não existem ainda.
