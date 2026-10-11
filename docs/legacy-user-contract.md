# Contrato de usuário legado e portabilidade
O fluxo que cria o usuário está em UserLoginProcessor.Run, não em uma API HTTP de cadastro. A porta inicial em src/domain/legacy_user.rs constrói somente os campos de um candidato a INSERT, sem autenticar, consultar nome, gravar banco ou migrar UserLogin completo.

## Fluxo recuperado
Fonte: legacy/decompiled/LoginServiceCore-1600fb4b593d/LoginServiceCore.Processor/UserLoginProcessor.cs.
1. Linhas31–39 exigem conexão interna FrontendServiceCore.FrontendService; linhas40–51 tratam sessão duplicada. Isso depende da camada Entity/Location/DisconnectClient, ainda pendente.
2. Linha52 obtém NexonSN do ID da entidade e linha53 o nome da operação. Cria UserEntity e trata clientes dummy.
3. Linha77 consulta dbo.User por Name via LINQ-to-SQL. O índice único UK_User e a collation SQL_Latin1_General_CP1_CI_AS participam da identidade.
4. Linhas79–115, se ausente: friend_loyalty pode inicializar recomendação; PassportImmigrator pode chamar serviço externo; constrói User com ID=NexonSN, Name=NexonID, ServiceID da instância, ChannelingCode null se0 ou byte se não zero; InsertOnSubmit/SubmitChanges; ReplaceUserAchievementNIDtoUID.
5. Linhas116–199 tratam recomendação/retorno, dormência e recompensas postais conforme FeatureMatrix. Esses efeitos não estão implementados.
6. Linhas200–246 consultam banimento, CheckLastLoginIP e possíveis callbacks PassportImmigrator. LoginService.GetMemberState usa GetUserBanInfo do banco, não uma verificação HTTP equivalente.
7. Linhas250–268 verificam LimitedAccount, agendam revisão de banimento e devolvem resultado interno de sessão; não é LoginFailMessage em TCP diretamente.

O usuário pode ser inserido antes de falhar uma verificação posterior de banimento. Não envolver o fluxo inteiro em uma nova transação sem avaliar essa mudança. Também não substituir duas chamadas SQL independentes por um UPSERT que atualiza um usuário existente silenciosamente.

## Identidade e dados
| Campo | Contrato encontrado | Consequência |
|---|---|---|
| ID | SQL int NOT NULL, PK, sem IDENTITY | Preservar signed32; não regenerar por UUID/sequência |
| Name | nvarchar(24) NOT NULL, UK_User, CI_AS | Limite em unidades UTF16; equivalência de comparação precisa de oracle SQL |
| NextCharacterSN | int NOT NULL sem DEFAULT SQL | Zero inicial vem do campo CLR, deve ser fornecido explicitamente |
| ServiceID | int NOT NULL, DEFAULT(-1); novo usuário recebe ID do LoginService | Default do schema não substitui valor explícito do fluxo |
| ChannelingCode | tinyint anulável |0 no novo cadastro vira NULL;1–255 permanecem valores |
| SecondPassword | SQL nvarchar256; DTO declara128 | Drift schema/fonte; preservar dados256 na importação, não truncar silenciosamente |
| LastUpdate | SQL timestamp/rowversion8bytes; ausente no DTO LoginService.User | Não mapear para data; preservar token original separado da concorrência PostgreSQL |
| CreateTime | datetime DEFAULT getutcdate() | Data criada no banco; observar precisão/UTC |
| IPLockTime / SecondPasswordLastFail | datetime2(7) anulável | Precisão100ns; não presumir que DateTime.Now é UTC |
| FacebookToken | varchar255 anulável | Dado sensível; não exportar ou registrar valores em evidências |

IDs vindos do hash podem colidir, e passports diferentes podem produzir o mesmo nome truncado. O legado procura por nome e nem sempre confirma ID da linha contra o SN da entidade. Uma política de rejeição/reparo deve ser definida explicitamente e comparada; não vincular automaticamente accounts.id UUID a dbo.User.ID.

## Dependências T-SQL
scripts/database/user_contract.py extrai o contrato e hashes das definições já exportadas, sem consultar/exibir dados de contas. Resultado: docs/evidence/database/user-contract.json.

