# Decisão de banco de dados

**Decisão: preservar SQL Server como referência do legado e usar PostgreSQL para a implementação Rust nova.** A troca integral depende de equivalência comprovada, não apenas de conseguir importar tabelas.

Os seis arquivos em `../ServidorVindictus/database` são backups do SQL Server. PostgreSQL não restaura esses `.bak`; primeiro é necessário restaurá-los em SQL Server, inventariar o esquema e exportar dados com transformações explícitas.

SQL Server reduz o risco inicial porque mantém T-SQL, procedures, funções, tipos, identidades e collation do comportamento existente. A implementação nova pode usar PostgreSQL sem carregar essas dependências, mas cada regra de negócio em procedure/trigger precisa ganhar implementação Rust ou SQL PostgreSQL equivalente e testes contra a referência.

O Compose sobe PostgreSQL 17 em Debian Bookworm e a aplicação. O serviço `migrate` é uma execução separada: banco saudável → migrações concluídas → servidor. SQL Server 2022 Developer fica em um perfil opcional `legacy`, próprio para estudo/desenvolvimento. Os bancos persistem em volumes independentes; os backups originais são montados somente para leitura. Portas publicadas ficam vinculadas a `127.0.0.1`.

## Operação

Na pasta do projeto, gere `.env` a partir de `.env.example`, com senhas diferentes e fortes. Para PostgreSQL use uma senha sem caracteres reservados de URL. Nunca versione `.env`.

```powershell
docker compose up -d --build --wait
Invoke-RestMethod http://127.0.0.1:8080/health/ready
docker compose --profile legacy up -d --wait legacy-sqlserver
```

O perfil aceita a EULA da Microsoft por `MSSQL_ACCEPT_EULA=Y` e usa `MSSQL_PID=Developer`. Ele cria uma instância nova e não modifica os serviços Windows existentes.

Restaure cada backup para um nome novo com prefixo `study_`:

```powershell
.\scripts\database\Restore-Legacy.ps1 -BackupName heroes.bak -DatabaseName study_heroes
.\scripts\database\Export-LegacySchema.ps1 -DatabaseName study_heroes
```

O restaurador usa `RESTORE FILELISTONLY`, remapeia **todos** os arquivos lógicos para o volume de estudo, recusa substituir um banco existente e executa `DBCC CHECKDB`. Não usa `WITH REPLACE`. Um backup parcial precisa de um plano específico e é recusado. O banco de estudo resultante permanece gravável para a restauração e análise; os `.bak` de origem permanecem somente leitura.

O exportador grava `docs/evidence/database/<nome>.schema.json` com módulos T-SQL completos, tabelas, colunas, chaves, índices, constraints, tipos, dependências e metadados. É um catálogo para comparação, não um script DDL de restauração nem uma exportação de dados. Módulos criptografados aparecem com definição nula e exigem investigação separada. Contagens de linhas vêm do catálogo de partições e são aproximadas.

## Critérios para migrar dados e comportamento

- Converter tipos SQL Server e limites de precisão explicitamente (`datetime`, `money`, `uniqueidentifier`, `rowversion`, `nvarchar`, `bit`, tipos definidos pelo usuário).
- Preservar identificadores existentes; reposicionar sequences/identities após importar.
- Reimplementar T-SQL, procedures, funções e triggers com contratos e transações equivalentes.
- Testar collation: nomes de conta/personagem, igualdade, ordenação, acentos, maiúsculas/minúsculas e espaços finais.
- Comparar contagens, chaves, relações, saldos e itens entre origem e destino, sem importar senhas antigas como se fossem Argon2id.
- Validar criação/login, personagem, inventário e persistência em testes de comportamento antes de retirar SQL Server.

`postgres:17-bookworm`, `rust:1.98-bookworm` e SQL Server `2022-latest` são versões de desenvolvimento verificadas como disponíveis. Para release reproduzível, registrar os digests das imagens testadas e fazer upgrades explícitos. Nunca remover volumes para resolver uma falha de migração.

