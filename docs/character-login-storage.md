# Personagens, conexões e fila preservada — M13.2d.2b.1

Esta subetapa importa quatro tabelas completas e porta duas consultas de login. Não implementa criação de personagem, entrega de itens, combate ou autenticação pelo cliente. M13.2d.2b e M13.2d.2 continuam parciais.

## Contrato recuperado

| Referência | Campos | Linhas reais | PostgreSQL |
| --- | --- | --- | --- |
| Character | 1 | 49 | game_characters |
| CharacterInfo | 37 | 49 | game_character_info |
| LastConnectInfo | 8 | 49 | game_last_connect_info |
| QueuedItem | 18 | 140 | game_queued_items |

Total:64 campos fonte,287 linhas. O ID de CharacterInfo referencia Character.ID; LastConnectInfo.ID referencia CharacterInfo.ID. As duas FKs foram recuperadas e conservadas: Character→CharacterInfo usa ON DELETE/UPDATE CASCADE; CharacterInfo→LastConnectInfo usa ON DELETE CASCADE/ON UPDATE NO ACTION. A migração0008 corrige as ações por avanço, preservando o checksum da0007 já aplicada no laboratório. Não há FK de QueuedItem no catálogo; CID é preservado sem inventar uma relação. Não foi adicionada FK de CharacterInfo.UID para game_users ausente na referência.

Todos os campos são explícitos, incluindo status/delete, aparência textual, stats, AP, conexões de café, processed/expiry/event, item IDs, durabilidade e correio. nvarchar é UTF16 little-endian em bytes, preservando NULL, vazio, NUL e surrogates; timestamp/rowversion é oito bytes opacos. datetime é unidade exata1/300s desde1900; datetime2(7) é tick100ns desde0001, sem atribuir fuso. bigint usa strings inteiras canônicas no transporte JSON, evitando perda numérica em ferramentas intermediárias. tinyint possui faixa0–255 no domínio/schema.

Esta migração preserva snapshots. Não reproduz defaults de INSERT de personagem/fila, não aloca identidades de QueuedItem nem fornece criação de novas entidades. IDs são explícitos da referência; nenhum MAX+1. Nomes não receberam uma chave ASCII que limitaria os nomes Unicode originais; as consultas portadas usam UID/ID.

## Consultas e limites

GetCharacters seleciona todos os campos por UID e ordena COALESCE(OrderNum,255),ID. Não filtra status, DeleteTime ou conexão: esse filtro não existe na procedure. A API Rust recusa resultados acima de1000 em vez de truncar silenciosamente.

GetLatestConnectedDate conserva MAX(CASE WHEN LastConnectDate<LastCafeLogout THEN LastCafeLogout ELSE LastConnectDate END) para IDs do usuário. Logout NULL usa LastConnectDate; ausência de registros retorna None. Não usa LastLogout ou datas de criação como substituto.

O oracle executa os dois corpos recuperados em procedures/tabelas temporárias da conexão. GetCharacters usa uma tabela sintética com as colunas necessárias para comprovar filtro/ordenação, inclusive status e delete diferentes. Seis casos de conexão comparam NULL, maior/menor/igual e instante anterior a1900; há também resultado vazio e MAX de vários personagens. As tabelas persistentes da referência são somente contadas. Os64 campos completos são verificados separadamente por snapshots Rust e comparação da importação real.

## Importação privada e atomicidade

A leitura SQL Server é SERIALIZABLE, SELECT-only, com contagens e dados no mesmo snapshot e limite1000 por tabela. A exportação privada fica em .cache/private-character-import com ACL do usuário atual e SYSTEM; nomes e conteúdo de correio não aparecem em logs/evidências públicas. O CLI aceita apenas stdin.

O parser limita6MiB, exige todas as colunas (inclusive nullable), recusa campos desconhecidos, duplicidades, datas/bytes inválidos, contagens divergentes e IDs duplicados em cada tabela. As quatro tabelas são importadas numa transação, em ordem de dependência e IDs. INSERT ON CONFLICT não sobrescreve um vencedor; leitura FOR SHARE compara cada campo. Qualquer diferença ou FK inválida reverte o batch, inclusive linhas anteriores. VerifyOnly não insere linhas. lock_timeout2s, statement_timeout5s e deadline30s limitam espera; em timeout/queda, confira o mesmo snapshot antes de repetir porque o commit pode ter ocorrido.

~~~powershell
# Requer referência study_heroes restaurada e servidor Docker com migrações0007/0008.
# Executar uma vez e depois conferir; não imprime os valores privados.
pwsh -File scripts/database/Import-Characters.ps1 -Mode Import
pwsh -File scripts/database/Import-Characters.ps1 -Mode Verify
~~~

O modo Import recusa sobrescrever uma exportação privada anterior diferente. O modo Verify compara os64 campos e reexporta a referência por leitura para detectar drift. Banco novo após clone não contém os snapshots privados. Referência e catálogos devem existir localmente para usar estes helpers; testes comuns usam fixtures sintéticos versionados.

## Verificação da subetapa

Quatro testes novos do parser e quatro integrações PostgreSQL cobrem NULL/vazio/bytes/extremos, limites, colunas ausentes, contagens e IDs duplicados, todos os64 campos, mudança individual de cada campo não-chave, replay, VerifyOnly, rollback de FK inválida e conflito tardio,16 imports concorrentes (uma cópia/15 replays), upgrade6→8 preservando usuário, cascade de delete/update e recusa de update bloqueado pela FK filha, migração idempotente e readiness com coluna ausente. O gate final, build/smoke e comparação após reinício ficam no relatório/evidências; resultados intermediários com falha não são aceitos.

## Próxima ação

M13.2d.2b.2: recuperar qualificação/recompensas e operações QueuedItem, alocador identity compatível, relógios/defaults, processamento/expiração e transação/replay/concorrência. Estender o executor sem descartar recompensas obrigatórias, integrar observações reais de GetCharacters/GetLatestConnectedDate e tratar novos usuários/ownership. Preservar ordem de efeitos anterior a ban/IP. FacebookToken/UserLogin completo, ownership distribuído e TCP/cliente continuam pendentes.
