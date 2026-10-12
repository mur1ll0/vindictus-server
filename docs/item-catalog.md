# Projeção efetiva do catálogo de itens

M13.2d.2b.2c.2b.3b.1 implementa a projeção necessária para existência de classe, equipamento, armazenamento e vínculo. O carregamento completo de ItemContents permanece parcial: stats, taxas aleatórias, demais tabelas e conversão integral dos DTOs são dependências separadas.

## Regras recuperadas

O source carrega EquipItemInfo habilitado por FeatureMatrix em dicionário com chave ordinal exata e sobrescrita de duplicatas. Depois, ItemClassInfo avalia Feature antes de IsValid: MaxStack ≤0 é inválido; para classe presente no dicionário de equipamentos, MaxStack deve ser exatamente 1. A validação precede a sobrescrita: uma duplicata posterior desabilitada ou inválida não apaga um vencedor anterior. Uma classe não passa a ser equipável por convenção de nome.

ItemClassExtraInfo também filtra por feature e sobrescreve duplicatas. StoryTabInInventory é consultado uma vez depois dessas três cargas, mesmo se não houver classes. Quando falso, somente Storage=5 passa a 1. Bind e TradeOnBind são conservados como dados, sem avaliação durante a carga. O objeto efetivo fornece BindingInfo para os predicados de vínculo portados na etapa anterior.

Chaves são UTF16LE opacas, inclusive NUL e surrogates. NULL na busca de classe significa ausente; NULL como chave habilitada de dicionário gera erro explícito. MaxStack inválido pode rejeitar a linha antes de alcançar sua chave NULL, conforme o source. Não há trim, normalização ou busca sem distinguir case.

## Ordem e limites

HeroesContentsLoader usa Select * From [Tabela], sem ORDER BY, e materializa os DTOs por reflexão. O exportador reproduz SELECT * somente leitura e preserva a sequência observada, sem ordenar ou deduplicar entradas. O resultado last-wins depende dessa sequência. A ordem histórica do provider na VM não foi observada; tampouco a conversão integral por reflexão foi portada. Erros nessa conversão podem gerar objetos parciais no legado; formatos não suportados são recusados neste exportador.

O domínio aceita até 50.000 linhas por tabela e 512 unidades UTF16 por chave. O reader admite até 16 MiB, exige campos nullable presentes, checa formato/manifest, quantidade de linhas, hashes e limites. Erros públicos omitem conteúdo; o JSON privado tem ACL para o usuário Windows e SYSTEM. Nenhum conteúdo original é commitado.

A política de feature conserva as 81 comparações finitas anteriores e adiciona 24 pares necessários ao perfil de conteúdo. Cada novo par foi executado no Framework x86/x64, com concordância entre invariant/en-US/zh-TW/zh-CN/tr-TR. A regra é String.CompareTo do source; não se substitui por comparação semântica de versões. Pares desconhecidos continuam sendo recusados.

## Conteúdo real e diagnóstico

O perfil selecionado pela configuração fornecida é EN-EU-X-GM. O arquivo tem 12.377 linhas de EquipItemInfo, 19.702 de ItemClassInfo e 6 de ItemClassExtraInfo. Antes dos filtros, há 1.095 duplicatas de equipamentos e 841 de classes. A projeção habilitada contém 10.967 equipamentos e 18.547 classes; 1.155 linhas de classes são desabilitadas, nenhuma habilitada falha em MaxStack, e as duplicatas de classe restantes são eliminadas pelo filtro de feature. Os seis extras ficam selecionados. Storage não muda nesse perfil, porque StoryTabInInventory está habilitado; casos sintéticos verificam sua remapagem.

~~~powershell
pwsh -File scripts/content/Export-ItemCatalog.ps1
pwsh -File scripts/reverse/generate-item-catalog-fixtures.ps1
# Depois de cargo build --locked; repita -Docker após construir a imagem.
pwsh -File scripts/content/Verify-ItemCatalog.ps1 -Docker
~~~

A CLI inspect-item-catalog --input-stdin lê a projeção privada e devolve somente contagens/hash. Esse comando retorna antes de abrir conexão PostgreSQL. O diagnóstico compara Framework, Rust nativo e Docker usando um digest canônico de chaves, índices dos vencedores, MaxStack, Storage, equippable, Bind e TradeOnBind. Ordenar o digest por bytes/hex é apenas uma definição do verificador; a seleção dos vencedores conserva a sequência de entrada.

## Evidência e próximo trecho

391 casos sintéticos por arquitetura verificam contagens/digest, erros e ordem das consultas de feature. Há mais 120 observações de pares de versão por arquitetura: 1.022 comparações no total. Seis testes Rust incluem integração do catálogo ao parser/binder, nulabilidade estrita e limites; quatro testes Python usam apenas SQLite temporário para verificar exportação, ordem, override, tipos e preservação por hash.

O oráculo extrai o trecho de inicialização até Storage, IsValid e os helpers de dicionário. A avaliação de feature usa o corpo real recuperado. Os DTOs são projeções próprias e GetItemStat é um shim que retorna null. Caixas aleatórias e todas as inicializações posteriores são excluídas de forma explícita; as flags de diagnóstico permanecem falsas para essas dependências.

Próxima subetapa 3b.2: recuperar GetItemStat/conversões dos campos relevantes e as taxas de ItemClassExtraInfo, com float/cultura/ordem e falhas comprovadas. Só depois ampliar a aceitação do catálogo completo e prosseguir com modelos/importação de inventário e entrega. Login do cliente permanece indisponível.

Evidências: docs/evidence/content/item-catalog.json, docs/evidence/auth/item-catalog.json e docs/evidence/verification/REPORT.md. Snapshots e fila reais devem permanecer inalterados durante a verificação.
