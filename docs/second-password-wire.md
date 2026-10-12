# Mensagens de segunda senha — M13.3b

O módulo protocol::second_password implementa os corpos dos cinco tipos recuperados. Não despacha operações no frontend: o cliente ainda recebe a recusa diagnóstica. A aplicação persistente permanece descrita em [second-password.md](second-password.md).

| Tipo original | Categoria | Direção no GameClient | Campos |
| --- | ---: | --- | --- |
| AskSecondPasswordMessage | 231 | Servidor → cliente | Presença da classe |
| SecondPasswordMessage | 405 | Cliente → servidor | Password nullable UTF8 |
| SetSecondPasswordMessage | 572 | Cliente → servidor | NewPassword nullable UTF8 |
| HasSecondPasswordMessage | 413 | Servidor → cliente | Três bools, FailCount e RetryLockedSec int32 |
| SecondPasswordResultMessage | 562 | Servidor → cliente | OperationType int32, Passed bool, FailCount e RetryLockedSec int32 |

Categorias e GUIDs são conferidos nas tabelas originais x86/x64. O codec aceita o corpo tipado ou categoria0 com o GUID exato do tipo escolhido. System.Object só representa NULL; objetos não nulos, outros tipos, categorias diferentes, truncamento, bytes excedentes e tamanho acima do limite recusam. Não registra classes remotas. Int32 é big-endian; strings usam o comprimento variável recuperado, preservando NULL/vazio/NUL. UTF8 inválido é recusado explicitamente em vez do fallback permissivo do Framework. Valores desconhecidos da enum de resultado permanecem int32 opacos; o codec não lhes atribui comportamento.

Decoder empresta as strings e não implementa Debug/Serialize para mensagens com senha. Encoder retorna Zeroizing<Vec<u8>>; saída parcial em erro também é zeroizada. O futuro dispatcher deve preservar as direções e estados recuperados; possuir codec de uma resposta não autoriza recebê-la como comando.

## Verificação reproduzível

```powershell
pwsh -NoProfile -File scripts/reverse/generate-password-wire-fixtures.ps1
pwsh -NoProfile -File scripts/reverse/generate-fixtures.ps1
cargo test --test password_wire --locked
pwsh -NoProfile -File scripts/dev/verify.ps1 -DatabaseIntegration
```

Harness próprio usa SerializeWriter/SerializeReader/Packet recuperados sem alterar os métodos. As cinco cópias ignoradas dos DTOs recebem somente o GuidAttribute da identidade original: a identidade do assembly de teste seria diferente. O allocator é um shim sintético. Nenhum serviço original é executado. Os426 casos por arquitetura cobrem142 mensagens em três formas (genérico não registrado, object registrado, tipado registrado), incluindo NULLs, strings Unicode/vazias/NUL/limites e extremos signed32. Rust compara campos e bytes exatos do corpo, verifica a tabela original, malformações e limitações. Integração com a cifra frontend já recuperada conserva o estado entre426 frames e alimenta o receiver em fragmentos de três bytes; isso é teste de composição, não observação do cliente.

A evidência final está em [second-password-wire.json](evidence/auth/second-password-wire.json) e [REPORT.md](evidence/verification/REPORT.md). Gate188Rust+52PostgreSQL+19Python,0falhas/avisos; build Docker/migrate/readiness/duplicate/restart e preservação287snapshots64campos/29users15campos/fila140/0recibos PASS. Próximos bloqueios: consulta real/CharacterList, composição TCP, criação/seleção/LoginOk e canal MMO. A migração completa, inventário/entrega e batalha continuam pendentes.