## Fontes

- [Microsoft: restauração em container SQL Server](https://learn.microsoft.com/en-us/sql/linux/tutorial-restore-backup-in-sql-server-container?view=sql-server-ver17)
- [Microsoft: RESTORE FILELISTONLY](https://learn.microsoft.com/en-us/sql/t-sql/statements/restore-statements-filelistonly-transact-sql?view=sql-server-ver17)
- [Imagem oficial PostgreSQL](https://hub.docker.com/_/postgres)
- [PostgreSQL: collation](https://www.postgresql.org/docs/17/collation.html)
- [Docker: imagens Rust](https://docs.docker.com/guides/rust/)

## Privilégios PostgreSQL

O bootstrap usa POSTGRES_USER (postgres por padrão) somente para inicialização. A aplicação e as migrações usam APP_DATABASE_USER (vindictus_app), criado com NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION, e senha própria APP_DATABASE_PASSWORD. Esse papel é dono do banco dedicado para permitir migrações neste laboratório. Antes de produção, separar o papel de migração (DDL) do papel de runtime (apenas SELECT/INSERT/UPDATE/DELETE necessários), incluindo o CLI administrativo de contas. O script init-postgres.sh roda apenas no primeiro uso de um volume vazio; não apague um volume existente para aplicar mudanças de papel.

## Esquema observado nos seis backups

| Banco de estudo | Tabelas | Procedures | Funções | FK |
|---|---:|---:|---:|---:|
| study_heroes | 134 | 272 | 6 | 31 |
| study_HeroesExcept | 12 | 18 | 2 | 0 |
| study_heroesLog | 141 | 70 | 2 | 0 |
| study_heroesMarketPlace | 14 | 26 | 2 | 0 |
| study_heroesRank | 23 | 24 | 0 | 0 |
| study_heroesShare | 4 | 10 | 0 | 0 |

Total: **328 tabelas, 420 procedures e12 funções**; nenhum trigger, módulo CLR ou módulo sem definição foi encontrado. Todos os bancos usam SQL_Latin1_General_CP1_CI_AS e compatibility_level100. Contagens e catálogos estão em docs/evidence/database/summary.json; a integridade e hashes foram verificados em2026-10-10 por Verify-Legacy.ps1.

O banco heroes contém117 módulos com NOLOCK,34 com MERGE e37 com @@ROWCOUNT em busca textual (indicadores de revisão, não análise semântica). Há duas dependências de catálogo para HeroesTemp, banco ausente dos seis backups: _uid_working e _uid_working_withip. A referência study_* não remapeia automaticamente essas referências para outros bancos. Não executar essas procedures antes de mapear suas dependências.

dbo.User tem ID int fornecido pela identidade de sessão, Name nvarchar(24) e SecondPassword separado; não há senha primária no esquema observado. Personagens usam IDs bigint e modelos distintos Character/CharacterInfo. LastUpdate usa rowversion de8bytes: a conversão não deve tratá-lo como data/hora. O novo esquema de contas UUID/Argon2 não é importação dessa tabela.

**Escolha prática:** manter SQL Server em Docker durante toda a comparação de comportamento; PostgreSQL já atende a base Rust. Substituir SQL Server definitivamente após portar as420 procedures/12 funções aplicáveis (ou justificar aposentadoria) e validar transações, collation, IDs, inventário e batalhas. Não há evidência de que uma troca imediata do banco faça o jogo antigo funcionar.


A comparação SQL de usuários também confirmou21 casos de collation: minúsculas/trim de espaços finais coincide no subconjunto ASCII testado, mas Unicode inclui equivalências como ß=ss, largura completa e composição de acentos. Assim, lower/citext não pode ser declarado conversão geral da CI_AS. Há29 nomes ASCII imprimíveis na referência atual, por contagem agregada. Ver legacy-user-contract.md e evidence/database/collation-oracle.json. PostgreSQL permanece viável por migração explícita de contratos; SQL Server permanece oracle e fonte preservada.