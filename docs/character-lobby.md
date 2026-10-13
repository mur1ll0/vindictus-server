# Consulta inicial de personagens — M13.3c

A aplicação interna prepara CharacterListMessage somente quando a consulta real de CharacterInfo por UID está vazia. Isso recupera um caminho de conta nova; qualquer registro de personagem recusa a dependência CharacterSummary/aparência ainda não portada. Não omite personagens existentes nem envia uma lista constante para esconder dependências. O frontend TCP continua diagnóstico.

## Persistência recuperada

A migração0011 conserva UserInfo (3campos,25linhas), UserProducts (5campos,0linhas) e UserCareInfo (4campos,2linhas). Não cria FKs que a referência não possui. NexonSN/UID são signed32; CID/ID signed64; nvarchar128 conserva até256bytes UTF16, inclusive NUL/surrogates; datas conservam datetime2(7) em ticks e datetime em unidades1/300s. UserProducts.ID tem identity1/1, cujo last_value é NULL na origem vazia. O importer não expõe alocação de novos produtos nem reseeding.

Importação stdin é limitada2MiB/1000linhas por tabela, exige nullable presentes, datas/IDs64 em strings inteiras canônicas e chaves sem duplicatas. Transação faz INSERT-or-compare de todos os campos, sem UPDATE do vencedor; conflito posterior causa rollback. Replay/verify não alteram registros. Arquivo privado tem ACL de usuário Windows atual e SYSTEM; evidência pública contém somente hashes/contagens.

## Consulta e efeitos

QueryLastCID ausente/NULL retorna0; ProloguePlayed ausente retornafalse. Slots contam ID por NexonSN e ProductName CharacterSlot/PremiumCharacterSlot, sem filtro ExpireTime. Comparação usa somente ASCII imprimível com casing e espaços finais comprovados na collation SQL_Latin1_General_CP1_CI_AS; outras unidades recusam consulta, mas continuam preserváveis no snapshot. Erro de banco é explícito, em vez do catch legado que devolve0.

A consulta exige UID+nome+revisão da conexão admitida, transação REPEATABLE READ, lock_timeout2s/statement_timeout5s e deadline10s incluindo pool. Também consulta o contador real UsePreset && Status!=1. MaxCharacterSlot e o ramo trusted Cheat são recuperados de CharacterContents; o perfil candidato declara20 e DisableCharacterSlotCheat=1. A aplicação exige as declarações relevantes, sem valores presumidos para flags ausentes.

UserCareSystem=2.00.14 está habilitado em CurrentVer3.09.00: o callback de lista vazia executa UserCareInfoUpdate(UID,1,2,DateTime.Now). A MERGE original tem UID como única chave e substitui Type/State/LastUpdate ou insere. Rust usa UPSERT equivalente com clock civil explícito e conversão SqlClient datetime recuperada; se o flag está desabilitado, nem avalia esse relógio. Fonte/runtime e perfil de clock continuam limites explícitos do laboratório, sem inferir fuso pelo PostgreSQL.

## Corpo de rede e limites

CharacterListMessage tem categoria479 e GUID original f2750037-d9ab-3b34-9697-e1626e224040. O codec aceita coleção de personagens NULL/vazia; recusa qualquer elemento antes de interpretá-lo. Preserva três contadores de slots int32, byte ProloguePlayed, contador de preset int32 e coleção nullable de LoginPartyState int32 (limite1000). NULL/vazio, signed32, presença, counts e envelopes categoria0 usam o serializador recuperado. A mensagem é servidor→cliente; decoder não autoriza recebê-la como comando no futuro dispatcher.

A aplicação converte seu resultado real para coleção presente vazia e LoginPartyState presente vazio, conforme os loops originais sem personagens. LastCID é usado pelo ramo SingleCharacter no legado e não é um campo desta mensagem. Não há summaries, aparência padrão fictícia, seleção ou LoginOk nesta etapa.

## Verificação reproduzível

```powershell
pwsh -NoProfile -File scripts/reverse/generate-lobby-feature-fixtures.ps1
pwsh -NoProfile -File scripts/database/Compare-LobbyProducts.ps1
pwsh -NoProfile -File scripts/reverse/generate-empty-character-list-wire-fixtures.ps1
pwsh -NoProfile -File scripts/reverse/generate-fixtures.ps1
cargo test --test character_lobby --test empty_character_list_wire --locked
cargo test --test postgres lobby_ --locked -- --ignored --test-threads=1
pwsh -NoProfile -File scripts/dev/verify.ps1 -DatabaseIntegration
# Depois do gate e build/migrate Docker:
pwsh -NoProfile -File scripts/database/Import-CharacterLobby.ps1 -Mode Import
pwsh -NoProfile -File scripts/database/Import-CharacterLobby.ps1 -Mode Verify
```

Três testes Rust comparam preservação/limites, nove comparações SQL somente SELECT e30 observações Framework de features (2arquiteturas×5culturas×3pares). Quatro testes wire comparam183casos/arquitetura com DTO original completo e serializer/reader, campos inteiros diferenciados, três formas de envelope, framing/cifra retida em fragmentos e entradas malformadas/limites. Quatro testes PostgreSQL isolados verificam importação/replay/conflito/concorrência, slots expirados, UID-only UPSERT, ownership/revisão/fechamento, cancelamento, readiness e a integração consulta→corpo codificado. Não executa serviços originais. Logs completos somente .cache/verification.

A etapa passou com195Rust/56PostgreSQL/19Python e0falhas/avisos. Importação25/0/2, replay e igualdade de todos os campos após restart Docker passaram; a evidência está em [character-lobby.json](evidence/auth/character-lobby.json), [character-lobby-import.json](evidence/database/character-lobby-import.json) e [REPORT.md](evidence/verification/REPORT.md). Próximo: ligar TCP ao login/segunda senha/lista para conta sintética nova e executar o cliente; depois criar personagem com defaults reais, portar Summary/seleção/PlayerLoader/LoginOk e canal MMO/cidade. A migração completa continua pendente.
