VERIFICATION REPORT
-------------------
Claim: M13.2d.2a verificada: snapshots completos de lealdade/achievement, primitivas SQL e executor restrito de efeitos para usuário existente; M13.2d.2 permanece parcial.
Command: scripts/database/Compare-LoginAccountEffects.ps1; scripts/dev/verify.ps1 -DatabaseIntegration; docker compose build server; scripts/dev/smoke-docker.ps1; scripts/database/Import-Users.ps1 -Mode Verify
Executed: oracle 2026-10-11T05:09:12.2513067+00:00; gate 2026-10-11T05:13:21.0649037+00:00; smoke 2026-10-11T14:08:53.1665070+00:00; usuários pós-restart 2026-10-11T14:08:53.6638745+00:00, após as últimas alterações de código.
Exit code: 0 nos comandos finais
Output summary: Account SQL oracle: cases=10; loyalty source=0; achievement source=0; persistent source writes=false. rust-tests: exit=0; passed=116; failed=0; ignored=20. postgres-integration: exit=0; passed=20; failed=0; ignored=0. client-python: exit=0; passed=7; failed=0; ignored=0. Format/Clippy/doc/build exit=0. Docker build exit=0. PASS: migrate, readiness, stdin account creation, duplicate rejection, non-root/non-superuser, PostgreSQL/server restart preserves account and synthetic game user, ban, local revision, loyalty, achievement and replay receipt. User verify after restart: matched=29; fields=15; inserted=0; export digest unchanged=true.
Warnings: 0 no gate final
Errors: none na verificação final
Verdict: PASS para M13.2d.2a; login pelo cliente e migração completa ainda indisponíveis

As20 integrações foram executadas explicitamente em schemas isolados. Quatro novas integrações cobrem os10 resultados SQL, nove campos fonte, snapshots idempotentes/conflitantes, bytes UTF16 e bounds, commit/replay/mismatch, dependências recusadas, rollback na falha do INSERT FINAL do recibo,16 concorrentes (1 vencedor/15 recusas),16 replays (1 execução/15 replays), cancelamento durante aquisição bloqueada e upgrade5→6/readiness. Doctest tem0 exemplos. Logs completos ficam privados em .cache/verification.

O oracle executa quatro corpos recuperados em procedures/tabelas temporárias de conexão, com nomes de tabelas redirecionados e default de tempo sintético. Tabelas persistentes da referência são somente COUNTadas; nenhum serviço original foi iniciado. FriendLoyaltyInfo e UserAchievement têm0 linhas na referência. CharacterInfo49/LastConnectInfo49/QueuedItem140 continuam pendentes; não foram substituídos por observações vazias.

O executor admite somente SetFriendFlag/ReleaseDormant/UpdateDormantEvent(None)/ReadDormantEvent, com transação, revisão local e recibo UUID. Novos usuários, ownership via plano, set dormência e recompensas são recusados antes de SQL. A primitiva de ownership separada rejeita nomes pendentes fora da igualdade ASCII comprovada. Recibo final conflituoso causa rollback integral. Escritas externas nas dependências precisam da mesma coordenação; a revisão não as cobre automaticamente. Timeout exige repetir o UUID/argumentos originais para verificar eventual commit.

Smoke Docker usa SQL sintético para comprovar preservação dos nove campos e recibo; execução dos adapters Rust é verificada nas integrações isoladas. Os29 usuários reais permanecem iguais nos15 campos. Hashes atuais da imagem/fontes e catálogo/corpos conferidos: divergentes0.

Falhas intermediárias de harness RETURN, tipo de bytes e Clippy foram corrigidas antes do gate final; nenhum teste foi pulado para passar. Evidências: docs/evidence/auth/login-account-effects.json; docs/evidence/database/login-account-contract.json; docs/evidence/database/login-account-oracle.json; gate.json; docker-smoke.json. Contrato: docs/login-account-effects.md. Próximo M13.2d.2b: personagens/conexões/fila, regras de recompensas e executor completo. FacebookToken, UserLogin e TCP/cliente continuam pendentes.
