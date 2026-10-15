# Persistência recuperada de itens

M15.1b.2e.3 preserva em conjunto as quatro tabelas que sustentam a futura entrega inicial. O importador CLI `import-item-state --input-stdin` recebe exclusivamente um snapshot privado; `--verify-only` compara sem inserir. Não existe operação HTTP/TCP de entrega ou criação de personagem nesta etapa.

A migração18 conserva Item(9 campos), ItemAttribute(6), Equippable(7) e ItemStorageStatus(7). Textos nvarchar são bytes UTF16 little endian, inclusive NUL e surrogates; varchar conserva bytes da página de código original. Datetime2 conserva ticks e datetime unidades de1/300s, sem inferir fuso. NULL e vazio permanecem distintos. Os inteiros mantêm os tipos e limites da origem; não se acrescentam regras de validade de gameplay a snapshots históricos.

OwnerID em Item e Storage referencia ItemCollection com UPDATE/DELETE CASCADE. A referência não declara FK de Attribute/Equippable; o snapshot conserva inclusive órfãos e não inventa cascatas. Consequentemente, a rotina real ProcessAfterDeleteItem/Undo precisa ser recuperada antes de habilitar a escrita de itens de personagens novos. A criação de IDs de Item continua dependente de MakeEntityID; não foi substituída por uma sequence arbitrária.

O índice original `NCL_ItemAttribute_ItemID_Attribute` é único, embora não seja uma constraint UNIQUE. Sua comparação é `SQL_Latin1_General_CP1_CI_AS`. O perfil implementado aceita apenas chaves de atributos ASCII imprimíveis, inclusive vazio e espaços; compara sem caixa e sem espaços finais. Value conserva bytes arbitrários. Chaves fora desse perfil são recusadas, sem aproximação de collation. O SELECT sintético de referência mede101 entradas e10.201 pares; o teste PostgreSQL compara todo o conjunto de igualdades e exercita o índice único efetivo. A fixture contém somente amostras próprias e o hash da consulta, sem linhas privadas.

ItemAttribute e Storage têm identidades separadas. O bootstrap recebe `sys.identity_columns.last_value`, com seed/increment1 verificados, preserva lacunas e usa `ALTER SEQUENCE RESTART` transacional. Não utiliza MAX+1 ou setval. O estado fixa os dois contadores e as quatro contagens originais. Replay exige o mesmo perfil e o mesmo conjunto completo, compara todos os campos por PK e não reinicia as sequences nem repara conflitos. Linhas novas no destino não são removidas. Importação inicial exige que a totalidade das quatro tabelas corresponda ao snapshot; conflitos tardios e cancelamento antes do COMMIT revertem o lote inteiro e os RESTARTs. Resultado desconhecido de COMMIT e crash não estão resolvidos.

A leitura limita o pacote a16MiB e50.000 linhas somadas, exige campos nullable presentes, inteiros canônicos, IDs únicos, comprimentos de bytes e datas válidas. A identidade suportada exige IDs de Attribute/Storage positivos, contadores nulos somente sem linhas e last_value até Int64.MaxValue−1. Cada transação tem prazo60s, lock_timeout2s e statement_timeout5s. Importação concorrente serializa no estado de bootstrap. Buffers de dados privados usam as representações zeroizáveis existentes; erros e resumos não incluem valores privados.

Reproduzir em PowerShell:

~~~powershell
./scripts/reverse/generate-item-attribute-collation-fixtures.ps1
cargo test --test item_state --locked
./scripts/dev/verify.ps1 -DatabaseIntegration -DatabaseIntegrationDocker
./scripts/database/Import-ItemState.ps1 -Mode Import
./scripts/database/Import-ItemState.ps1 -Mode Verify
~~~

O wrapper usa apenas SELECTs na referência, confirma o digest antes/depois e guarda a exportação em `.cache/private-item-state` com ACL do usuário Windows e SYSTEM. O destino é o PostgreSQL local; apenas contagens, hashes e resultados sanitizados entram no Git. A referência esperada tem2336/209/1032/19 linhas. Logs completos permanecem em `.cache/verification`.

Próximo: construir propriedades reais e Item.Initialize (expiração, atributos gerados, cores/durabilidade/stats/checksum), colocar os itens no storage e salvar/compensar segundo ItemOwner/DBLoader/ProcessAfterDeleteItem. Depois concluir o initializer com Costume, perfil aritmético original, habilidades, títulos e Summary, vinculá-lo ao lifecycle e só então habilitar Create TCP. O aceite de criação/relogin, seleção, cidade e movimento exige o cliente; a migração completa M01–M24 permanece pendente.
