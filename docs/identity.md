# Identidade local de laboratório

Este módulo porta o cálculo de `NexonID`/`NexonSN` do ramo `NGM_disable` com contrato limitado e explícito. Ele não valida um passport online, não concede permissões, não cria contas e não implementa login do cliente. Contas HTTP/PostgreSQL continuam independentes dessa identidade.

## Evidência recuperada

- `legacy/decompiled/FrontendServiceCore-6be475728091/FrontendServiceCore.States/PassportCheck.cs:146`: o ramo local depende de `FeatureMatrix.IsEnable("NGM_disable")`.
- Linha 148: `zhCN` adiciona restrições de nomes de laboratório. Aceita prefixo `cmhtest`, sufixo de exatamente 2 unidades UTF-16 e inteiro 1–60; ou `whb`, sufixo de exatamente 3 unidades e inteiro 1–6. `int.Parse` permite sinais/espaços; por isso `cmhtest+1` e `whb +1` são exemplos aceitos no oracle. Não basta restringir a dígitos.
- Linhas 204–214: passport vazio falha; `NexonID` preserva o caso original e é truncado em 24 unidades UTF-16; `NexonSN = passport.ToLower().GetHashCode()` usa o **passport inteiro**, antes do truncamento. No ramo `zhCN`, o vazio falha anteriormente como `NoUserInfo`.
- Linha 215: o legado concede permissões amplas nesse ramo. Essas permissões não foram copiadas para Rust.
- `legacy/decompiled/LoginServiceCore-1600fb4b593d/LoginServiceCore.Processor/UserLoginProcessor.cs:77`: consulta `dbo.User` pelo nome; linhas 102–114 criam usuário ausente com `ID=NexonSN` e `Name=NexonID`, depois da etapa de passport. Reproduzir esses campos isolados não conclui esse fluxo de autenticação/persistência.

Frontend original SHA256: `6be47572809197138400163ea6f4484f45ba7228beadd15ab256daa653534e5b`. O fixture registra também SHA256 do fonte recuperado e do harness sintético.

## Perfil explícito e limites

`derive_lab_identity(Option<&str>, LabIdentityOptions)` exige `IdentityProfile`, sem perfil padrão. O chamador seleciona arquitetura `X86` ou `X64` e cultura `Invariant`, `EnUs`, `ZhTw` ou `ZhCn`. São perfis de hash **não aleatório**. Não inferir arquitetura apenas de um cabeçalho PE32: `Executer.exe` e `FrontendServiceCore.dll` têm flags CLR `0x1` (ILOnly, sem 32BITREQUIRED/32BITPREFERRED). A arquitetura efetiva depende de execução/host. O arquivo original `Executer.exe.config` declara CLR v4.0 e `.NETFramework,Version=v4.5`; não define cultura ou randomização.

Nenhuma atribuição explícita de `CurrentCulture` foi encontrada nos fontes recuperados de Executer, RCService e Frontend examinados. Isso não comprova a cultura do processo legado. O Windows da VM, configuração global e outros carregamentos ainda podem afetar esse valor.

A API aceita somente ASCII imprimível, unidades 32–126. Nesse conjunto, o oracle confirma a mesma conversão para minúsculas nas quatro culturas suportadas. Unicode/controles retornam `UnsupportedCharacters`; null retorna `NullPassport` em lugar da exceção legada. O limite máximo é configurável e aplicado antes da alocação. São restrições defensivas deliberadas, não paridade para todo passport aceito pelo legado.

`ToLower()` usa cultura atual; o `I` torna-se `i` nas culturas suportadas, mas `ı` em `tr-TR`. O diagnóstico turco está no oracle para impedir que uma conversão ASCII universal seja apresentada como prova de equivalência cultural. Ver [String.ToLower (.NET Framework)](https://learn.microsoft.com/en-us/dotnet/api/system.string.tolower?view=netframework-4.8).

`framework_string_hash(&[u16], FrameworkArchitecture)` calcula somente o hash; não faz normalização. A variante x86 empacota pares UTF-16 little-endian e usa deslocamento aritmético assinado; x64 alterna unidades e termina no primeiro NUL. Ambas usam overflow de 32 bits. Portanto `"ab"` resulta em `-840386625` no oracle x86 e `1093630535` no x64. O algoritmo é respaldado pelo [fonte oficial Microsoft de String.GetHashCode](https://github.com/microsoft/referencesource/blob/main/mscorlib/system/string.cs#L759).

O Framework permite hash aleatório por domínio quando configurado; perfis aleatórios são excluídos aqui. Não usar o hash moderno de .NET/Rust como substituto nem considerar esse inteiro como credencial segura. Colisões de 32 bits e nomes truncados repetidos precisam de tratamento no futuro adaptador persistente. Ver [UseRandomizedStringHashAlgorithm](https://learn.microsoft.com/en-us/dotnet/framework/configure-apps/file-schema/runtime/userandomizedstringhashalgorithm-element).

## Oracle e reprodução

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/reverse/generate-identity-fixtures.ps1
cargo test --test identity_compatibility --locked
```

O gerador compila somente `scripts/reverse/framework/IdentityOracle.cs`, código sintético próprio que adapta as expressões recuperadas. Não carrega nem executa DLL/EXE original. `TryParse` substitui o bloco `Parse`/catch para comparar sucesso/falha sem dependências dos serviços.

Foram gerados 2 executáveis próprios com os compiladores Microsoft Framework correspondentes, um `/platform:x86` e outro `/platform:x64`; `.exe.config` próprio fixa randomização em0. Dois processos novos por arquitetura produziram JSON idêntico. Ambiente observado: CLR `4.0.30319.42000`, BCL `4.8.4084.0 built by: NET48REL1`. Isso verifica essa BCL instalada, não uma execução do processo original v4.5 na VM.

`tests/fixtures/framework-identity.json` contém 2 arquiteturas × 5 culturas × 194 entradas: vazio/null, paridades de comprimento, 24/25 unidades, sufixos restritos, sinais/espaços, Unicode/NUL para o hash bruto e 160 entradas ASCII determinísticas com comprimentos 1–160. Casos são exclusivamente sintéticos, sem credenciais reais.

Os testes comparam hash bruto UTF-16 em ambas arquiteturas; identidade ASCII e restrições `zhCN`; divergência de cultura turca; rejeição de online/Unicode/controles; limite antes de alocar; e diferença entre nome truncado e hash completo. A verificação global deve ser executada após integrar o módulo; o resultado pertence à memória MIGRATION.md.

Ainda pendem perfil efetivo original, compatibilidade Unicode/cultura além deste subconjunto, adapter `dbo.User`/PostgreSQL, colisões, frontend e state machine de autenticação, mensagens/categorias completas e login real pelo cliente. M13 continua parcial.

