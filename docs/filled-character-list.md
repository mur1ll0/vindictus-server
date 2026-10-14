# Lista de personagens preenchida

M15.1b.2c verificada no escopo abaixo. O codeccharacter_list recupera CharacterListMessage479/GUIDf2750037-d9ab-3b34-9697-e1626e224040 com os39campos deCharacterSummary e28deCostumeInfo. PetNULL é o subset usado peloQueryCharacters original; PetnãoNULL é recusado explicitamente e continua pendente. Isso não torna a lista do runtime preenchida: o adaptador atual ainda exige consulta e inicialização completas dos personagens.

Fixtures273casos porarquitetura usam DTOs/serializer/reader originais e dados exclusivamente sintéticos. Cobrem todos os campos individualmente, ctor/coalescing, valores extremos, enums, listas/nullentries, dicionários ordenados efloatbits, e três modos de envelope. BaseCharacter ébyte, VocationEnum éint32 eNone=-1. BodyShape usa bitsfloat32, conservandoNaN/zero assinado; não converter parafloat64 nem presumirNone0.

Parse/encode bounded1000personagens/1000partystates/128entradas por dicionário, UTF8estrito, fim exato/truncation/trailing/limite de corpo. Chaves duplicadas são recusadas como noDictionary.Add da referência. Modelos privados não têmDebug/Serialize; corpo codificado e buffers parciais são zeroizados. Nenhum personagem, equipamento ou resposta de sucesso foi fabricado.

Comandos: scripts/reverse/generate-full-character-list-wire-fixtures.ps1; scripts/reverse/generate-fixtures.ps1; scripts/dev/verify.ps1 -DatabaseIntegration -DatabaseIntegrationDocker;.cache/verify-filled-character-list-docker.ps1. A regeneração básica emitiuCA2200 na fonte recuperadaMessageAnalyzer, separadamente do gateRust; nenhuma fonte original alterada. O oracle novo compilou semavisos e nenhum serviço legado foi executado.

Próximo paraCreate: chamador slots/CID-SN/collision/cleanup/dormant e initializer deitens/skills/Costume/títulos; consultar Summary desses dados e ligar o handler/retorno àCharacterList. Depois exercitarCreate no candidato1.69EN e conferir persistência/relogin. Seleção/cidade/movimento e migração integralM01–M24 continuam necessários.

Gate233Rust/87PGexecutados/19Python,0falhas/avisos;Docker/preservação/restartPASS. Codec apenas; nenhumCreate observado. Evidência:evidence/auth/filled-character-list-wire.json.
