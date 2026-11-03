# Correio: referência legada e ciclo de vida

## Referência legada (M15.1b.2e.10b.22f.1)

A migração 0032 conserva os 14 campos de `dbo.Mail`: MailID, FromCID, FromName, ToCID, ToName, MailType, PostedTime, Title, Content, ArriveTime, ExpireTime, IsRead, ChargedGold, IsDeleted. PK MailID sem identidade nem FK, defaults FromCID/ToCID = -1, IsRead = false, IsDeleted = false. IDs assinados e tinyint 0..255 preservados. Datas em unidades exatas de 1/300 s desde 1900 (binary8 do SQL); ExpireTime e IsRead mantêm NULL distinto de zero/false. Strings em bytes UTF-16LE brutos, com os limites reais 512/40/512/2048 bytes.

O import usa stdin privado limitado (16 MiB/100000 linhas), recusa campos desconhecidos e NULLs omitidos, ordena por PK e compara todos os campos. Um marcador SHA-256 congela a exportação; o bootstrap é transacional e só em tabela vazia. `mail-source.sql` valida a fonte fresca (colunas, tipos, PK, contagem 169 e a definição de QueryMailList contra o hash recuperado). Dados históricos não vão para fixtures nem Git.

## Ciclo de vida (B5, 03/11/2026)

Implementado em `src/infrastructure/minimum_mail.rs` a partir de `PlayerService.ItemServiceCore` (MailBox, MailFactory, TimeLine, os processadores Query/Send/Get/Delete/Return/NoticeMail e `ItemOwner.TransferToSystem`/`TransferFromSystem`), com o roteamento do `FrontendServiceCore.Proxy.ItemServiceProxy`. Fontes e hashes em `src/domain/mail_sources.json`.

- **Esquema (0054):** `game_mail_items` (as 14 colunas de `dbo.MailItem`, MailItemID identidade, sem FK, como no esquema real) e o diário postal `game_postal_journals`/`game_postal_ledgers` (AddPostalJournal/AddPostalLedger do banco de logs).
- **Anexos:** cada anexo é um item de sistema em `game_items` (dono NULL, local nulo 255/-1), como `MakeSystemItem`; ouro anexado também. Receber dá uma cópia ao personagem (tudo ou nada, preservando cores, durabilidade, atributos e validade) e destrói o item de sistema; o ouro de sistema confere o checksum.
- **Envio:** nível ≥ 10 (diálogo `#GameUI_Heroes_Cannot_Send_Mail_Under_Level_5`), `IsCorrectUserMailType`, `GetCIDByName` (Status = 0, sem diferenciar maiúsculas), recusa de si mesmo, `IsMailableTo` (sem validade vencida, sem vínculo, sem RESTORED/proteção de SkillEnhance, `TradeRestirction` nulo ou ACCOUNTONLY entre personagens da conta, não cash). Taxa = Fee (+ FeeForItem% do Price de cada item transferido, ouro incluído), em ponto flutuante simples como no x64 (casos do oráculo em `tests/fixtures/framework-mail-wire.json`). Chegada em Arrive_in minutos (expresso: imediata); expiração em Arrive + Expire_in.
- **Lista (ProcessMailList):** TOP 300 recebidas + TOP 300 enviadas por ArriveTime; RequiresDelete (postada há mais de 30 dias com expiração) é apagada; vencida e devolvível volta ao remetente como ReturnedMail; até 36 por lista; NewMailCount ignora anexos recebidos.
- **Retirada:** carta cobrada exige nível ≥ 10 e paga ChargedGold do destinatário, que segue ao remetente como ChargePayMail. Correio de sistema sem item (GM) usa GiveItem vinculado.
- **Apagar/devolver:** apagar devolve o que é devolvível; senão marca IsDeleted e destrói os itens de sistema pendentes (GM recusa a operação inteira). Devolver exige o primeiro anexo pendente.
- **Avisos:** a lista vai ao cliente na seleção (ItemServiceProxy.Open) e a cada job vencido da TimeLine (sondagem de 30 s); NoticeMail de outro personagem chega pelo hub da party (`Notice::Mail`), e o do próprio personagem (ForceMail) logo depois do pedido.
- **GiveItem ForceMail:** `minimum_give::apply_force_mail` aplica a parte que cabe e transforma as sobras (sem espaço ou acima do único) em itens de sistema enviados como DungeonItemMail (`#GameUI_Heroes_GiveFailMail_Title,<classe>,<n>`, remetente `Heroes_GameUI_MailSender_SYSTEM`), com a mensagem `GameUI_Heroes_GiveFailMail_Success_Console`, na mesma transação. Origens ligadas: loot/erg/coleta e itens reservados do chefe (GiveItemRewards), moeda de serviço da quest, recompensas de história e `give_item_failmail`, caixas RandomItem (V1 com mensagem; V2 sem ela e com `RandomItem_UseItem_SendMail`), sobras do Enhance, restauração, pedra protegida do SkillEnhance, itens diários e desempartilhar comida de pet. Laboratório: `lab-give-item --force-mail`.
- **Sem correio no original (mantido):** recompensa de conclusão de missão (RewardCharacter usa GiveItem comum), compra em loja, Craft e Manufacture (ProcessProduct). Itens reservados de quem saiu da missão vão para QueuedItem, não para o correio.

Desvios conhecidos: MailID vem da própria tabela (maior valor + 1, nunca abaixo dos ticks de agora) em vez do MakeEntityID; a segunda senha não é pedida (o laboratório não tem bloqueio em jogo); o aviso entre personagens exige os dois conectados ao mesmo processo; ReceivedDate usa o relógio UTC do laboratório.

## Verificação legada depois do ciclo de vida

`preserve(verify)` aceita o que o ciclo de vida faz numa linha preservada (IsRead e IsDeleted só passam a true; linha apagada ou devolvida com diário postal daquela MailID) e ignora linhas novas; qualquer outra alteração ou perda sem diário é `Reference`. O antigo guard `require_query_empty` foi removido: o QueryMailList real é comparado ao oráculo SQL Server (`tests/fixtures/sqlserver-empty-mail.json`).
