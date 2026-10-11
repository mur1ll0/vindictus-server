# Preservação completa de dbo.User

A subetapa M14.3b.1 adiciona as representações dos dez campos restantes às cinco colunas legadas iniciais. migrations/0003_game_user_legacy_fields.sql é incremental: não altera accounts, não lê backups e não copia automaticamente usuários reais. O nome continua limitado ao contrato ASCII explícito de laboratório.

## Mapeamento dos quinze campos

| SQL Server | PostgreSQL | Regra |
|---|---|---|
| ID int | id INTEGER | Preserva signed32, sem sequência |
| NextCharacterSN int | next_character_sn INTEGER | Preserva o contador, não reinicia na importação |
| Name nvarchar24 | name TEXT / name_key gerada | Nome original preservado; comparação ASCII restrita |
| LastLoginIP nvarchar50 | last_login_ip_utf16 BYTEA | Até100 bytes, comprimento par, UTF16 little endian |
| IPLockTime datetime2(7) | ip_lock_time_ticks BIGINT | Até3155378975999999999 ticks de100ns desde0001 |
| ServiceID int | service_id INTEGER | Valor exato, inclusive-1 |
| FacebookToken varchar255 | facebook_token_bytes BYTEA | Até255 bytes originais, sem recodificar |
| IsDormantUserEvent datetime | is_dormant_user_event_units300 BIGINT | Unidades exatas de1/300s desde1900 |
| ChannelingCode tinyint | channeling_code SMALLINT | NULL e0 distintos em snapshot; faixa0–255 |
| SecondPassword nvarchar256 | second_password_utf16 BYTEA | Até512 bytes, comprimento par; respeita schema256, não o DTO128 |
| SecondPasswordFailCount int | second_password_fail_count INTEGER | NULL distinto de0, mantém signed32 |
| SecondPasswordLastFail datetime2(7) | second_password_last_fail_ticks BIGINT | Mesma representação100ns |
| LastUpdate timestamp/rowversion | legacy_last_update BYTEA | NULL ou8 bytes; token congelado da referência |
| IsDormantUser datetime | is_dormant_user_units300 BIGINT | Mesmo contrato1/300s |
| CreateTime datetime | create_time_units300 BIGINT | Snapshot pode informar NULL; cadastro novo usa relógio UTC arredondado |

persisted_at continua metadado operacional independente. name_key também é auxiliar, não uma coluna de dbo.User. Nenhum UUID de accounts é usado como ID do jogo.

## Precisão, bytes e ausência de fuso

[datetime2(7)](https://learn.microsoft.com/en-us/sql/t-sql/data-types/datetime2-transact-sql?view=sql-server-ver17) permite100ns, enquanto os [timestamps PostgreSQL](https://www.postgresql.org/docs/17/datatype-datetime.html) têm resolução de microssegundo. BIGINT evita descartar o sétimo dígito. Os valores não recebem timezone automaticamente: o SQL original não carrega offset e o código legado usa também DateTime.Now.

[datetime](https://learn.microsoft.com/en-us/sql/t-sql/data-types/datetime-transact-sql?view=sql-server-ver17) possui precisão própria. A comparação sintética recuperou dias assinados desde1900 e unidades de1/300s dentro do dia a partir de CONVERT(binary(8),valor). O domínio SqlDateTime conserva o valor como inteiro e reconstitui esses oito bytes, inclusive antes de1900. Este formato foi medido na referência SQL Server atual; novas versões/fontes exigem repetir o oracle.

Nvarchar pode conter NUL e UTF16 sem par válido. Esses valores não podem ser preservados diretamente em uma String Rust/PostgreSQL TEXT. BYTEA conserva as unidades originais, inclusive vazio, sem trocar bytes por caracteres de substituição. Varchar é preservado como bytes da fonte; interpretar o codepage é uma tarefa posterior.

LegacyBytes e os buffers SQL decodificados de IP/credenciais são apagados ao liberar memória. Os tipos de snapshots e credenciais não implementam Debug ou Serialize. SQL usa parâmetros; erros não incluem valores. Essa preservação não verifica nem ativa senhas secundárias.

[rowversion](https://learn.microsoft.com/en-us/sql/t-sql/data-types/rowversion-transact-sql?view=sql-server-ver17) não é uma data. legacy_last_update guarda o token original, sem fingir que PostgreSQL gerou o contador SQL Server. Cadastros novos mantêm esse campo NULL. A geração de versão local para futuras alterações de estado ainda precisa de regra própria.

## Snapshot e conflito

src/infrastructure/legacy_users.rs expõe insert_snapshot e read_snapshot. A inserção fornece explicitamente os quinze valores, incluindo NULL; retorna true para linha nova e false somente se todos os valores já forem idênticos. Qualquer divergência retorna LegacyUserSnapshotConflict sem UPDATE. A transação usa READ COMMITTED, timeouts e locks da linha existente.

A API de cadastro inicial continua retornando o usuário existente intacto. Ela não reinicia contador nem altera campos importados. Snapshot usa ChannelingCode0 como0 quando veio da referência; o initializer de novo cadastro mantém0→NULL.

A migração deixa CreateTime desconhecido para linhas PostgreSQL anteriores à0003, em vez de inventá-lo a partir de persisted_at. Após a migração, somente INSERTs que omitem a coluna recebem relógio UTC em unidades300; um snapshot com CreateTimeNULL permanece NULL.

## Evidência e limites

scripts/database/Compare-UserTypes.ps1 faz somente SELECTs sintéticos na referência. docs/evidence/database/user-types-oracle.json contém8 casos datetime,6 datetime2,5 nvarchar e3 varchar, query/schema SHA256 e timestamp. Nenhum valor de usuário real foi exportado. As strings de data são mantidas como texto pelo gerador.

Os testes verificam extremos, precisão100ns, bytes com NUL/surrogates, limites do banco, NULL/vazio/zero, ida e volta dos quinze campos, replay idêntico e conflito. Um teste aplica as migrações1/2, cria uma linha, aplica3 e confirma que a data antiga continua desconhecida. O smoke Docker guarda campos sintéticos completos e confere o registro após restart; a API Rust é testada em schemas isolados.

Pendente: exportação/importação controlada dos29 usuários reais com contagens e reconciliação, nomes Unicode fora do contrato, personagens, banimento/IPLock, efeitos/procedures, autenticação e sessão. O schema completo de usuário não conclui a migração de LoginService e ainda não permite entrar no jogo.

A importação em lote e seus comandos estão em [user-import.md](user-import.md). O export real é privado; evidências públicas registram somente hashes e contagens.
M14.3b.3 adiciona local_revision separado do token da origem e os adapters de IPLock/banimento; perfil de relógio explícito e limites em [login-policy.md](login-policy.md). Snapshots da origem não são sobrescritos em replay.
