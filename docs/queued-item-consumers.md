# Consumidores de QueuedItem — M13.2d.2b.2c.1

Esta subetapa recupera dependências e porta decisões puras em domain::queued_item. Não executa entrega, expiração destrutiva ou marcação como processado. O executor de recompensas anterior continua apenas inserindo entradas. A etapa mãe 2c permanece parcial; login de cliente continua pendente.

## Seleção e contexto

DBLoader.GetQueuedItems: CID igual ao dono e Processed=false. Não há ORDER BY, limite, filtro de expiração ou filtro de estado do personagem. Preservar a ordem recebida; não assumir RowID crescente. A seleção Rust é um predicado em snapshots, ainda sem uma consulta operacional ou consumidor PostgreSQL.

ProcessDailyItemProcessor processa a fila antes dos itens diários com itemFestival=false; ItemFestivalProcessor e AllUserGoalRewardProcessor usam true. O loop materializa a seleção em lista. Processa quando itemFestival=true, dono ausente em RefreshTarget ou EventCode=null. String vazia é evento. O contador aumenta depois do catch mesmo quando a tentativa falha; não representa quantidade entregue. Exceções por entrada são registradas e o loop continua.

SharedDBLoader retorna fila vazia e RemoveQueuedItem sem efeito. NonCharacterDBLoader retorna um elemento null, incompatível com esse loop que acessa EventCode fora do try. Não generalizar esses loaders ao consumidor de personagem.

## Decisões portadas

O match do evento é ordinal exato em UTF16: GuildLevelUp → GuildLevelUpItemMail; MobileAppBuy → AuctionItemMail com ItemGenerator.CheckCharacterBind; FriendEventReward_Add → DungeonItemMail; demais/null/vazio → GMMail. Os enums Rust são internos; valores numéricos do protocolo não foram portados aqui.

A expiração usa HasValue e data <= UtcNow por entrada. A data SQLdatetime é materializada em milissegundos pelo SqlClient antes da comparação com o relógio CLR de 100ns. O limite 1/300s → 3ms e seu tick anterior são testados. Não comparar diretamente unidades de 1/300s a relógios arredondados.

Sem ItemID é criação; com ItemID é recuperação/transferência de item existente. Correio personalizado exige ambos MailTitle e MailContent não-null: vazio e UTF16 opaco continuam válidos. Uma das duas colunas null seleciona inventário com correio automático para sobras.

O binder padrão calcula bound OR FeatureMatrix.IsEnable(info.Bind), com short-circuit, antes de forçar false para gold/silver_coin. Feature inválida pode falhar antes da exceção de moeda. MobileAppBuy tem binder próprio, apenas identificado e ainda não portado. O binder recebe a classe base após parsing, nunca ItemClassEx com atributos.

## Escritas e falhas recuperadas, ainda não executadas em Rust

| Caminho | Comportamento recuperado |
| --- | --- |
| Expirado sem ItemID | Prepara ledger SendToMailFail/QueuedItemExpired por classe/quantidade; marca e Save |
| Expirado com ItemID | GetSystemItem exige item válido em localização nula; ausente registra SimpleLog, presente registra ledger e DestroyItem; marca e Save |
| Criação com catálogo ausente | Retorna mantendo a entrada pendente |
| Criação com correio personalizado | Marca, MakeSystemItems, Save; depois envia cada item; callbacks vazios; emite ledger/mensagem mesmo sem usar o resultado individual do correio |
| Criação para inventário | GiveItem; UnknownError mantém pendente; demais resultados marcam e Save; sobras vão ao correio com título/conteúdo automático |
| Item existente ausente/inválido/localização ocupada | Registra OperationFail/GMMail e mantém pendente |
| Item existente com correio personalizado | Marca e Save antes de enviar; callback síncrono distingue sucesso e registra SendMailFail |
| Item existente para inventário | TransferFromSystem exige Success; depois marca e Save; falhas mantêm pendente |

RemoveQueuedItem não apaga: altera Processed=true e ProcessedTime=UtcNow, sem alterar ItemID, ExpireTime ou LogDate. DBLoader.Save chama SubmitChanges; falha retorna false. ItemOwner.Save chama Undo ao falhar; Undo recarrega o loader e limpa alterações/logs. Sucesso dispara sync, ProcessAfterDeleteItem e LogManager.Write.

ItemTransactionScope só define/reseta o nome e chama onFailed se houver alterações pendentes. Não cria transação SQL e não faz rollback por si só. A entrega original pode ter vários Saves: marcação e correio não têm atomicidade conjunta garantida. Na expiração o resultado de Save é ignorado; na criação MakeSystemItems pode retornar null; a exceção é capturada pelo loop. Essas condições precisam de testes próprios no futuro executor.

SendItemToMail valida item sem dono, monta anexos/MailFactory, chama MailBox.SendMail e Save, e pode gravar nova QueuedItem de recuperação ao falhar, por classe ou ItemID conforme a existência do item. O overload com callback chama o método síncrono e então o callback. Portar apenas uma marcação isolada perderia esse contrato.

## Dependências concretas

- ItemClassExBuilder/ItemContents: parsing, catálogo/Bind/MaxStack/durabilidade e política MobileAppBuy.
- ItemFactory/ItemGenerator: MakeEntityID().Ticks, criação de itens de sistema, pilhas, armazenamento e transferência; não reutilizar identity de QueuedItem como ID de item.
- Item/ItemAttribute/ItemStorageStatus: 9/6/7 colunas; validar localizações, integridade e cascades antes de importar.
- Mail/MailItem/MailBox: 14/14/2 colunas; catálogo, datas, IDs, truncamento UTF16 e anexos. MailFactory usa limite 128/512 chars e múltiplas amostras UTC.
- ItemJournal/ItemLedger e partições 01/02 no banco de logs: allocator de journal e roteamento ainda não recuperados completamente. Tabelas base têm 3/12 colunas; partições, 4/15.
- Save/Undo/log buffer e notificações; uma futura transação Rust precisará documentar diferenças e impedir duplicação em retry.

O catálogo sanitizado docs/evidence/auth/queue-consumer-contract.json lista somente estruturas e hashes, sem linhas reais ou corpos descompilados.

## Verificação reproduzível

scripts/reverse/generate-queue-decisions-fixtures.ps1 extrai seleção, loop/catch, switch, binder e condições do código original. O corpo de entrega é substituído por contador/erro controlado; LINQ usa linhas sintéticas em memória e base.Context vira Context. Os predicados são mapeados para nomes de rota. Isso verifica decisões, sem provar o corpo de entrega ou SQL gerado por LINQ-to-SQL.

Framework x86/x64: 72 tentativas, 504 destinos/expiração, 56 binders e 8 seleções por perfil. Inclui null/vazio, case diferente, NUL, surrogate isolado, ItemID signed, datas SQL extremas, igualdade/um tick antes e feature com falha/short-circuit. cargo test --test queue_decisions --locked compara Rust a esses resultados.

Gate: scripts/dev/verify.ps1 -DatabaseIntegration. Docker: build locked, smoke/restart e conferência somente leitura de 287 snapshots/64 campos e 29 usuários/15 campos. Nenhuma entrada real é entregue ou processada.

Próximo passo M13.2d.2b.2c.2: recuperar ItemClassExBuilder, ItemContents e CheckCharacterBind, estabelecer snapshots/FKs/localizações de Item/ItemAttribute/ItemStorageStatus antes da execução da entrega.
