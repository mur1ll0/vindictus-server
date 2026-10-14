# Auditoria exigida pela criação

M15.1b.2b verificada no escopo abaixo. O ramo check_for_creation usa as regras reais de nome/estado/retention e grava ChangedNameCheck depois de sucesso, como CharacterMaker.CheckCharacterName(...,needChangedNameLogging=true). O Check da tela permanece no ramo false. O registro contém JournalID0, Event com prefixo e NexonID, Target com nome/período/COUNTzero e Value0. Nenhum parâmetro privado é publicado em logs.

Os dois ledgers mantêm os seis campos e os limites nvarchar50/256, TimeSQLdatetime e LastUpdateTime de referência opcional. Não há PK/identity/FK/unique no catálogo original. Buffers UTF16 são zeroizados. Registros novos não fabricam rowversionSQLServer. DATEDIFFday2011-01-01/GETDATE escolhe01 para dias pares e02 para ímpares; o defaultTime é uma segunda amostra, conforme a procedure. O offset civilSQL da referência foi medido0, separado da civilCLR+8 do laboratório.13datas sintéticas SELECT-only incluem1753/1900/dias negativos em torno2011/meia-noite/9999.

O writer expõe somente os dois eventos da criação normal e exige dono/revisão sintéticos; não é uma porta genérica de AddCharacterLedger. O evento ProgressTitleGoal, ignorado pela procedure genérica, não pode ser solicitado por esta interface. Histórico6809/9616 e o writer genérico permanecem dependências da migração completa, sem serem lidos pelo Create mínimo. Nenhuma flag de logging foi desabilitada.

A reserva de nome continua sendo consumida e confirmada antes das verificações posteriores e do ledger; uma falha do ledger impede retorno de sucesso, mas não desfaz o consumo original já confirmado. Falhas de nome não geram registro. Falha/cancelamento durante append não deixam registro parcial; repetições de sucesso geram registros separados, sem deduplicação inventada. Não há handlerTCP de Create ou CLI para este writer.

Comandos locais: .cache/recover-creation-ledger-routing.ps1 (SELECT-only); scripts/dev/verify.ps1 -DatabaseIntegration -DatabaseIntegrationDocker; .cache/verify-creation-ledger-docker.ps1. Logs completos em.cache/verification; nomes/targets privados não são publicados.

Próximo: chamador com slots/CID-SN/collision/cleanup/dormant e inicialização completa de itens/skills/aparência/títulos/Summary. Somente depois ligar Create e exercitar o cliente. Seleção/cidade/movimento e migração completa M01–M24 permanecem pendentes.

Gate231Rust/87PGexecutados/19Python,0falhas/avisos;Docker/preservação/restartPASS. SemCreateTCP habilitado. Evidência:evidence/auth/character-creation-ledger.json.
