# Cliente, endereço e contas: evidências e limites

Verificação estática e preparação de cópia em 2026-10-10. Nenhum executável legado foi iniciado e nenhum arquivo original de cliente/servidor foi alterado nesta investigação.

## Qual cliente usar

O candidato para os primeiros testes de compatibilidade é `../Vindictus1.69EN/Vindictus EU/en-EU`. A pasta `../Client` tem os mesmos hashes SHA-256 para todos os nove arquivos comparados, inclusive `Vindictus.exe`, `bin/client.dll`, `bin/server.dll`, as duas DLLs em `bip`, os scripts de inicialização e as duas configurações comparadas. Isso prova equivalência desses arquivos; não prova equivalência de todos os recursos nem compatibilidade jogável com o servidor.

Os dois `start.bat`, linha 1, contêm:

```bat
start vindictus.exe -stage -dev -lang en-eu-x-gm -noupdate -threads 4
```

O nome da pasta sugere versão 1.69. `CurrentInfo.txt` informa data de compilação/distribuição de 2016-07-11. A versão de protocolo e o pareamento completo com os serviços ainda precisam de validação. Não usar um cliente atual como substituto presumidamente compatível.

O inventário reproduzível está em [evidence/client/inventory.json](evidence/client/inventory.json). Para gerá-lo novamente a partir da raiz deste projeto:

```powershell
py -3.13 scripts/client/inventory_client.py
```

O script abre os bancos SQLite com `mode=ro` e `PRAGMA query_only=ON`, compara hashes e registra strings com offsets; não executa nem modifica binários legados.

## Endereço IP do servidor: bootstrap recuperado

A inspeção estática recuperou dois endereços distintos. Em `sql/heroes.db3`, a linha `FeatureMatrix.EndPoints` da coluna `EN-EU-X-GM` aponta para `http://192.168.0.200/en-EU-x-GM/EndPoint.txt`. Esse é o servidor HTTP que publica a lista. No disco VMDK do pacote, o arquivo `xampp/htdocs/en-eu-x-gm/EndPoint.txt` contém o endereço de jogo `192.168.50.227:27015` dentro da árvore KeyValues `Address / Vindictus Test / IP`. São valores antigos e inconsistentes entre si; ambos precisam ser ajustados para o laboratório.

A rotina nativa de `engine.dll` consulta `EndPoints`, usa WinINet para buscar o texto e lê `IP`, `platform_server_code` e `localized_name`. A evidência inclui SHA256, offsets, disassembly e arquivo extraído em [evidence/client/endpoint-static.json](evidence/client/endpoint-static.json). A extração do VMDK usa 7-Zip sem iniciar a VM. O SQLite `hfs_extracted/sqlite/heroes.db3` contém URL oficial diferente; a precedência do override `sql/` sobre os arquivos HFS ainda precisa de validação durante a execução.

O frontend legado usa TCP27015 (`ServiceCore.dll.config:50` e `FrontendServicePool.cs:92`). O Rust atual oferece HTTP operacional8080 e um comando diagnóstico loopback probe-frontend na TCP27015. Esse comando só envia bootstrap e recusa o login; ainda não implementa o frontend completo do jogo. Nenhuma dessas verificações afirma que o cliente já consegue conectar ou autenticar.

### Preparar uma cópia do cliente

Use uma cópia completa de `../Vindictus1.69EN/Vindictus EU/en-EU` em `.cache/play-client`, com `sql/heroes.db3` preservado. A ferramenta recusa os diretórios originais e hard links para o banco original. Ela altera uma única célula, cria backup SQLite consistente e gera o arquivo HTTP:

```powershell
# Dry-run: somente apresenta as alterações planejadas.
py -3.13 scripts/client/prepare_lab_client.py --client-copy .cache/play-client --web-root .cache/client-bootstrap --bootstrap-base-url http://127.0.0.1:8081 --game-host 127.0.0.1 --game-port 27015
# Aplicar à cópia com backup antes da alteração.
py -3.13 scripts/client/prepare_lab_client.py --client-copy .cache/play-client --web-root .cache/client-bootstrap --bootstrap-base-url http://127.0.0.1:8081 --game-host 127.0.0.1 --game-port 27015 --apply
# Publicar a lista localmente em outra janela/terminal durante o teste.
py -3.13 -m http.server 8081 --bind 127.0.0.1 --directory .cache/client-bootstrap
```

Para um cliente em outra máquina, substitua o host HTTP e o host de jogo pelo IPv4 do servidor na rede local e publique o HTTP naquele endereço. A porta HTTP da lista e a TCP do jogo têm funções distintas. Só publicar `EndPoint.txt` não cria o frontend nem os serviços de batalha.

A mudança foi executada sobre uma cópia isolada do SQLite real em `.cache/client-lab` e testada com sete testes de preservação. O hash original permaneceu idêntico; [preparation.json](evidence/client/preparation.json) registra a prova. Esse diretório contém somente o banco copiado para testar o preparador, não um cliente completo. A aceitação durante a execução do cliente permanece pendente.

Os `.cfg` originais usam uma codificação XOR recuperada; não devem ser sobrescritos com texto simples. `start.bat` mantém o comando já existente `-stage -dev -lang en-eu-x-gm -noupdate -threads 4`. Não foi inventado um parâmetro `-ip`, `-port` ou `+connect` para login.

