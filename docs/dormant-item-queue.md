# Fila de recompensas de retorno — M13.2d.2b.2a

A API Rust reproduz a criação de uma entrada feita por `dbo.SendDormantItem`. A entrega de itens, inventário, correio e a execução completa do login continuam pendentes.

## Contrato recuperado

A procedure recebe CID bigint, ItemClassEx nvarchar(1024), Count int, IsBinded bit, MailTitle nvarchar(256) e MailContent nvarchar(1024). As duas últimas aceitam NULL. Todos os inteiros assinados são conservados, inclusive quantidades zero/negativas aceitas pelo SQL original. Não é adicionada uma FK de CID para personagens.

A entrada usa cores -1/-1/-1, ReducedDurability=0, MaxDurabilityBonus=0, ItemID=NULL, Processed=false, EventCode=NULL, ProcessedTime=NULL e ExpireTime=NULL. LogDate vem do default GETUTCDATE. A API recebe esse instante como um `SqlDateTime` já quantizado em unidades de 1/300s; o chamador futuro deve capturar o relógio UTC por chamada. Não há substituição automática por relógio local nem default de LogDate para INSERT SQL arbitrário. UTF16 permanece como bytes, com NUL/surrogates e distinção NULL/vazio.

`send_dormant_item` recebe uma transação do chamador, para permitir composição futura com efeitos da conta. `enqueue_once` fornece a transação com lock_timeout=2s, statement_timeout=5s e deadline=15s incluindo a aquisição da conexão. Nenhum endpoint de jogo chama essas APIs nesta etapa.

## Identidade e importação

Migração0009 cria uma sequência integer, seed/increment=1, CACHE1 e NO CYCLE, inicialmente desabilitada para a API. A referência recuperada tem sys.identity_columns.last_value=140. A inicialização exige esse contador explicitamente, depois da cópia dos snapshots. Não deriva o próximo ID de MAX(RowID), porque exclusões e rollbacks podem deixar o contador acima dos IDs sobreviventes.

`initialize_identity` bloqueia o singleton do alocador, verifica que nenhum snapshot supera o contador informado e instala o próximo valor usando ALTER SEQUENCE RESTART dentro de uma transação. Perfil repetido retorna initialized=false e conserva o contador avançado; perfil diferente é recusado. None só aceita uma fila vazia cuja identidade nunca foi usada. O perfil suportado tem último valor entre0 e2147483646 ou NULL; contadores negativos/reseed diferente/esgotado na origem exigem uma recuperação separada. A sequência pode emitir2147483647 uma vez e depois recusa novas alocações.

Após a inicialização, a importação permite verificar/repetir snapshots já existentes; novas linhas de fila por importação são recusadas, com rollback do bundle. Isso evita que uma importação posterior introduza IDs que o alocador ainda emitiria. Os INSERTs de preservação anteriores à inicialização mantêm IDs explícitos e não consomem a sequência.

Para a referência atual do laboratório, após `Import-Characters.ps1 -Mode Import` e `-Mode Verify`:

~~~powershell
docker compose run --rm --no-deps server initialize-queue-identity --source-last-value 140
~~~

O140 pertence à evidência desta referência; em outra restauração, recupere o last_value do catálogo correspondente antes de inicializar. Para uma instalação nova sem snapshots e com identidade nunca usada, omita --source-last-value. A migração/readiness operacional permite o alocador ainda desabilitado; a criação de itens recusa esse estado.

## Replay, falhas e concorrência

O recibo identifica uma operação UUID e conserva uma representação binária versionada de todos os parâmetros e do relógio original. O chamador deve reusar o UUID e esses mesmos parâmetros no retry. Alterações são recusadas antes de alocar outro ID. Os buffers não têm Debug/Serialize e são zeroizados; o recibo contém dados privados no PostgreSQL e não deve ser publicado.

A reivindicação do recibo precede a alocação. INSERT da fila e conclusão do recibo ocorrem juntos. Um rollback elimina ambos, mas deixa uma lacuna no contador, como o IDENTITY observado no SQL Server. Um retry após commit desconhecido lê o recibo original. Operações distintas recebem IDs distintos. O recibo não tem FK para a fila: processar ou excluir uma entrada não apaga o histórico nem permite recriar a recompensa no replay. Essa garantia de retry é uma melhoria local explícita; a procedure SQL original cria outra linha a cada chamada.

Não se promete sequência sem lacunas, nem equivalência de contadores após crash/cache do SQL Server. O teste cobre concorrência e rollback normal; nenhuma VM original foi iniciada.

## Verificação e próxima etapa

`Compare-DormantItems.ps1` executa o corpo recuperado sem alteração semântica, redirecionado para uma tabela TEMP com18colunas/defaults do catálogo. Usa GETUTCDATE real, captura limites antes/depois e verifica que cada valor cai nesse intervalo. Dados sintéticos incluem limites assinados, NULL/vazio, NUL/surrogate e textos no tamanho máximo. A tabela temporária simula uma identidade previamente usada; DBCC RESEED numa tabela nunca usada tem comportamento diferente e foi corrigido no harness. IDs esperados:141/142/143, rollback144 e inserção145. As140linhas e o contador140 da origem não são alterados.

Sete testes PostgreSQL isolados comparam todos os18campos desses quatro resultados, comprovam inicialização simultânea/contador acima dos IDs,16retries/1vencedor,16UUIDs/16IDs, alteração dos sete parâmetros, replay após Processed/exclusão, falha tardia no recibo, esgotamento e readiness. O gate completo e Docker são registrados em docs/evidence/auth/dormant-item-queue.json e docs/evidence/verification/REPORT.md.

Próximo M13.2d.2b.2b: carregar/versionar DormantReward e resolver FeatureMatrix; montar recompensas por personagem na ordem recuperada, inclusive título e texto com nome UTF16; comprovar conversão SQL datetime→CLR callback e integrar observações reais no executor. Depois portar processamento/expiração com inventário, correio, logs e transações. DBLoader.GetQueuedItems filtra CID e Processed=false, sem ordem explícita; RemoveQueuedItem apenas marca Processed=true e ProcessedTime=UtcNow. QueuedItemController depende de ItemTransactionScope, ItemFactory, Save e envio de correio; marcar uma entrada como processada sem portar essas dependências descartaria itens. O executor atual continua recusando planos que exigem recompensas.
