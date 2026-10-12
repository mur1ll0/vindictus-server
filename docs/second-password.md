# Segunda senha recuperada — M13.3a

A aplicação interna consulta, verifica, registra e limpa a segunda senha no PostgreSQL. Isso é uma dependência da seleção de personagem, distinta do passport e da conta HTTP. Não há mensagens de segunda senha enviadas pelo TCP nesta etapa, nem login de jogo aceito.

As declarações originais EN-EU-X-GM incluem SecondPassword=1.08.02, EssentialSecondPassword=1, SecondPasswordClear=1.08.02, SecondPassword_UsingSHA=0 e SecondPassword_IP_Limit vazio. O exportador somente leitura inclui essas cinco declarações; configuração incompleta ou perfil de identidade diferente do admitido recusa antes de IO. Versões de features usam a matriz recuperada, sem conversão para semver.

UserEntity.QuerySecondPassword inicializa NULL como vazio/contador0/dataNULL, mas o primeiro feedback usa o contador/data capturados antes dessa escrita. Contadores >=5 restringem por dez minutos. A próxima falha após expirar o contador5 reinicia em1; acerto limpa contador/data e troca o hash se o perfil mudou. GetHashVersion usa o comprimento UTF16 (<128/128/>128), inclusive seus casos ambíguos, sem interpretar o prefixo.

O modo0 porta StringEncrypter e System.Random do Framework com arquitetura/hash de string explícitos e chave ASCII. Os modos1/2 conservam SHA512 e fallback Encoding.ASCII por unidade UTF16; modo2 inclui o nome minúsculo e prefixo2:. Estes formatos são compatibilidade do laboratório. A conta operacional mantém Argon2id independente. Passwords/hashes não têm Debug/Serialize e buffers temporários são zeroizados; SQL é parametrizado.

Persistência usa lock curto na linha, ownership UID+nome, revisão gerenciada pelo trigger, transação e prazo de dez segundos incluindo aquisição do pool. A sessão exige PoliciesAccepted, perfil correspondente e identidade da mesma conexão. O prazo capturado na primeira consulta permanece na sessão mesmo quando NULL é inicializado e o banco limpa seu contador; reconsultas NoPassword/Passed usam o feedback em cache recuperado sem apagar esse prazo. Query cancelado permanece Unknown; check/change em erro permanecem Locked. Fechamento, reconexão e outra sessão negam seleção, mesmo depois de Passed. EssentialSecondPassword exige Passed; NoPassword só permite cadastrar a segunda senha.

Diferenças explícitas: restrição de tentativas é relida da linha persistida antes de check/change, enquanto o legado também mantém estado em memória; contadores negativos/overflow recusam writes; tamanho SQL >512bytes é recusado antes de IO. O legado captura exceções e pode produzir feedback padrão; Rust propaga erro tipado e nega seleção. IP_Limit habilitado recusa registro não vazio, pois SecondPasswordLimitCheck ainda não foi migrado (flag efetivo vazio). Não se simulam resultados dessa procedure.

## Verificação reproduzível

```powershell
pwsh -NoProfile -File scripts/reverse/generate-second-password-fixtures.ps1
cargo test --test second_password --locked
# TEST_DATABASE_URL configurada privadamente para banco descartável.
cargo test --test postgres second_password --locked -- --ignored --test-threads=1
pwsh -NoProfile -File scripts/dev/verify.ps1 -DatabaseIntegration
```

O harness extrai métodos/arrays de UserEntity em cópia ignorada, substituindo somente UtcNow por clock determinístico; StringEncrypter e SQLDateTime permanecem recuperados. Contexto de uma linha é um shim e não prova LINQ/SQL Server. Cada arquitetura produz63hashes/8comprimentos/126consultas/90checks/21registros sintéticos. Os cinco testes Rust comparam primitivas/planos; cinco integrações isoladas verificam a persistência real, CAS concorrente, timeout/cancelamento, estados/ownership/fechamento e rehash0→1→2→0. Hashes de fontes e mscorlib constam no fixture público; nenhum serviço original executado.

Evidência final e contagens estão em [second-password.json](evidence/auth/second-password.json) e [REPORT.md](evidence/verification/REPORT.md). Logs completos somente .cache/verification. Próximas dependências: codecs dessas mensagens, QueryCharacters/CharacterSummary e composição de CharacterList; depois criação, seleção, LoginOk e cidade/MMO. Migração completa permanece pendente.