## Como o legado cria contas

A autenticação inicial é baseada em um **passport/sessão**. `legacy/decompiled/ServiceCore-be65d59d2385/ServiceCore.EndPointNetwork/UserLoginMessage.cs:8` declara `Passport`; o mesmo contrato contém endereço local, MachineID, NexonID, UpToDateInfo e CheckSum. Não é o contrato de uma API HTTP de cadastro com usuário e senha.

Em `legacy/decompiled/FrontendServiceCore-6be475728091/FrontendServiceCore.States/Initial.cs:28`, o frontend recebe `UserLoginMessage` e entra em `PassportCheck` na linha 45. O caminho normal de `PassportCheck.cs:260` solicita `CheckPassport` ao LoginService, que valida a sessão e também tem verificações de versão em `LoginServiceCore.Processor/CheckPassportProcessor.cs:132` e linha 140.

Existe um modo de laboratório já presente nos binários, condicionado ao feature flag `NGM_disable` (`PassportCheck.cs:146`). Nesse caminho, um passport não vazio vira `NexonID` (limitado a 24 caracteres), e o identificador numérico é calculado com `passport.ToLower().GetHashCode()` (`PassportCheck.cs:209` e linha 214). O flag vale `1` nas colunas `EN-EU-X-GM` e `ZH-TW-X-GM` tanto no SQLite do cliente quanto no `heroesContents.db3` do servidor, conforme o inventário. A cultura, a versão do runtime, a arquitetura e o comportamento exato desse hash precisam ser tratados como compatibilidade, não substituídos por um hash Rust arbitrário.

Depois que a autenticação encaminha `UserLogin`, `legacy/decompiled/LoginServiceCore-1600fb4b593d/LoginServiceCore.Processor/UserLoginProcessor.cs:75` procura o nome em `dbo.[User]`. Se ele não existe, as linhas 101 a 115 constroem e inserem o registro com o identificador de sessão. Isso é **provisionamento de um usuário do jogo após autenticação**, não evidência de senha local ou de cadastro independente. O modelo `LoginServiceCore/User.cs:7` mapeia `dbo.[User]`; `SecondPassword` é um campo distinto e não comprova autenticação primária com senha.

Também existe `TryCheckDeveloperPassport` (`LoginServiceCore/LoginService.cs:198`), mas `EnableDeveloperPassport` está `False` no arquivo original `ServiceCore.dll.config:260`. Não foi ativado nem usado. O comportamento de laboratório não deve ser promovido a serviço público.

## Contas da base Rust

O comando de provisionamento da base Rust é:

```text
vindictus-server create-account --username estudante --password-stdin
```

Ele exige uma senha pela entrada padrão e armazena uma conta no esquema independente do PostgreSQL. As migrações devem estar aplicadas. Em PowerShell7, use o helper que solicita a senha sem exibi-la e a envia ao CLI por stdin:

```powershell
pwsh -File scripts/dev/create-account.ps1 -Username estudante
```

**Essa conta ainda não permite entrar no cliente Vindictus.** Falta implementar e validar o adaptador entre autenticação local, passport do laboratório, identificadores legados, frontend TCP e sessão do jogo. Criar uma linha manualmente em `dbo.[User]` também não resolve os serviços e contratos ausentes.

## Critérios para marcar compatibilidade como concluída

- [x] Comparação dos executáveis e scripts dos dois candidatos, registrada por SHA-256.
- [x] Leitura somente leitura das tabelas e flags relevantes dos SQLite legados.
- [x] Fluxo estático de criação de usuário e handshake inicial recuperado dos assemblies .NET.
- [x] Formato e origem estáticos recuperados; preparador testado em cópia SQLite com backup. Aceitação no jogo ainda pendente.
- [x] Framing, ISAAC, transporte cifrado e payload UserLogin comparados com fixtures C# recuperadas. Negociação e sessão reais ainda pendentes.
- [ ] Login local com falhas controladas, sem conceder permissões privilegiadas por padrão.
- [ ] Criação/listagem de personagens e sessão de jogo verificadas.
- [ ] Entrada em batalha e persistência após reconexão verificadas.

## Teste técnico disponível agora
O serviço operacional em Docker pode ser testado por /health/ready e /capabilities em http://127.0.0.1:8080. O segundo continua informando game_client_compatible=false e game_authentication_available=false.

Para diagnóstico de rede no próprio host, sem autenticação:
    cargo run --locked -- probe-frontend --bind 127.0.0.1:27015

O endereço de jogo da cópia preparada pode apontar para127.0.0.1:27015 durante futura investigação de handshake. A ferramenta emite bootstrap e rejeita logins com Reason13/PermissionFailed; não existe cenário de jogo aceito. Essa porta não é publicada pelo Compose e o probe não aceita bindings para outras máquinas. O envelope categoria0 de UserLogin foi recuperado e testado contra Framework x86/x64 e sockets reais (M11.6; [contrato](login-ingress.md)). Demais tipos polimórficos, ligação à sessão e aceitação pelo cliente continuam pendentes. Use o cliente legado fornecido como candidato, não um cliente atual presumidamente compatível.

Prioridade intermediária atual: login → seleção → cidade/movimento básicos. Consulte [caminho mínimo e bloqueios](client-minimum-path.md); isso não conclui a migração completa nem o aceite de batalha/recompensas.