Chamadas diretas incluem GetFriendRecommendFlag, SetFriendRecommendFlag, ReplaceUserAchievementNIDtoUID, GetLatestConnectedDate, GetDormantUserData, ReleaseDormant, SetDormantUser, GetCharacters, SendDormantItem, DormantUserEventUpdate e DormantUserEventSelect. GetUserBanInfo é usado no callback do LoginService. QueryLoginService existe no DataContext recuperado, mas não no catálogo restaurado: não assumir que essa procedure pode ser chamada.

ReplaceUserAchievementNIDtoUID atualiza UID onde era-1 e Name corresponde ao NID. GetLatestConnectedDate usa MAX de CASE entre datas de login/café, com semântica SQL de NULL. SetFriendRecommendFlag usa MERGE, transação e limpa FriendCID/RecommenderCID em atualização. GetUserBanInfo usa status diferente de0, prazo de expiração local e NOLOCK. Isso exige traduções verificadas; copiar apenas o nome da procedure não migra a regra.

## Porta e oracle do cadastro
scripts/reverse/generate-user-fixtures.ps1 recompila o DTO User.cs recuperado sem alterações com referências .NET Framework e um harness próprio que reproduz o initializer de novo cadastro. Não executa assembly original nem SQL. tests/fixtures/framework-user.json cobre20 combinações de ID extremo/negativo/zero e canais0/1/6/255.

prepare_legacy_user conserva IDs, case, defaults CLR e null versus zero. A proteção de24 unidades UTF16 aplica o limite real do banco antes do INSERT; o objeto C# isolado não valida esse limite. A estrutura é um candidato de cadastro, não um espelho completo de todas as15 colunas SQL ou de usuário persistido.

## Próximas provas
- Comparar collation para lookup/UK_User, espaços finais, case e Unicode com SELECT sintéticos na referência SQL, sem alterar usuários.
- Mapear callbacks FeatureMatrix/UserBan, limites e locks para estados de autenticação; testar cenários de falha e IDs conflitantes.
- Introduzir schema persistente PostgreSQL separado de accounts, preservar IDs/defaults/tokens, testar transações e efeitos das procedures contra schema SQL isolado.
- Importar cópia dos dados somente depois do mapeamento, com contagens/hashes e reconciliação dos personagens.
- Conectar o frontend à autenticação de laboratório e provar login real, sem conceder Permissions.Cheat implicitamente.

M13 e M14 continuam parciais; nenhum cadastro de jogo utilizável foi criado por esta porta.
## Comparação SQL executada
scripts/database/Compare-UserCollation.ps1 executou21 comparações sintéticas, somente SELECT, e contagens agregadas. docs/evidence/database/collation-oracle.json registra query/schema SHA256. Nos casos ASCII imprimíveis, minúsculas + remoção somente de espaços finais teve0 divergências. Isso é evidência dos casos testados, não prova geral de equivalência Unicode.

A collation considera ß e ss equivalentes; também trata A de largura completa e A/a como equivalentes e aproxima é composto/decomposto, enquanto é/e são diferentes. Portanto apenas lower() ou citext PostgreSQL não reproduz o contrato completo. Os29 nomes de usuário desta referência são ASCII imprimíveis, confirmado por contagem agregada; nenhum nome real foi exportado.

O adapter inicial de laboratório agora usa esse subconjunto ASCII, recusa colisões de ID/nome e preserva a linha existente. Ver [persistência inicial](game-user-persistence.md). Importação geral e comparação Unicode continuam pendentes. A consulta não executou INSERT/UPDATE/DELETE nem alterou usuários, tabelas ou procedures.
A migração0002 cria somente o subconjunto inicial de game_users, separado de accounts. A API de persistência não foi ligada à autenticação: as15 colunas completas e as regras/dependências T-SQL continuam em M14.3.

M14.3b.1 agora representa os quinze campos SQL em snapshots tipados e persistência incremental: [mapeamento completo](legacy-user-storage.md). As credenciais permanecem bytes opacos e o token rowversion é preservado separadamente. Isso ainda não importa usuários reais nem implementa banimento, IPLock ou sessão de jogo.
