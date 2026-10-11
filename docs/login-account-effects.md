# Efeitos persistentes de conta (M13.2d.2a)

Esta subetapa adiciona as nove colunas completas de FriendLoyaltyInfo e UserAchievement e um executor transacional limitado para contas existentes. M13.2d.2 permanece parcial: os49 personagens,49 conexões e140 itens da fila ainda não foram importados; nenhum cliente autentica no Rust.

## Fonte e preservação

scripts/database/Compare-LoginAccountEffects.ps1 confirma por COUNT que FriendLoyaltyInfo e UserAchievement têm0 linhas na referência restaurada. Portanto não havia dados reais dessas duas tabelas a copiar nesta execução. Uma fonte futura não vazia exige importação privada bounded antes de afirmar paridade dos dados. O script aborta nessa condição.

A migração0006 conserva UID/ID signed32, CIDs signed64 nullable, flag signed32, UpdateTime datetime em unidades1/300s e os dois nvarchar em bytes UTF16LE, incluindo NUL/surrogates. Não há foreign keys nessas tabelas no catálogo. O UID=-1 de achievement é preservado; chaves ID e RecommenderUID seguem a referência. O índice de nome é não único e usa apenas igualdade ASCII medida.

Snapshots são insert-or-compare: replay idêntico não faz UPDATE; divergência em qualquer coluna recusa. A implementação de achievement recebe IDs explícitos para snapshots. A alocação automática de novos achievements/identity original ainda não está implementada; não se deve usar MAX(id)+1.

## Operações portadas

SetFriendRecommendFlag usa UPSERT: cria UID/flag com UpdateTime fornecido explicitamente como o valor do default GETUTCDATE. Em linha existente, atualiza flag e limpa RecommenderCID/FriendCID, preservando UpdateTime. Atualizar com a mesma flag também limpa links. O caller fornece o instante SQL quantizado; não é inferido a partir do relógio/container.

ReplaceUserAchievementNIDtoUID altera somente UID=-1 e nome equivalente ASCII, mantendo ID e payload opacos. Nome case/espaços finais seguem a igualdade medida. Nomes Unicode desconhecidos em linhas pendentes recusam toda a operação. Um lock curto SHARE ROW EXCLUSIVE protege a verificação e o UPDATE contra inserções concorrentes de nomes cuja igualdade não está comprovada. A primitiva exige uma transação do caller com limites de espera; criação de usuário/executor completo ainda pendentes.

apply_existing_account_effects admite apenas SetFriendRecommendFlag, ReleaseDormant, UpdateDormantEvent(NULL) e ReadDormantEvent. Qualquer plano com InsertNewUser, ReplaceAchievementOwner, SetDormantUser ou SendDormantRewards é recusado ANTES de abrir a transação. Não cria dormência deixando de enfileirar recompensas.

O executor confere ID+nome equivalente, bloqueia o usuário, valida local_revision esperada e executa o conjunto inteiro em uma transação. Liberação/limpeza são consolidadas numa escrita; uma revisão local cobre o batch, inclusive quando só a lealdade mudou. Leitura do evento observa o estado bloqueado/consolidado do batch admitido. A source rowversion continua opaca e não é regenerada. Um batch sem mutações mantém a revisão.

A tabela operacional game_login_account_effect_runs guarda UUID, identidade, pedido canônico e resultado. Retentar o MESMO UUID/pedido retorna o resultado histórico sem nova escrita/revisão; UUID com outra identidade, revisão, clock ou ações recusa. Retenha o UUID e os argumentos originais em timeout/queda de conexão: commit pode ter ocorrido antes de o resultado chegar. Uma nova operação com revisão antiga recusa. Deadlines15s por batch, lock_timeout2s e statement_timeout5s limitam espera. Erro/cancelamento antes do commit provoca rollback pelo driver.

A proteção de revisão cobre este executor e UPDATEs de game_users. Escritas diretas nas tabelas de lealdade/achievement precisam respeitar a mesma coordenação; não há trigger global propagando toda alteração de dependências. O plano ainda vem de observações explícitas e confiáveis do caller. Não passar NULL para substituir uma consulta de personagens ainda não migrada. Nenhum endpoint ou sessão de laboratório chama esse executor nesta etapa.

Há diferenças deliberadas: batch atômico e recibo idempotente, em vez das várias chamadas com commits independentes do legado; leitura committed, em vez de NOLOCK; revisão local e recusa de dependências não migradas. No original os efeitos precedem ban/IP; a ligação à sessão precisa preservar ou justificar essa ordem explicitamente.

## Evidências e testes

O oracle executa quatro corpos T-SQL recuperados em procedures/tabelas TEMPORÁRIAS da própria conexão: SetFriendRecommendFlag, ReplaceUserAchievementNIDtoUID, ReleaseDormant e DormantUserEventUpdate. Somente os nomes das tabelas são redirecionados; MERGE/UPDATE/RETURN ficam preservados. O default do fixture é um instante sintético explícito. As tabelas persistentes originais são somente COUNTadas. O oracle produz10 cenários: INSERT, UPDATE inclusive mesma flag, limpeza de links/tempo conservado, ownership/igualdade/bytes e release/clear de usuário. Hashes do catálogo e dos corpos ficam no artefato.

Quatro integrações PostgreSQL novas comparam esses resultados e todos os campos dos snapshots; verificam conflitos e bytes/bounds, commit/replay/mismatch, recusa de recompensas/novo usuário, rollback quando falha o INSERT FINAL do recibo,16 concorrentes (1 vencedor/15 revisões recusadas),16 replays (1 execução/15 replays), cancelamento durante aquisição efetivamente bloqueada do pool, upgrade5→6/readiness/idempotência sem alterar usuário existente. Vinte integrações totais são executadas explicitamente em schemas isolados.

O smoke Docker grava apenas dados sintéticos nas duas tabelas e no recibo e compara o digest depois de restart PostgreSQL/server. Não é prova de execução do adapter Rust no Docker; essa execução é verificada nas integrações isoladas. Os29 snapshots reais de User continuam conferidos nos15 campos após restart.

Resultados finais: docs/evidence/auth/login-account-effects.json e docs/evidence/verification/REPORT.md. SQL/catálogos: docs/evidence/database/login-account-oracle.json.

## Próxima subetapa

As dependências foram preservadas em [M13.2d.2b.1](character-login-storage.md): Character49/CharacterInfo49/LastConnectInfo49/QueuedItem140,64campos. M13.2d.2b.2 continua com execução de recompensas/fila, identidade/defaults, falha/replay/concorrência, integração das consultas reais e extensão do executor. FacebookToken, UserLogin completo e integração TCP/cliente permanecem pendentes.
