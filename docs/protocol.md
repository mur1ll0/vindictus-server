# Protocolo recuperado e evidência de paridade

O transporte genérico do Core, a variante Heroes do frontend, o TypeConverter e o payload de UserLogin foram portados em módulos independentes. O comando probe-frontend é um diagnóstico TCP loopback, com recusa de login; autenticação de jogo continua indisponível. O HTTP em8080 continua operacional.

## Fontes e método

Core.dll SHA256 `8942a0c26a9abf4c5f0d1f7ffff41452a3714d099e3b76012cebdee97293c2f0`: Packet, Isaac, CryptoTransform, MessageAnalyzer, SerializeWriter/Helper e SerializedSizeCalculator/Helper. ServiceCore.dll: Isaac, CryptoTransformHeroes, UserLoginMessage e IMessage. Os fontes recuperados são compilados em um oracle isolado, com apenas o alocador ChunkSegment substituído por arrays novos. Os assemblies originais não são executados. `scripts/reverse/generate-fixtures.ps1` regenera os casos.

O analyzer recuperado mantém um aviso CA2200 (rethrow no ramo OutOfMemory). Ele não foi alterado para esconder o aviso; o oracle usa entradas limitadas e não chama a importação Windows GlobalMemoryStatusEx daquele ramo. O Rust usa erros tipados e limites antes da alocação de payload.

## Transporte

- InstanceId: i64 big endian. CategoryId e comprimento: int32 com codificação variável de1–6 bytes, semelhante a UTF8, inclusive valores negativos.
- ISAAC: 256 palavras por bloco, consumo invertido, overflow32bits e apenas byte inferior de uma palavra inteira por byte de transporte. Há duas inicializações: Core grava seed no slot0; Heroes multiplica seed por4 (wrapping) e grava nos slots0 e(seed%64). FrontendService.OnClientAccept usa explicitamente CryptoTransformHeroes. O Rust exige CipherProfile::VindictusFrontend para essa porta; a API new() permanece perfil Core e não deve ser escolhida para o frontend.
- Cada direção tem estado independente. O primeiro pacote é cifrado inteiro e depois sobrescreve os primeiros8bytes com salt big endian. O receptor consome oito palavras mesmo nesse trecho sobrescrito e entrega InstanceId0 no primeiro pacote. Os seguintes preservam InstanceId e continuidade da cifra.
- Sender recebe salt explícito no intervalo de System.Random.Next. A seleção de salt para uma conexão real ainda será vinculada ao listener.
- Receiver aceita fragmentação, concatenação e bytes vazios. Não aloca a partir de comprimentos anunciados; entradas inválidas fecham o estado para impedir tentativa de ressincronização incorreta.

O limite padrão102400bytes e a rejeição de strings UTF8 inválidas são endurecimentos deliberados. O legado só avisava sobre mensagens grandes e substituía UTF8 malformado. Testes provam equivalência para mensagens válidas dentro dos limites, não para todos os comportamentos malformados do servidor antigo.

## Payload de login

A mensagem inclui marcador de presença (bool), Passport string anulável, LocalAddress u32, hwID string anulável, MachineID byte[] anulável, GameRoomClient i32, IsCharacterSelectSkipped bool, NexonID string anulável, UpToDateInfo string anulável e CheckSum i64. Strings/arrays usam count0 para null e count(comprimento+1) para presentes; string usa comprimento UTF8 em bytes, sem terminador.

`src/protocol/login.rs` decodifica campos emprestados, preserva null versus vazio e não fornece Debug para evitar registrar passport. Conta e senha Argon2 não fazem parte desse payload. Fixtures são sintéticas, sem credenciais reais. Categoria1 no oracle é exclusiva do teste de serializer: não é opcode de login.

## Categorias e metadados

`scripts/reverse/export-metadata.ps1` usa PEReader/MetadataReader para ler ServiceCore.dll sem Assembly.Load. A evidência `docs/evidence/reverse/messages.json` contém673 tipos IMessage concretos, tokens e campos em ordem original. `Messages.Types` filtra tipos do namespace; `Extension.GetConverter` numera os selados a partir de1, atribuindo0 aos demais.

A reconstrução por TypeDef foi confirmada por `scripts/reverse/export-framework-types.ps1` usando Assembly.ReflectionOnlyLoadFrom no .NET Framework instalado, nas arquiteturas x86 e x64. As673 mensagens têm os mesmos nomes, categorias e GUIDs nos dois processos. São649 categorias não zero, todas para tipos não genéricos e com GUIDs únicos. UserLoginMessage=478, SyncFeatureMatrixMessage=185 e LoginFailMessage=484 nessa versão exata de ServiceCore.dll. O perfil gerado em `src/protocol/server_profile.rs` conserva o SHA256 `be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683`; não deve ser reutilizado para um DLL de outro hash. Isso confirma a tabela do servidor fornecido, não a aceitação do cliente durante uma sessão.

