# Importação privada de dbo.User

M14.3b.2 preserva uma cópia de dados; não ativa credenciais antigas nem autenticação do jogo. A base `accounts` continua independente. PostgreSQL é o alvo; SQL Server `study_heroes` continua referência.

O comando `import-users --input-stdin` recebe JSON UTF8 versão1, origem `study_heroes.dbo.User`, `exported_row_count` e `rows`. Cada linha exige todos os quinze campos, inclusive os nullable. Datas são inteiros decimais **em strings** (ticks100ns ou unidades300), bytes são hex sem prefixo; NULL, bytes vazios e zero são distintos. A fixture `tests/fixtures/user-export.json` contém somente valores sintéticos.

O leitor recusa entrada acima de2MiB, lotes vazios/acima de1000 linhas, origem/versão/contagem divergentes, campos ausentes/desconhecidos/duplicados, datas numéricas ou não canônicas, bytes inválidos e nomes fora do contrato ASCII. Todo o lote é validado antes das consultas; IDs e chaves de nome duplicados são recusados. JSON e textos de credenciais mantidos pelo Rust são zeroizados ao sair de escopo; objetos de dados não têm Debug/Serialize. Isso não garante apagar cópias internas de drivers, sistema operacional ou PowerShell.

O lote é ordenado por ID e processado numa transação READ COMMITTED, com binds, `ON CONFLICT DO NOTHING`, locks e comparação dos quinze valores. Um registro existente só é aceito quando todos os campos são iguais. Divergência causa rollback do lote; não há UPDATE, correção automática, rehash de ID ou rehash de senha. Depois dos INSERTs todos os snapshots são relidos/comparados sob lock antes do commit. Duas consultas distintas preservam a visibilidade necessária após um INSERT concorrente. Timeouts: lock3s, statement5s, operação30s. Falhas durante commit ou deadline podem deixar o resultado desconhecido para o chamador: executar `--verify-only` antes de repetir. Replay idêntico é idempotente.

`--verify-only` compara os quinze campos de cada ID e não insere nada. A saída mostra somente contagens e capacidade de autenticação=false. A comparação cobre as linhas da cópia; não exige apagar outros usuários sintéticos existentes no banco.

## Execução local

Com os containers `legacy-sqlserver`, `postgres` e `server` disponíveis e a imagem atual construída:

```powershell
./scripts/database/Import-Users.ps1
./scripts/dev/smoke-docker.ps1
./scripts/database/Import-Users.ps1 -Mode Verify
```

O primeiro script espera29 linhas por padrão, lê o SQL Server em transação SERIALIZABLE com SELECT/variáveis locais e nenhuma escrita, mantém NULL e precisão e exporta para `.cache/private-user-import/users.json`. A pasta privada tem ACL restrita ao usuário Windows atual e SYSTEM, sem herança externa; `.cache` está ignorada. Não publicar o export nem copiá-lo para evidências. Ele contém nomes e dados antigos sensíveis. O script evita argumentos com esses valores, passando o JSON pelo stdin do binário dentro do Docker.

A importação verifica delta de contagem do PostgreSQL, replay sem INSERT, igualdade dos quinze valores e reexport da origem com digest idêntico. `docs/evidence/database/user-import.json` contém apenas contagens, hashes, método e timestamps. O smoke reinicia PostgreSQL/server e verifica infraestrutura; executar `-Mode Verify` depois dele para conferir novamente os usuários reais. Esse modo rotula a evidência como pós-reinício; o operador deve efetuar o restart primeiro.

O export e o SQLcmd/PowerShell usam buffers privados em memória, sem promessa de zeroização desses runtimes. Os erros detalhados são suprimidos nesse script para evitar que mensagens de drivers revelem linhas. O gate de testes utiliza somente dados sintéticos em schemas isolados.

Os29 usuários não bastam para permitir login: banimentos/IPLock, identidade efetiva do runtime antigo, sessões, personagens, demais dependências e aceite do cliente continuam pendentes.
