# Transação base de criação

M15.1b.2a verificada no escopo interno abaixo. O módulo interno character_creation_core porta os inserts da procedure CreateCharacter no perfil mínimo recuperado: Fiona normal, dono sintético com revisão admitida, nome candidato medido e valor exato de MySettings que foi conferido pela expressão original FreeItem. Outros perfis são recusados antes da escrita. Não há comando externo nem handler Create ligado a este módulo.

A ordem das recusas SQL é CID existente em CharacterInfo, nome em status diferente de1 e UID/CharacterSN em status diferente de1; os resultados correspondem a0/-3/-2. Inserts de Character, CharacterInfo e das quatro coleções ficam na mesma transação. O alvo serializa verificação/inserção de nomes medidos sob lock de tabela, com prazos, sem alegar equivalência de concorrência ao NOLOCK original. Colisões, erro tardio ou cancelamento não deixam personagem ou coleção parcial.

Os37campos de CharacterInfo recebem os defaults recuperados e Level1/Dirtytrue, com UTC SQL datetime para LastAPUpdate e a conversão datetime2 de CreateTime conferida nas300fases subsegundo por SELECT sintético. legacy_last_update ficaNULL para uma linha nova sem rowversion da referência; não se fabrica o contador SQLServer. O contrato de revisão local para uso posterior permanece necessário. MySettings não é copiado por personagem. O ramo de recompensas FreeItem habilitado permanece pendente e é recusado por escopo, sem desabilitar configuração.

Os testes isolados verificam todos os campos/defaults, colisões na ordem original, exclusão de status1 na procedure base, concorrência, dono/revisão, recusa de flags/settings não medidos e rollback tardio/cancelamento. Isso não substitui a verificação de nome e retenção anterior do CharacterMaker nem os slots, o ledger, o alocador e o loop de colisões no chamador.

Antes de ligar Create, completar esses passos e a inicialização de itens/skills/aparência/títulos e CharacterSummary. Depois: seleção/PlayerLoader, canal MMO, spawn real e movimento no mapa inicial. Nenhuma criação ou caminhada foi confirmada no cliente. A migração completa M01–M24 permanece obrigatória.

Comandos locais: .cache/recover-creation-clock.ps1 (SELECT-only; fonte inalterada); scripts/dev/verify.ps1 -DatabaseIntegration -DatabaseIntegrationDocker; .cache/verify-creation-core-docker.ps1. Fixture pública contém somente300valores temporais sintéticos; exportações privadas e logs completos permanecem no cache.

Gate final230Rust/84PGexecutados/19Python,0falhas/avisos; Docker/preservação/restart PASS. Evidência:evidence/auth/character-creation-core.json. Não houve Create no cliente.
