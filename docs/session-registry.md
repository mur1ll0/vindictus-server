# Reserva de sessões de laboratório (M13.2b)

O runtime de laboratório agora exige uma SessionRegistry compartilhada por todas as suas conexões. FrontendSession::new(&registry) substitui o construtor Default; clonar o handle compartilha o mesmo mapa. Instanciar um registro novo por conexão quebra a exclusividade e não é a configuração suportada. A implementação continua interna, sem ligação ao frontend TCP e sem login de cliente.

## Evidência do legado e diferença deliberada

UserLoginProcessor.Run exige uma conexão de FrontendServiceCore.FrontendService. Se LocalEntity.Tag já contém UserEntity, o legado registra Duplicate, solicita DisconnectClient para o FID anterior, retorna FailReason.DuplicateLogin=2 e enfileira o fechamento da entidade. Essa verificação acontece antes de consultar/provisionar dbo.User.

LoginService.MakeEntity fecha a entidade quando UseCount==0. No evento Closed tenta AcquireService(ID,Category,-1,ServiceID) e depois limpa Tag, inclusive quando o acesso ao catálogo de entidades falha. AcquireService e o mecanismo distribuído de UnifiedNetwork não estão portados nesta etapa.

O harness scripts/reverse/generate-session-lifecycle-fixtures.ps1 extrai esses blocos, conserva a ordem/condicionais, e adapta somente interface/acessibilidade para stubs próprios de entidade/evento, scheduler, logger e banco. Não executa os serviços originais. Cada perfil Framework x86/x64 gera12 casos de duplicidade e18 de lifecycle, incluindo ID signed mínimo/zero/máximo e9 falhas simuladas de AcquireService. Não comprova o SQL de AcquireService nem ownership distribuído.

Rust mantém a sessão ativa e recusa a nova com DuplicateLogin. A tentativa nova não pode desconectar o dono anterior. Essa diferença é deliberada no laboratório, onde um passport local não comprova posse de uma credencial.

## Propriedade, prazo e cancelamento

A reserva ocorre após o gate de identidade/permissão do laboratório e antes das consultas de políticas. CheckingPolicies e PoliciesAccepted mantêm a reserva do NexonSN. Falha limpa a reserva; close é idempotente; Drop da sessão libera mesmo em task abort. Reserva duplicada preserva a sessão anterior e volta a tentativa nova para Initial. Usuários diferentes não interferem.

SessionRegistry guarda somente IDs signed32 e referências fracas de ownership, sob um Mutex curto sem await. O dono forte é privado, não clonável e pertence a exatamente uma sessão. O descarte não precisa de mutex ou tarefa de limpeza assíncrona. Entradas fracas mortas são removidas na reutilização/capacidade ou ao obter contagem agregada. Um callback/close antigo não remove o dono de uma reconexão.

Capacidade do operador:1–16384. O mapa nunca cresce além da capacidade; saturação e mutex poisoned retornam erros tipados e recusam admissão. Não há expiração automática de uma sessão abandonada cujo proprietário ainda esteja vivo: o adaptador de conexão deve fechar/descartar a sessão quando perder o peer. A integração TCP dessa responsabilidade está pendente.

PolicyGuard acompanha evaluate_existing_lab_user durante aquisição de conexão/consultas e fecha a sessão quando o future já iniciado é descartado, mesmo se o chamador conservar a sessão. O encerramento por deadline também libera. Um future nunca polled não inicia o guard; a reserva pertence ao objeto de sessão e exige close/drop pelo proprietário. Cancelamento durante commit pode deixar resultado persistente incerto: reler a revisão antes de retentar. Não se afirma rollback automático após um commit confirmado pelo banco.

PoliciesAccepted mantém exclusividade, mas não autenticação de gameplay. A reserva não é uma linha permanente de usuário nem uma credencial. Após encerrar o processo todas as reservas desaparecem; usuários persistentes continuam no PostgreSQL. O escopo é UM processo com UM registry compartilhado; múltiplas réplicas precisam de ownership distribuído comprovado antes de serem suportadas.

## Verificação e continuação

Os testes novos cobrem16 conexões disputando um ID (1 dono/15 recusas), usuários distintos, saturação/reutilização,512 ciclos de churn, IDs signed extremos, poison, callbacks/close antigos e task abort. A integração PostgreSQL cobre recusa antes de mutação, manutenção da reserva após políticas, reconexão com no-op de IP e cancelamento real durante aquisição bloqueada (pool isolado com8 conexões ocupadas), mantendo a sessão viva para comprovar cleanup.

A verificação final fica em docs/evidence/auth/session-registry.json e docs/evidence/verification/REPORT.md. M13.2 continua parcial. [LimitedAccount e contrato tipado UserLogin](limited-account-return.md) verificados na M13.2c. Próxima subetapa: dormência/retorno, achievements/recompensas e conversão de token; personagens e ligação ao frontend/cliente continuam pendentes.
