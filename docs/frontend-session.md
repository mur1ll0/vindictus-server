# Sessões internas do laboratório (M13.2a)

Esta etapa implementa uma sessão de laboratório em Rust e integra as políticas persistentes de usuários já provisionados. Ela está disponível como API interna, sem comando de autenticação, endpoint HTTP ou ligação ao diagnóstico TCP. Nenhum LoginSuccess é enviado e o cliente ainda não consegue entrar no jogo.

## Contrato recuperado

- Initial.OnMessageReceive encaminha UserLoginMessage para PassportCheck.
- PassportCheck.OnEnter tem o ramo NGM_disable: nome/ID derivam do passport, sem CheckSession externo. UserLogin nesse ramo não atribui IsSecuredLogin; o default é false e SecureCode=0.
- CheckPassportProcessor.Run pertence ao caminho externo: sessão NPL/SSO, idade/metadados, permissões por dados de conteúdo, versão/checksum e manutenção. Não foi substituído por um passport qualquer.
- PassportCheck.checkOp_OnComplete exige Permissions.Login=1 antes de testar manutenção. Se IsOnline=false, exige LoginWhileMaintenance=2. Cheat=0x02000000 sozinho não substitui esses bits.
- GameClient.IsOTP é SecureCode>0, atribuído pelo CheckPassport. Campos de rede do cliente não são uma prova de OTP.
- UserLoginProcessor verifica membro/ban antes de CheckLastLoginIP. A integração desta etapa preserva essa ordem; erro de banco nunca é tratado como ausência de ban.

O gerador scripts/reverse/generate-frontend-gate-fixtures.ps1 extrai sem alterar os blocos de gates e a expressão IsOTP, compila os enums originais e usa shims de operação/parent/logger. Executa somente o harness próprio em Framework x86/x64. Cada perfil tem144 casos de gates e256 códigos secure; fontes/harness têm SHA256 no fixture. Os casos de falha sem Login bit cobrem PassportFail, Banned e PermissionFailed; isso não prova todos os estados externos de CheckPassport.

## Arquitetura e limites

FrontendSession controla Initial → CheckingPassport → CheckingPolicies → PoliciesAccepted. PoliciesAccepted indica somente que os controles implementados passaram; não autenticação de gameplay. Falha volta a Initial; desconexão/deadline fecha definitivamente. UUID por tentativa rejeita callbacks antigos e callbacks de outra conexão, sem armazenar passport bruto ou expor Debug de identidade.

LabAdmission é configuração do operador, nunca obtida de UserLoginMessage. Exige peer loopback e NGM_disable, perfil de identidade explicitamente selecionado, ASCII e limites de tamanho do contrato M13.1. Login é habilitado para esse laboratório; manutenção e Cheat são opt-ins separados e false por padrão nos exemplos/testes. Rede privada não equivale a loopback. O legado concedia permissões amplas automaticamente no ramo local; essa diferença é deliberada.

A arquitetura efetiva do processo original continua não comprovada: Executer e frontend são ILOnly/AnyCPU sem flags32required/32preferred, e a VM x64/fuso UTC+8 não prova o bitness do processo. A API não escolhe um perfil por conta própria e não rehashes IDs importados. Os testes escolhem x86/Invariant somente para usuários sintéticos. Mesmo um hash igual ao importado não prova posse de uma credencial.

evaluate_existing_lab_user consulta ambas as chaves (ID/hash e nome equivalente SQL), recusa ausente/colisão, lê ban, e aplica IPLock/CAS com revisão esperada. Após commit de IP consulta [LimitedAccount](limited-account-return.md) e devolve limited=0/1; erro nessa leitura não desfaz commit anterior. Não cria usuários nem executa dormência, achievements ou recompensa. Erro DB limpa a tentativa e retorna erro sanitizado; revisão divergente exige nova leitura. A entrada de clocks é explícita e separa amostras de aplicação/SQL. Ban e IP continuam em statements/transações separados, sem promessa de atomicidade global contra mudança concorrente de ban.

O prazo absoluto por tentativa é maior que zero e no máximo30s. O adapter inclui aquisição de conexão e todas as consultas no prazo via timeout_at. Cancelamento durante uma escrita/commit pode ter resultado persistente incerto: a sessão fecha e o chamador deve reler a revisão antes de retentar. A sessão não declara sucesso tardio. A API exige propriedade mutável exclusiva da sessão durante avaliação.

## Verificação e próximas dependências

Sete testes Rust cobrem gates/OTP diferenciais, correlação entre tentativas/conexões, recusa de passport fora do contrato, loopback, permissões, prazo na igualdade e transições. Duas integrações isoladas cobrem sucesso das políticas, IP normal600s versus Cheat60s explícito, CAS stale, ban sem mutação de IP, usuário ausente/colisão, falha de banco e consulta realmente bloqueada até deadline.

Gate99Rust+13PostgreSQL+7Python e build/smoke Docker passaram nesta execução.29 usuários importados conferidos nos15 campos após restart. Evidência: docs/evidence/auth/frontend-session.json e docs/evidence/verification/REPORT.md.

M13.2 permanece parcial. Reserva exclusiva e liberação no processo implementadas na [M13.2b](session-registry.md). FrontendSession::new exige o registry compartilhado do runtime. Próximos trabalhos: montagem completa de UserLogin (dormência, token e efeitos; LimitedAccount e contrato tipado verificados na [M13.2c](limited-account-return.md)), seleção comprovada de perfil de implantação e transição para serviços/mensagens de personagem. Depois disso, integração opt-in no frontend e aceitação com cliente. Sessão NPL/SSO e validações online não estão implementadas.

M13.2d.1 verificou o [planejador de efeitos](login-effects.md). Decisões de dormência/retorno e seleção de recompensas estão portadas; execução PostgreSQL, fila e integração à sessão permanecem pendentes.
