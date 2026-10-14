# Preparação recuperada de entrega de itens

M15.1b.2e.2 implementa a parte pura de `ItemFactory.MakeVirtualItems`, `MergeDuplicatedRequest` e `ItemTraitExtension.IsStackableWith`. `ItemImpl.CustomizeCharacter` passa os itens padrão por `FromDictionaryEx`, entrega e chama `SaveWithoutSync` antes de produzir o resultado de aparência. Esta preparação é uma dependência desse fluxo; não grava nem entrega itens.

A API recebe propriedades já construídas e a projeção `EffectiveItem` do catálogo. A construção de `ItemRequestProperty` (incluindo atributos gerados), resolução de expiração e stats permanece uma responsabilidade anterior e ainda pendente. A preparação:

- recusa o lote inteiro quando falta uma classe ou MaxStack/count não é positivo;
- conserva a ordem de primeira ocorrência e agrupa na primeira propriedade compatível;
- divide cada grupo por seu MaxStack original;
- conserva integralmente a primeira propriedade, inclusive cores, durabilidade, Arg2 e BeforeItemID, mesmo quando esses campos não participam da comparação;
- compara classe/chaves/valores por unidades UTF-16 exatas, vínculo (com exceção se qualquer Storage é 3), ticks de expiração e atributos Value/Arg; equipamentos não se agrupam;
- reproduz adição Int32 unchecked, inclusive overflow que elimina uma pilha com total não positivo. Isso é comportamento medido, não validação suficiente para permitir entregas no servidor;
- retorna erro explícito ao ultrapassar limites de 4096 pedidos/pilhas, 128 atributos, 1024 bytes por texto UTF-16, datas Framework ou orçamento de comparação. Não devolve resultado parcial.

O oracle isolado extrai os três métodos originais para `.cache/framework`, compila contra Framework4.8 x86 e x64 e usa somente classes de fronteira próprias. Os serviços antigos não são executados. A fixture pública contém exclusivamente entradas sintéticas, resultados e hashes; os métodos proprietários permanecem ignorados. Há 361 comparações e 585 lotes por arquitetura: ordem, NULL versus vazio, Arg/Arg2, atributos fora de ordem, diferenças de cores/durabilidade, storage, equipamentos, classes ausentes, contagens inválidas, divisão e overflow. Rust compara também a identidade da propriedade de saída, não apenas valores coincidentes.

Reproduzir:

~~~powershell
./scripts/reverse/generate-item-preparation-fixtures.ps1
cargo test --test item_preparation --locked
./scripts/dev/verify.ps1 -DatabaseIntegration -DatabaseIntegrationDocker
~~~

A fixture não mede o construtor real de ItemRequestProperty, Item.Initialize, ItemOwner.GiveItem, colocação em storage, IDs, checksum, equipamento ou persistência. Próxima etapa: recuperar esses componentes e preservar Item/ItemAttribute/Equippable/storage antes da entrega inicial. Depois integrar Costume, habilidades, títulos e Summary real ao lifecycle; só então ligar Create TCP e pedir novo teste no cliente. Seleção, cidade e movimento continuam pendentes. Nenhuma compatibilidade adicional foi observada.

Metadados recuperados para a continuação: Item tem nove colunas e ID não identity; ItemAttribute tem seis colunas e ID identity; Equippable tem sete colunas e ID não identity; ItemStorageStatus tem sete colunas e ID identity. As FKs declaradas de OwnerID em Item e ItemStorageStatus apontam para ItemCollection, com DELETE CASCADE. O catálogo de referência não declara FKs de ItemAttribute/Equippable. A futura migração deve verificar linhas e vínculos antes de acrescentar restrições, sem presumir OwnerID diretamente como Character nem calcular uma identidade via MAX+1. Fontes: schema de referência versionado e DBLoader/Item.Initialize ignorados; nenhum dado privado foi publicado.