ReflectionOnly examina metadados sem executar métodos, construtores ou atributos dos assemblies originais, conforme [documentação Microsoft](https://learn.microsoft.com/en-us/dotnet/framework/reflection-and-codedom/how-to-load-assemblies-into-the-reflection-only-context). Somente o programa revisado de inspeção e serializers recuperados recompilados são executados. `framework-types-x86.json` e `framework-types-x64.json` registram a evidência.

Os nove campos do login têm tokens contíguos67113148–67113156, confirmando a ordem usada na porta do payload. O GUID de UserLogin é `47f6f00c-0bd3-3059-afad-84a872ed7399` nessa versão.

## Pacote inicial TypeConverter

FrontendService.OnClientAccept transmite MF.GetTypeConverter() por SerializeWriter.ToBinary(object), depois SyncFeatureMatrix. TypeConverter tem GUID `ffffffff-ffff-ffff-ffff-ffffffffffff`, categoria de framing0 e um campo IDictionary<int,Type>. O corpo contém GUID16bytes, count(n+1) e cada par int32 big endian + GUID16bytes em ordem de campos network. Dictionary null usa count0; vazio usa1. Não há marcador bool para o TypeConverter, pois ele é struct.

`scripts/reverse/generate-framework-fixtures.ps1` compila os serializers recuperados sem modificar os fontes e os executa no .NET Framework BCL. Um DTO mínimo copia o layout e o GUID explícito do struct privado original. O caso completo usa as649 Type carregadas exclusivamente por ReflectionOnly do DLL original. O fixture compara toda a tabela, não só dois exemplos. O .NET moderno não é usado como oracle desse caso porque a reflexão sobre campos internos de Guid/KeyValuePair varia entre implementações BCL.

`src/protocol/converter.rs` codifica e decodifica essa forma para tipos simples sem alocar de acordo com counts remotos. Count, tamanho, truncamento, GUID do envelope e trailing bytes são validados. É um codec: a validação de collisions/tipos permitidos ainda pertence aos futuros consumidores que aceitem tabelas remotas. O Frontend legado usa MessageHandlerFactory(external:true) e não registra ProcessTypeConverter para o peer; não deve ser criada uma exigência de converter de retorno. Tipos array/generic com descrição recursiva não são suportados por esse codec; a tabela recuperada de649 tipos não inclui esses casos.

## Próxima dependência

Capturar sessão real do cliente candidato, fixar o perfil original de identidade NexonSN/passport e portar estados Frontend/LoginService. Handshake e resposta de recusa comparados ao oracle não comprovam aceite pelo cliente ou autenticação.

## Features e sequência inicial
src/protocol/features.rs preserva presença de mensagem, dicionário null/vazio, strings UTF8 anuláveis e counts recuperados; rejeita chaves null/duplicadas e limita tamanho/contagem antes de alocar. O bootstrap em src/protocol/bootstrap.rs envia TypeConverter seguido de SyncFeatureMatrix apenas se houver overrides não vazios e devolve o sender já avançado.

FeatureMatrix.GetFeatureMatrixDic() copia somente overrideMap. A FeatureMatrix SQLite fornece a base por região; OverrideSetting/NGM_Disable alimentam overrides separados. Sincronizar todas as features SQLite como se fossem overrides mudaria o contrato do cliente.

## Diagnóstico TCP
probe-frontend é uma ferramenta independente de HTTP/PostgreSQL e só aceita bindings loopback. Envia a tabela649; no comando CLI os overrides ficam vazios. A API aceita overrides explícitos para comparação do pacote completo em testes. Aceita somente UserLoginMessage na categoria confirmada478, decodifica sem registrar passport/hardware e envia LoginFailMessage categoria484, Reason13 (PermissionFailed), BannedReason vazio. A cifra Heroes continua no mesmo estado da sequência inicial.

Essa recusa é uma política explícita do diagnóstico enquanto os estados de autenticação estão pendentes. Diferentemente do PassportCheck original, ele fecha a conexão após a recusa. Não grava usuário, não calcula identidade e não concede permissões. Categoria0 de entrada, mensagens adicionais e null login são recusados; o legado externo não aceita um TypeConverter de retorno automaticamente.

Recursos: body102400bytes, read4096bytes, no máximo8 tarefas de conexão, deadline absoluto10segundos cobrindo escrita/leitura e cancelamento de sockets no shutdown. Os testes usam TCP loopback real, fragmentação, recusa, EOF parcial, categoria inesperada, timeout e parada com sessão pendente.

## LoginFailMessage
src/protocol/login_failure.rs porta presence bool, Reason int32 big endian e BannedReason string anulável. Fixtures .NET Framework incluem valores negativos/extremos, null/vazio e UTF8. O número13 vem do enum FailReason recuperado; não é um opcode inventado.