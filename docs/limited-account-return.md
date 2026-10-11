# LimitedAccount e retorno interno UserLogin (M13.2c)

Esta etapa porta a consulta LimitedAccount e o contrato tipado de retorno da operação interna UserLogin. A sessão de laboratório passa a devolver limited=0/1 após verificar IP. Ela ainda não emite LoginSuccess nem permite login no cliente: dormência, recompensas, FacebookToken como texto do SQL legado e demais efeitos continuam necessários.

## Contrato recuperado e dados de referência

LimitedAccount.cs declara dbo.LimitedAccount com apenas NexonID nvarchar(24) NOT NULL. O catálogo real confirma48bytes, collation SQL_Latin1_General_CP1_CI_AS e nenhuma chave única. LoginService.Limited consulta igualdade por nome e chama SingleOrDefault: zero linhas=false, uma=true, duas ou mais lançam InvalidOperationException.

scripts/database/Compare-LimitedAccount.ps1 executa somente SELECT no SQL Server restaurado. Nesta execução source_rows=0 e source_keys=0, além de7 casos sintéticos de igualdade ASCII. Não havia registros reais a copiar; nenhum nome real foi exportado. Uma futura fonte não vazia requer importação privada de multiset/replay antes de afirmar paridade de dados.

Migration0005 cria game_limited_accounts com UTF16 little-endian opaco, máximo48bytes pares, NOT NULL. local_id é apenas surrogate operacional; duplicates são mantidos. A função generated limited_ascii_key só normaliza ASCII imprimível, case e espaços finais. O índice é não único. NUL/surrogates/Unicode preservam seus bytes e têm chave desconhecida(NULL); a consulta recusa qualquer presença desconhecida para evitar ignorar uma possível restrição cuja collation ainda não foi portada.

A consulta Rust retorna erro tipado para duplicidade e igualdade não comprovada. LIMIT2 limita o material recebido; consultas têm timeouts e transações curtas. insert_snapshot acrescenta uma linha, não é um importador geral/idempotente de uma tabela sem chave.

## Ordem e sessão

UserLoginProcessor verifica banimento, atualiza IP/SubmitChanges, lê LimitedAccount e só então retorna os campos. O adapter de laboratório conserva a leitura Limited após o commit de IP. Uma falha nessa leitura recusa a sessão e libera sua reserva, mas não desfaz um commit anterior de IP. Reler local_revision antes de retentar. Um limited=1 sinaliza restrição; não é uma recusa global de login no legado, portanto a sessão interna continua PoliciesAccepted.

## Sete campos do retorno

UserLogin.Request.Run aceita como sucesso um primeiro feedback cujo tipo é int, seguido de:
1. PackageCount (int);
2. LastLoginIP (string nullable);
3. LockLastSecs (int);
4. Limited (int);
5. IsDormant (bool);
6. FacebookToken (string nullable);
7. IsReturn (bool).

FailReason é um tipo distinto de int, inclusive quando o número coincide: Integer(4) é package count, FailReason(4) é Banned. Banned consome um segundo feedback string nullable para BanedReason; as demais falhas mantêm o motivo vazio. Valores enum assinados desconhecidos são preservados.

domain/user_login_return.rs modela esses tipos internos e exige formato completo. Textos usam unidades UTF16 sem perda de NUL ou surrogate isolado e têm limite explícito; null e vazio são distintos. Nenhum Debug/Serialize captura token/motivo automaticamente. Não se trata do pacote nativo UserLoginMessage nem de novos opcodes/serialization do cliente.

O decoder recusa tipos errados, campos faltantes, excesso e bool false ambíguo. No código original, excesso podia ser ignorado e um primeiro bool false terminava com Reason=None; esses dois casos são endurecimentos deliberados. Campos completos de dormência/token/retorno não são fabricados a partir das políticas parciais. UserLoginSuccess exige os sete valores; o adapter parcial entrega somente revision, client_lock_seconds e limited.

## Provas e continuidade

O harness compila UserLogin.cs e FailReason originais sem alterações, usando stubs próprios da infraestrutura de Cooperation. Cada perfil Framework x86/x64 tem44 casos (sucesso/nulls/UTF16/extremos, enum desconhecido, casts incorretos/truncamento/excesso/false ambíguo) e4 casos BCL SingleOrDefault. Não executa serviços antigos.

Quatro testes Rust novos comparam o contrato; duas integrações PostgreSQL novas verificam collation medida, duplicate error, bytes opacos/bounds, upgrade4→5, readiness, retorno limited=1 e liberação da sessão em erro. O smoke inclui uma linha sintética de LimitedAccount e compara seu digest após restart. Resultados finais e hashes ficam em docs/evidence/auth/limited-account-return.json e docs/evidence/verification/REPORT.md.

Próxima subetapa: efeitos de dormência/retorno/achievements/recompensas e conversão comprovada de FacebookToken. NPL/SSO, perfil efetivo original, ownership distribuído, frontend TCP, personagem e jogo permanecem pendentes.

M13.2d.1 verificou o [planejador de efeitos](login-effects.md). Decisões de dormência/retorno e seleção de recompensas estão portadas; execução PostgreSQL, fila e integração à sessão permanecem pendentes.
