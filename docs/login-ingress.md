# UserLogin no ingresso categoria zero — M11.6

O diagnóstico frontend agora reconhece UserLoginMessage na categoria recuperada478 e no envelope polimórfico categoria0. Isso permite chegar à recusa explícita13 com ambos os formatos. Não autentica conta, não muda a sessão interna para jogo e não envia lista/personagem/LoginOk.

## Contrato recuperado

SerializeWriter.ToBinary<T> usa o codec selado quando ClassInfo<T>.CategoryId é diferente de0. Quando ainda é0, encaminha para o overload object. Esse overload usa categoria0 mesmo se a classe já estiver registrada. SerializeWriterHelper<object>.SerializeCoreVirtual escreve primeiro a identidade CLR do tipo concreto e depois o mesmo payload selado. MessageHandlerFactory constrói um dispatcher para categoria0 que usa SerializeReader.FromBinary<object>; o reader resolve o GUID e entrega o objeto ao handler.

UserLogin GUID `47f6f00c-0bd3-3059-afad-84a872ed7399`, categoria478, vem da metadata original ServiceCore SHA256 `be65d59d23859320236643b7bdf7496822bfe9d5ec0c63c364f50aa7d6412683`, concordante Framework x86/x64. Os16 bytes são a ordem de rede dos campos do GUID, não Guid.ToByteArray. Depois vêm presence e os nove campos já recuperados em login.rs, com null/vazio preservados.

Para ToBinary<UserLoginMessage>(null) antes do registro e ToBinary((object)null), o fallback escreve **System.Object** GUID `81c5fe01-027c-3e1c-98d5-da9c9862aa21` e presence0; não escreve o GUID de UserLogin. O reader recuperado retorna null. Rust conserva None, e o probe fecha com NullLogin. System.Object presente não é login: erro explícito.

## Limites e diferenças deliberadas

- O decoder empresta campos do buffer; não aloca a partir de lengths do peer, carrega tipos CLR ou retém passport/hardware. Não possui Debug nos campos do login.
- O limite do corpo inclui os16 bytes de GUID. Truncamento, overflow/length inválido, UTF8 inválido e trailing bytes são recusados. O reader original não valida consumo integral do corpo; a rejeição de trailing bytes é um endurecimento explícito, como nos codecs existentes.
- Só GUID de UserLogin e System.Object **null** são aceitos. Tipos desconhecidos, arrays/generics, GUID vazio e GUID do TypeConverter são recusados. Não implementa um deserializador polimórfico geral nem renegocia categorias enviadas pelo peer.
- O probe continua loopback, oito conexões, deadline absoluto10s, sem login/persistência. Bootstrap e estado contínuo do sender não mudaram.

## Evidência reproduzível

`scripts/reverse/generate-login-envelope-fixtures.ps1` compila o writer/reader/cifra recuperados sem alterar seus corpos, no Framework x86 e x64. Uma cópia ignorada de UserLogin recebe apenas o atributo Guid da metadata original, pois a identidade do assembly de harness difere. Campos são sintéticos; allocator é um shim. Fontes originais não são editadas, assemblies originais não executados. O script confere o hash do ServiceCore original e ambos os catálogos de metadata.

39casos por arquitetura:13objetos (um null e12variações) ×3caminhos (genérico não registrado, object registrado, tipado registrado). Recovered reader faz roundtrip para o payload tipado. O Rust compara todos78casos/payloads, recebe toda cifra byte a byte e verifica GUID/limites/prefixos/trailing/categorias/objeto. Dois testes TCP novos usam cifra de referência e rejeitam negociação remota/nulo. Regressão antes da implementação: teste TCP categoria0 falhou com UnsupportedCategory(0); após mudança passou.

Comandos: gerar oracle acima; `scripts/reverse/generate-fixtures.ps1` (exigido por AGENTS para alteração de protocolo); `scripts/dev/verify.ps1 -DatabaseIntegration`; build/smoke Docker. Logs completos em .cache/verification; hashes/contagens sanitizadas em docs/evidence/auth/login-envelope.json e relatório do gate.

Cliente não executado. Não há evidência de que o cliente candidato efetivamente envie categoria0; a etapa remove uma lacuna conhecida no contrato do dispatcher recuperado. Próxima dependência prioritária: retorno integral UserLogin e adapter TCP→sessão, com QueryCharacters/lista/segunda senha recuperados antes de qualquer sucesso ao cliente. Caminho e bloqueios em client-minimum-path.md; stats/rates/inventário e objetivo final continuam pendentes.
