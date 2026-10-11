# Persistência inicial de usuários de jogo

A subetapa M14.3a implementa um subconjunto de dbo.User para usuários novos de laboratório. A API Rust é application::game_users::provision_game_user; não foi ligada ao frontend, HTTP ou CLI. Ela recebe um NexonSN já determinado pelo chamador e não autentica passport, concede permissões ou abre sessão.

## Identidade e comparação

O nome original, incluindo case e espaços, é preservado. A chave remove somente espaços ASCII finais e converte A–Z para a–z. Nomes têm no máximo24 unidades UTF16 e pertencem ao intervalo ASCII imprimível32–126. Unicode e controles são recusados: a collation SQL Server possui equivalências que lower()/citext PostgreSQL não reproduzem.

Os21 pares sintéticos da referência SQL estão em docs/evidence/database/collation-oracle.json. Onze pertencem a esse subconjunto e são comparados pelos testes Rust. Isso não demonstra equivalência geral de collation. Vazio e somente espaços permanecem equivalentes no armazenamento, conforme SQL; a política de autenticação é separada e o módulo de passport já recusa vazio.

| Situação | Resultado |
|---|---|
| ID e chave de nome ausentes | Cria uma linha com os defaults recuperados |
| ID e chave de nome correspondem à mesma linha | Retorna a linha sem atualizar nome, contador, serviço ou canal |
| Nome equivalente já associado a outro ID | GameUserNameConflict, nenhuma alteração |
| ID já associado a outro nome | GameUserIdConflict, nenhuma alteração |
| Ambos correspondem a duas linhas diferentes | Recusa; não devolve os dados de nenhuma das linhas |
| Linha desapareceu/mudou entre conflito e consulta | GameUserConcurrentChange, recusa sem reassociação automática |

O legado procura por nome e pode aceitar um ID diferente. A recusa é uma mudança deliberada para evitar associação ambígua. Truncar o nome antes de armazenar não elimina conflitos: o hash legado usa o passport inteiro. Trocar entre perfis x86/x64 também pode produzir outro ID para o mesmo nome. Os testes cobrem os dois casos.

Um hash32 não prova identidade. Dois passports com o mesmo ID e o mesmo nome truncado não podem ser distinguidos por essas duas chaves. Autenticação futura exige seu próprio vínculo/verificação de passport e perfil original confirmado; o armazenamento atual nunca declara login autorizado.

## Schema e escopo

migrations/0002_game_users.sql cria game_users separado de accounts. Não há vínculo automático entre NexonSN signed32 e UUID de contas operacionais.

| Legado | PostgreSQL inicial |
|---|---|
| ID int PK, sem IDENTITY | id INTEGER PK, fornecido pelo chamador |
| Name nvarchar24, CI_AS | name TEXT COLLATE C, limites e ASCII por CHECK |
| Chave de comparação | name_key gerada pelo banco, COLLATE C e UNIQUE |
| NextCharacterSN int, sem DEFAULT SQL | next_character_sn INTEGER, zero fornecido pelo initializer Rust |
| ServiceID int DEFAULT(-1) | service_id INTEGER DEFAULT(-1), novo cadastro usa valor explícito |
| ChannelingCode tinyint anulável | SMALLINT com CHECK0–255; initializer0→NULL |
| Metadado operacional novo | persisted_at TIMESTAMPTZ; não representa CreateTime ou LastUpdate |

O candidato vem de prepare_legacy_user, comparado às20 fixtures do DTO C# recompilado. A migração0003 agora adiciona os dez campos restantes com representações sem perda; ver [preservação completa de usuário](legacy-user-storage.md). A tabela inicial e o cadastro resumido permanecem separados do adapter de snapshot completo. Exportação/importação de dados reais, comparação Unicode geral e personagens continuam pendentes.

## Concorrência e limites

A transação usa READ COMMITTED explicitamente, lock_timeout3s e statement_timeout5s. INSERT ON CONFLICT DO NOTHING protege PK e nome sem atualizar o vencedor. Uma segunda consulta com novo snapshot lê e protege a linha existente com FOR SHARE, conferindo ambas as chaves antes do retorno. Se falhar, a transação é desfeita.

Esse segundo comando é necessário porque um INSERT pode esperar um concorrente que ainda não estava visível no snapshot inicial. A documentação oficial descreve esse comportamento de [READ COMMITTED](https://www.postgresql.org/docs/17/transaction-iso.html#XACT-READ-COMMITTED) e [ON CONFLICT](https://www.postgresql.org/docs/17/sql-insert.html#SQL-ON-CONFLICT).

Não há rede externa ou callbacks dentro dessa transação. Ela não incorpora banimento, friend_loyalty, dormência, achievements ou correio: essas regras precisam manter seus próprios efeitos e fronteiras recuperados do legado.

## Verificação e próximos passos

scripts/dev/verify.ps1 -DatabaseIntegration executa testes com schemas UUID isolados. Inclui colisões, IDs extremos/negativos/zero, restrições SQL, leitura dos valores existentes, prontidão do schema e16 cadastros concorrentes. Uma prova controlada observa wait_event=transactionid antes de commit/rollback do primeiro INSERT, evitando depender apenas de um atraso arbitrário.

O smoke Docker aplica a migração nova e confere persistência de uma linha sintética após reiniciar PostgreSQL/servidor. Esse INSERT de smoke é feito por SQL para testar o volume/schema; a API Rust é exercitada pelos testes de integração.

Depois da preservação de campos da0003: exportar/importar usuários reais com verificação e implementar/comparar GetUserBanInfo/IPLock e dependências do LoginService, fixar o perfil de identidade, então integrar sessão real do frontend. Nenhum cliente pode autenticar ou jogar nesta etapa.
